//! Discovery runtime of the node: which interfaces are active (trust rules from
//! docs/networks.md), watching for network changes, and turning discovery results into
//! [`NodeEvent`]s.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use localdrop_core::{DeviceId, DeviceIdentity, GroupKey};
use localdrop_discovery::Subnet;
use localdrop_discovery::net::{NetworkId, list_interfaces, network_id};
use localdrop_discovery::{
    ActiveInterface, DiscoveryEvent, DiscoveryService, InterfaceClass, NetInterface, NetworkKey,
    ServiceConfig,
};
use localdrop_store::Store;
use localdrop_transport::{Endpoint, Session};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::task::JoinHandle;

use crate::NodeEvent;
use crate::api::{NetworkTrust, NetworkView};
use crate::settings::{NetworkSelection, OnNewNetwork, Settings, StateDir};

/// How often interfaces are polled for changes; a change must also be stable for one
/// more poll before discovery restarts (debounce for roaming and DHCP renewals).
pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// A connected interface that could carry discovery, with its network.
#[derive(Debug, Clone)]
pub(crate) struct Connected {
    pub iface: NetInterface,
    pub id: NetworkId,
}

/// The outcome of applying the trust rules to the connected interfaces.
#[derive(Debug, Clone, Default)]
pub(crate) struct Plan {
    pub active: Vec<ActiveInterface>,
    pub views: Vec<NetworkView>,
    /// Connected, not trusted, and the user should be asked.
    pub ask: Vec<(NetworkKey, String)>,
    pub active_networks: u32,
}

pub(crate) fn connected(ifaces: Vec<NetInterface>, settings: &Settings) -> Vec<Connected> {
    ifaces
        .into_iter()
        .filter(|i| {
            i.class.usable_by_default()
                || (matches!(i.class, InterfaceClass::Virtual | InterfaceClass::Vpn)
                    && settings.networks.include_interfaces.contains(&i.name))
        })
        .map(|iface| Connected {
            id: network_id(&iface),
            iface,
        })
        .collect()
}

/// Applies trust, "trust every network" and the network menu to what is connected.
pub(crate) fn plan(settings: &Settings, connected: &[Connected]) -> Plan {
    let mut p = Plan::default();
    let mut seen = HashSet::new();
    let mut active_keys = HashSet::new();
    for c in connected {
        let known = settings.network(&c.id.key);
        let added_by_hand = !c.iface.class.usable_by_default();
        let trusted = added_by_hand
            || settings.networks.trust_every_network
            || known.is_some_and(|n| n.trust == NetworkTrust::Trusted);
        let selected = match &settings.networks.selection {
            NetworkSelection::TrustedNetworks => true,
            NetworkSelection::Only(k) => k == &c.id.key,
        };
        let active = trusted && selected;
        if active {
            p.active.extend(ActiveInterface::from_interface(&c.iface));
            active_keys.insert(c.id.key);
        }
        if !seen.insert(c.id.key) {
            continue; // same network on two interfaces: one row
        }
        let trust = known.map_or(NetworkTrust::NotTrusted, |n| n.trust);
        if !trusted
            && trust == NetworkTrust::NotTrusted
            && settings.networks.on_new_network == OnNewNetwork::AskToTrust
        {
            p.ask.push((c.id.key, label(settings, c)));
        }
        p.views.push(NetworkView {
            key: hex(&c.id.key),
            label: label(settings, c),
            kind: c.id.kind,
            address: c
                .iface
                .subnets
                .first()
                .map(|s| format!("{}/{}", s.addr, s.prefix)),
            last_seen_unix: known.map_or(0, |n| n.last_seen_unix),
            connected: true,
            active,
            trust: if trusted {
                NetworkTrust::Trusted
            } else {
                trust
            },
            weak_identity: c.id.weak,
        });
    }
    for n in &settings.known_networks {
        if !seen.contains(&n.key) {
            p.views.push(NetworkView {
                key: hex(&n.key),
                label: n.label.clone(),
                kind: n.kind,
                address: None,
                last_seen_unix: n.last_seen_unix,
                connected: false,
                active: false,
                trust: n.trust,
                weak_identity: n.weak,
            });
        }
    }
    p.active_networks = active_keys.len() as u32;
    p
}

fn label(settings: &Settings, c: &Connected) -> String {
    settings
        .network(&c.id.key)
        .map_or_else(|| c.id.default_label.clone(), |n| n.label.clone())
}

pub(crate) fn hex(k: &NetworkKey) -> String {
    k.0.iter().map(|b| format!("{b:02x}")).collect()
}

/// Matches a full or shortened hex key (at least 4 characters).
pub(crate) fn key_matches(k: &NetworkKey, query: &str) -> bool {
    let q = query.trim().to_ascii_lowercase();
    q.len() >= 4 && hex(k).starts_with(&q)
}

type Lister = Box<dyn Fn() -> Vec<NetInterface> + Send + Sync>;

/// State shared by the node and its background tasks.
pub(crate) struct Shared {
    pub settings: Mutex<Settings>,
    pub state_dir: Option<StateDir>,
    pub events: UnboundedSender<NodeEvent>,
    pub device_id: DeviceId,
    pub identity: DeviceIdentity,
    pub key: GroupKey,
    pub store: Arc<dyn Store>,
    pub discovery_on: AtomicBool,
    pub lister: Lister,
    /// Discovered group members and their announced addresses.
    pub peers: Mutex<HashMap<DeviceId, Vec<SocketAddr>>>,
    /// Authenticated connections, one per device.
    pub sessions: Mutex<HashMap<DeviceId, Arc<Session>>>,
    /// Devices being dialed now, and when each was last tried.
    pub dialing: Mutex<HashMap<DeviceId, std::time::Instant>>,
    /// Subnets discovery runs on; incoming connections must come from one of them.
    pub active_subnets: Arc<std::sync::RwLock<Vec<Subnet>>>,
    pub endpoint: Mutex<Option<Endpoint>>,
    run: tokio::sync::Mutex<Runtime>,
    asked: Mutex<HashSet<NetworkKey>>,
}

#[derive(Default)]
struct Runtime {
    started: bool,
    port: u16,
    service: Option<DiscoveryService>,
    disc_tx: Option<UnboundedSender<DiscoveryEvent>>,
    tasks: Vec<JoinHandle<()>>,
    last_active: Vec<ActiveInterface>,
    last_reported: Option<(bool, u32)>,
    announce: bool,
}

impl Shared {
    pub fn new(
        settings: Settings,
        state_dir: Option<StateDir>,
        events: UnboundedSender<NodeEvent>,
        identity: DeviceIdentity,
        key: GroupKey,
        store: Arc<dyn Store>,
    ) -> Self {
        Self {
            settings: Mutex::new(settings),
            state_dir,
            events,
            device_id: identity.device_id(),
            identity,
            key,
            store,
            discovery_on: AtomicBool::new(false),
            lister: Box::new(list_interfaces),
            peers: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            dialing: Mutex::new(HashMap::new()),
            active_subnets: Arc::new(std::sync::RwLock::new(Vec::new())),
            endpoint: Mutex::new(None),
            run: tokio::sync::Mutex::new(Runtime::default()),
            asked: Mutex::new(HashSet::new()),
        }
    }

    #[cfg(test)]
    pub fn with_lister(
        mut self,
        f: impl Fn() -> Vec<NetInterface> + Send + Sync + 'static,
    ) -> Self {
        self.lister = Box::new(f);
        self
    }

    pub fn emit(&self, e: NodeEvent) {
        let _ = self.events.send(e);
    }

    pub fn save(&self) -> localdrop_core::Result<()> {
        if let Some(dir) = &self.state_dir {
            dir.save_settings(&self.settings.lock().expect("poisoned"))?;
        }
        Ok(())
    }

    pub fn current_plan(&self) -> (Plan, Vec<Connected>) {
        let settings = self.settings.lock().expect("poisoned").clone();
        let conns = connected((self.lister)(), &settings);
        (plan(&settings, &conns), conns)
    }

    /// Starts the background tasks (idempotent).
    pub async fn start(self: &Arc<Self>) {
        let mut run = self.run.lock().await;
        if run.started {
            return;
        }
        run.started = true;
        let settings = self.settings.lock().expect("poisoned").clone();
        run.port = if settings.listen_port != 0 {
            settings.listen_port
        } else {
            rand::Rng::gen_range(&mut rand::thread_rng(), 49_152..=65_535)
        };
        run.announce = settings.visible;
        self.discovery_on
            .store(settings.networks.discover_at_launch, Ordering::SeqCst);
        match self.bind_endpoint(run.port, settings.listen_port != 0) {
            Ok(port) => run.port = port,
            Err(e) => tracing::warn!("no QUIC endpoint, only discovery runs: {e}"),
        }

        let (tx, mut rx) = mpsc::unbounded_channel::<DiscoveryEvent>();
        run.disc_tx = Some(tx);
        let me = Arc::clone(self);
        run.tasks.push(tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                me.on_discovery(ev);
            }
        }));
        let me = Arc::clone(self);
        run.tasks
            .push(tokio::spawn(async move { me.watch().await }));
        let me = Arc::clone(self);
        run.tasks
            .push(tokio::spawn(async move { me.accept_loop().await }));
        drop(run);
        self.reconcile().await;
        self.dial_remembered();
    }

    /// Stops discovery and the background tasks.
    pub async fn stop(&self) {
        let mut run = self.run.lock().await;
        for t in run.tasks.drain(..) {
            t.abort();
        }
        self.close_sessions();
        if let Some(ep) = self.endpoint.lock().expect("poisoned").take() {
            ep.close();
        }
        if let Some(s) = run.service.take() {
            s.stop().await;
        }
        *run = Runtime::default();
        let lost: Vec<_> = self
            .peers
            .lock()
            .expect("poisoned")
            .drain()
            .map(|(id, _)| id)
            .collect();
        for device in lost {
            self.emit(NodeEvent::PeerOffline { device });
        }
    }

    /// Brings discovery in line with the switch, the trust rules and what is connected.
    pub async fn reconcile(&self) {
        let (plan, _) = self.current_plan();
        let on = self.discovery_on.load(Ordering::SeqCst);
        {
            let mut asked = self.asked.lock().expect("poisoned");
            for (key, label) in &plan.ask {
                if asked.insert(*key) {
                    self.emit(NodeEvent::UntrustedNetwork {
                        key: hex(key),
                        label: label.clone(),
                    });
                }
            }
        }

        let visible = self.settings.lock().expect("poisoned").visible;
        let mut run = self.run.lock().await;
        if !run.started {
            return;
        }
        let want = if on { plan.active.clone() } else { Vec::new() };
        *self.active_subnets.write().expect("poisoned") = want.iter().map(|a| a.subnet).collect();
        if want.is_empty() {
            // Discovery off or no trusted network: nothing is sent or received.
            self.close_sessions();
            if let Some(s) = run.service.take() {
                s.stop().await;
            }
            let lost: Vec<_> = self
                .peers
                .lock()
                .expect("poisoned")
                .drain()
                .map(|(id, _)| id)
                .collect();
            for device in lost {
                self.emit(NodeEvent::PeerOffline { device });
            }
        } else if let Some(s) = &run.service {
            if run.last_active != want {
                s.set_interfaces(want.clone());
            }
            if run.announce != visible {
                s.set_announce(visible);
            }
        } else {
            let mut cfg =
                ServiceConfig::new(self.device_id, self.key.clone(), run.port, want.clone());
            cfg.announce = visible;
            let tx = run.disc_tx.clone().expect("started");
            run.service = Some(DiscoveryService::spawn(cfg, tx));
        }
        run.last_active = want;
        run.announce = visible;
        let report = (on, if on { plan.active_networks } else { 0 });
        if run.last_reported != Some(report) {
            run.last_reported = Some(report);
            self.emit(NodeEvent::DiscoveryChanged {
                on: report.0,
                active_networks: report.1,
            });
        }
    }

    fn on_discovery(self: &Arc<Self>, ev: DiscoveryEvent) {
        match ev {
            DiscoveryEvent::Found(p) => {
                let new = self
                    .peers
                    .lock()
                    .expect("poisoned")
                    .insert(p.device_id, p.addrs)
                    .is_none();
                if new && !self.is_connected(&p.device_id) {
                    tracing::info!(device = %p.device_id, "peer found");
                    self.emit(NodeEvent::PeerFound {
                        device: p.device_id,
                    });
                }
                self.maybe_dial(p.device_id);
            }
            DiscoveryEvent::Lost(device) => {
                if self
                    .peers
                    .lock()
                    .expect("poisoned")
                    .remove(&device)
                    .is_some()
                    && !self.is_connected(&device)
                {
                    // With a live connection the peer stays online: keep-alives decide.
                    tracing::info!(%device, "peer lost");
                    self.emit(NodeEvent::PeerOffline { device });
                }
            }
        }
    }

    /// Polls interfaces; on a stable change, restores the switch to its launch setting
    /// (a manual change lasts until the next network change) and reconciles.
    async fn watch(self: Arc<Self>) {
        let signature = |v: Vec<NetInterface>| {
            let mut s: Vec<_> = v
                .into_iter()
                .map(|i| (i.name, i.subnets, i.gateway_ip, i.gateway_mac, i.ssid))
                .collect();
            s.sort_by(|a, b| a.0.cmp(&b.0));
            s
        };
        let mut last = signature((self.lister)());
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            let now = signature((self.lister)());
            if now == last {
                continue;
            }
            tokio::time::sleep(POLL_INTERVAL).await;
            let settled = signature((self.lister)());
            if settled != now {
                continue; // still changing; look again next round
            }
            tracing::info!("network changed");
            last = settled;
            let at_launch = self
                .settings
                .lock()
                .expect("poisoned")
                .networks
                .discover_at_launch;
            self.discovery_on.store(at_launch, Ordering::SeqCst);
            // A new network rotates the announcement too: the service restarts.
            if let Some(s) = &self.run.lock().await.service {
                s.rotate();
            }
            self.reconcile().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use localdrop_discovery::{NetworkKind, Subnet};

    use super::*;

    fn iface(name: &str, class: InterfaceClass, third: u8, mac: u8) -> NetInterface {
        NetInterface {
            name: name.into(),
            label: name.into(),
            class,
            subnets: vec![Subnet {
                addr: Ipv4Addr::new(192, 168, third, 20),
                prefix: 24,
            }],
            gateway_ip: Some(Ipv4Addr::new(192, 168, third, 1)),
            gateway_mac: Some([mac; 6]),
            ssid: None,
        }
    }

    fn setup() -> (Settings, Vec<Connected>) {
        let s = Settings::new("pc", "home");
        let ifaces = vec![
            iface("wifi", InterfaceClass::WiFi, 1, 1),
            iface("eth", InterfaceClass::Ethernet, 2, 2),
            iface("vEthernet", InterfaceClass::Virtual, 3, 3),
            iface("rmnet", InterfaceClass::Cellular, 4, 4),
        ];
        let c = connected(ifaces, &s);
        (s, c)
    }

    #[test]
    fn nothing_trusted_by_default_and_each_new_network_is_asked() {
        let (s, c) = setup();
        assert_eq!(c.len(), 2, "virtual and cellular are left out");
        let p = plan(&s, &c);
        assert!(p.active.is_empty());
        assert_eq!(p.active_networks, 0);
        assert_eq!(p.ask.len(), 2);
        assert!(p.views.iter().all(|v| v.connected && !v.active));
    }

    #[test]
    fn trust_selection_and_never_ask() {
        let (mut s, c) = setup();
        let wifi = c[0].id.key;
        let eth = c[1].id.key;
        s.set_trust(
            wifi,
            "Home",
            NetworkKind::WiFi,
            false,
            NetworkTrust::Trusted,
        );
        s.set_trust(
            eth,
            "Office",
            NetworkKind::Ethernet,
            false,
            NetworkTrust::NeverAsk,
        );
        let p = plan(&s, &c);
        assert_eq!(p.active.len(), 1);
        assert_eq!(p.active[0].name, "wifi");
        assert!(p.ask.is_empty());
        assert_eq!(p.views[0].label, "Home");
        assert_eq!(p.views[0].kind, NetworkKind::WiFi);
        assert!(p.views[0].address.as_deref().unwrap().ends_with("/24"));
        assert!(p.views[0].last_seen_unix > 0);

        s.networks.trust_every_network = true;
        assert_eq!(plan(&s, &c).active_networks, 2);

        s.networks.selection = NetworkSelection::Only(eth);
        let p = plan(&s, &c);
        assert_eq!(p.active.len(), 1);
        assert_eq!(p.active[0].name, "eth");
    }

    #[test]
    fn virtual_interfaces_only_by_hand_and_forgotten_networks_listed() {
        let (mut s, _) = setup();
        s.networks.include_interfaces.push("vEthernet".into());
        s.set_trust(
            NetworkKey([9; 32]),
            "Cafe",
            NetworkKind::WiFi,
            true,
            NetworkTrust::NotTrusted,
        );
        let ifaces = vec![
            iface("wifi", InterfaceClass::WiFi, 1, 1),
            iface("vEthernet", InterfaceClass::Virtual, 3, 3),
        ];
        let c = connected(ifaces, &s);
        let p = plan(&s, &c);
        assert_eq!(p.active.len(), 1, "a hand-added adapter is active");
        assert_eq!(p.active[0].name, "vEthernet");
        let cafe = p.views.iter().find(|v| v.label == "Cafe").unwrap();
        assert!(!cafe.connected && cafe.weak_identity);
        assert_eq!(cafe.address, None);
        assert!(key_matches(&NetworkKey([9; 32]), "0909"));
        assert!(!key_matches(&NetworkKey([9; 32]), "09"));
    }

    #[tokio::test]
    async fn runtime_follows_trust_and_switch() {
        use localdrop_core::{DeviceIdentity, GroupConfig, KdfParams};
        let key = GroupKey::derive(
            &GroupConfig::new("home").unwrap(),
            "pw",
            KdfParams::INSECURE_TEST,
        )
        .unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut settings = Settings::new("pc", "home");
        // Loopback stands in for a trusted LAN (no packets leave the machine).
        let lo = NetInterface {
            name: "lo-test".into(),
            label: "lo-test".into(),
            class: InterfaceClass::Ethernet,
            subnets: vec![Subnet {
                addr: Ipv4Addr::LOCALHOST,
                prefix: 8,
            }],
            gateway_ip: None,
            gateway_mac: None,
            ssid: None,
        };
        let id = network_id(&lo);
        let shared = Arc::new(
            Shared::new(
                settings.clone(),
                None,
                tx,
                DeviceIdentity::generate(),
                key,
                Arc::new(localdrop_store::MemoryStore::new()),
            )
            .with_lister(move || vec![lo.clone()]),
        );
        shared.start().await;
        let mut got = Vec::new();
        while let Ok(e) = rx.try_recv() {
            got.push(e);
        }
        assert!(got.contains(&NodeEvent::UntrustedNetwork {
            key: hex(&id.key),
            label: "Ethernet 127.0.0.x".into()
        }));
        assert!(got.contains(&NodeEvent::DiscoveryChanged {
            on: true,
            active_networks: 0
        }));

        settings.set_trust(
            id.key,
            "lo",
            NetworkKind::Ethernet,
            true,
            NetworkTrust::Trusted,
        );
        *shared.settings.lock().unwrap() = settings;
        shared.reconcile().await;
        assert_eq!(
            rx.try_recv().unwrap(),
            NodeEvent::DiscoveryChanged {
                on: true,
                active_networks: 1
            }
        );

        shared.discovery_on.store(false, Ordering::SeqCst);
        shared.reconcile().await;
        assert_eq!(
            rx.try_recv().unwrap(),
            NodeEvent::DiscoveryChanged {
                on: false,
                active_networks: 0
            }
        );
        shared.stop().await;
    }
}
