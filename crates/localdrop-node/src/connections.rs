//! Authenticated connections to group members (M2): the QUIC endpoint, dialing
//! discovered peers, one session per device, key pinning and presence events.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use localdrop_core::{DeviceId, DeviceIdentity, Error, Result};
use localdrop_store::KnownDevice;
use localdrop_transport::{DEFAULT_FIXED_PORT, Endpoint, Session, code};

use crate::NodeEvent;
use crate::network::Shared;

/// The device with the larger id waits this long before dialing, so that normally only
/// the smaller one dials and no duplicate connection is made.
const DIAL_DELAY_HIGHER: Duration = Duration::from_secs(3);
/// Minimum time between two dial attempts to the same device.
const REDIAL_AFTER: Duration = Duration::from_secs(10);
/// After a failed handshake (the device has another password, e.g. after a password
/// change) wait longer: every attempt costs it one of the few tries the other side allows
/// per minute.
const REDIAL_AFTER_AUTH_FAILED: Duration = Duration::from_secs(120);

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Parses `host`, `host:port` or `[v6]:port`; without a port, [`DEFAULT_FIXED_PORT`].
pub(crate) async fn resolve(address: &str) -> Result<SocketAddr> {
    let a = address.trim();
    if a.is_empty() {
        return Err(Error::Transport("enter an address".into()));
    }
    if let Ok(sa) = a.parse::<SocketAddr>() {
        return Ok(sa);
    }
    if let Ok(ip) = a.parse::<std::net::IpAddr>() {
        return Ok(SocketAddr::new(ip, DEFAULT_FIXED_PORT));
    }
    let with_port = if a
        .rsplit_once(':')
        .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
    {
        a.to_owned()
    } else {
        format!("{a}:{DEFAULT_FIXED_PORT}")
    };
    tokio::net::lookup_host(with_port)
        .await
        .map_err(|e| Error::Transport(format!("can't find {a}: {e}")))?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| Error::Transport(format!("{a} has no IPv4 address")))
}

impl Shared {
    /// Binds the endpoint on `port`; if that is taken and the port isn't fixed, on
    /// another random one. Returns the port in use.
    pub(crate) fn bind_endpoint(self: &Arc<Self>, port: u16, fixed: bool) -> Result<u16> {
        let name = self.settings.lock().expect("poisoned").device_name.clone();
        let identity = DeviceIdentity::from_secret_bytes(&self.identity.secret_bytes());
        let mut tries = vec![port];
        if !fixed {
            tries.extend(
                (0..4).map(|_| rand::Rng::gen_range(&mut rand::thread_rng(), 49_152..=65_535)),
            );
        }
        let mut last = None;
        for p in tries {
            match Endpoint::bind(
                SocketAddr::from((Ipv4Addr::UNSPECIFIED, p)),
                &identity,
                self.group_key(),
                name.clone(),
            ) {
                Ok(ep) => {
                    let subnets = Arc::clone(&self.active_subnets);
                    // Only trusted networks: the same rule as discovery.
                    ep.set_allow(move |ip| match ip {
                        std::net::IpAddr::V4(v4) => subnets
                            .read()
                            .expect("poisoned")
                            .iter()
                            .any(|s| s.contains(v4)),
                        std::net::IpAddr::V6(_) => false,
                    });
                    let port = ep.local_addr()?.port();
                    tracing::info!(port, "QUIC endpoint listening");
                    *self.endpoint.lock().expect("poisoned") = Some(ep);
                    return Ok(port);
                }
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or(Error::Transport("no port".into())))
    }

    fn endpoint(&self) -> Option<Endpoint> {
        self.endpoint.lock().expect("poisoned").clone()
    }

    pub(crate) fn is_connected(&self, device: &DeviceId) -> bool {
        self.sessions
            .lock()
            .expect("poisoned")
            .get(device)
            .is_some_and(|s| !s.is_closed())
    }

    pub(crate) fn session(&self, device: &DeviceId) -> Option<Arc<Session>> {
        self.sessions
            .lock()
            .expect("poisoned")
            .get(device)
            .filter(|s| !s.is_closed())
            .cloned()
    }

    /// Accepts incoming connections until the endpoint closes.
    pub(crate) async fn accept_loop(self: Arc<Self>) {
        let Some(ep) = self.endpoint() else { return };
        while let Some(incoming) = ep.accept().await {
            let me = Arc::clone(&self);
            tokio::spawn(async move {
                let from = incoming.remote_address();
                match incoming.establish().await {
                    Ok(s) => me.on_session(s),
                    Err(e) => me.on_connect_error(None, from, e),
                }
            });
        }
    }

    /// Dials a discovered peer unless connected, already dialing, or tried recently.
    pub(crate) fn maybe_dial(self: &Arc<Self>, device: DeviceId) {
        if device == self.device_id || self.is_connected(&device) || self.endpoint().is_none() {
            return;
        }
        {
            let mut dialing = self.dialing.lock().expect("poisoned");
            if dialing
                .get(&device)
                .is_some_and(|t| t.elapsed() < REDIAL_AFTER)
            {
                return;
            }
            dialing.insert(device, Instant::now());
        }
        let me = Arc::clone(self);
        tokio::spawn(async move {
            if me.device_id > device {
                tokio::time::sleep(DIAL_DELAY_HIGHER).await;
            }
            me.dial(device).await;
        });
    }

    async fn dial(self: &Arc<Self>, device: DeviceId) {
        if self.is_connected(&device) {
            return;
        }
        let Some(ep) = self.endpoint() else { return };
        let addrs = self
            .peers
            .lock()
            .expect("poisoned")
            .get(&device)
            .cloned()
            .unwrap_or_default();
        for addr in addrs {
            match ep.connect(addr, Some(device)).await {
                Ok(s) => {
                    self.on_session(s);
                    return;
                }
                Err(Error::AlreadyConnected(_)) => return,
                Err(Error::AuthFailed) => {
                    // Back off: `maybe_dial` waits until REDIAL_AFTER has passed since
                    // this (future) instant.
                    let until = Instant::now() + REDIAL_AFTER_AUTH_FAILED - REDIAL_AFTER;
                    self.dialing.lock().expect("poisoned").insert(device, until);
                    self.on_connect_error(Some(device), addr, Error::AuthFailed);
                    return;
                }
                Err(e) => {
                    let stop = matches!(e, Error::KeyMismatch(_));
                    self.on_connect_error(Some(device), addr, e);
                    if stop {
                        return;
                    }
                }
            }
        }
    }

    /// *Connect by address*.
    pub(crate) async fn connect_address(self: &Arc<Self>, address: &str) -> Result<DeviceId> {
        let ep = self
            .endpoint()
            .ok_or(Error::Transport("LocalDrop isn't running".into()))?;
        let addr = resolve(address).await?;
        match ep.connect(addr, None).await {
            Ok(s) => {
                let id = s.peer().device_id;
                self.on_session(s);
                Ok(id)
            }
            // The device kept the connection we already have to it.
            Err(Error::AlreadyConnected(id)) if self.is_connected(&id) => Ok(id),
            Err(e) => Err(e),
        }
    }

    /// Tries the remembered *Connect by address* entries once, in the background.
    pub(crate) fn dial_remembered(self: &Arc<Self>) {
        let list = self
            .settings
            .lock()
            .expect("poisoned")
            .remembered_addresses
            .clone();
        for a in list {
            let me = Arc::clone(self);
            tokio::spawn(async move {
                if let Err(e) = me.connect_address(&a).await {
                    tracing::info!(address = %a, "remembered address: {e}");
                }
            });
        }
    }

    fn on_connect_error(&self, expected: Option<DeviceId>, addr: SocketAddr, e: Error) {
        match e {
            Error::KeyMismatch(device) => {
                tracing::warn!(%device, %addr, "refused: key does not match the device");
                let name = self.store.device(&device).ok().flatten().map_or_else(
                    || format!("Device {}", &device.to_string()[..8]),
                    |d| d.name,
                );
                self.emit(NodeEvent::KeyMismatch { device, name });
            }
            Error::AuthFailed => {
                tracing::warn!(%addr, "handshake failed: not in this group");
            }
            e => tracing::debug!(?expected, %addr, "connection failed: {e}"),
        }
    }

    /// A new authenticated session: pin the key, keep one session per device, report.
    pub(crate) fn on_session(self: &Arc<Self>, s: Arc<Session>) {
        let peer = s.peer().clone();
        let id = peer.device_id;
        if id == self.device_id {
            s.close(code::DUPLICATE, "that's me");
            return;
        }
        // Key pinning: the id is derived from the key, so a different key under a known
        // id can't pass TLS; check anyway in case the store holds something odd.
        let known = self.store.device(&id).ok().flatten();
        if known
            .as_ref()
            .is_some_and(|d| d.public_key != peer.public_key)
        {
            s.close(code::KEY_MISMATCH, "key mismatch");
            self.on_connect_error(Some(id), peer.addr, Error::KeyMismatch(id));
            return;
        }
        let replaced = {
            let mut sessions = self.sessions.lock().expect("poisoned");
            match sessions.get(&id) {
                Some(old) if !old.is_closed() => {
                    // Both sides dialed: keep the connection opened by the smaller id.
                    let lower = self.device_id.min(id);
                    if s.initiator() != lower || old.initiator() == lower {
                        s.close_duplicate(self.device_id);
                        return;
                    }
                    old.close_duplicate(self.device_id);
                    sessions.insert(id, Arc::clone(&s));
                    true
                }
                _ => {
                    sessions.insert(id, Arc::clone(&s));
                    false
                }
            }
        };
        self.dialing.lock().expect("poisoned").remove(&id);
        let _ = self.store.upsert_device(KnownDevice {
            id,
            public_key: peer.public_key,
            name: peer.name.clone(),
            last_seen_unix: now_unix(),
            accept_override: known.and_then(|d| d.accept_override),
        });
        if !replaced {
            tracing::info!(device = %id, name = %peer.name, addr = %peer.addr, "peer online");
            self.emit(NodeEvent::PeerOnline {
                device: id,
                name: peer.name,
            });
        }

        // Control messages beyond Ping/Pong belong to the transfer engine (M3).
        if let Some(mut rx) = s.take_incoming() {
            tokio::spawn(async move {
                while let Some(m) = rx.recv().await {
                    tracing::debug!(device = %id, "control message not handled yet: {m:?}");
                }
            });
        }
        let me = Arc::clone(self);
        tokio::spawn(async move {
            let why = s.closed().await;
            let removed = {
                let mut sessions = me.sessions.lock().expect("poisoned");
                if sessions
                    .get(&id)
                    .is_some_and(|x| x.stable_id() == s.stable_id())
                {
                    sessions.remove(&id);
                    true
                } else {
                    false
                }
            };
            if !removed {
                return;
            }
            tracing::info!(device = %id, "connection closed: {why}");
            if let Ok(Some(mut d)) = me.store.device(&id) {
                d.last_seen_unix = now_unix();
                let _ = me.store.upsert_device(d);
            }
            if me.peers.lock().expect("poisoned").contains_key(&id) {
                me.emit(NodeEvent::PeerFound { device: id });
                me.dialing.lock().expect("poisoned").remove(&id);
                me.maybe_dial(id);
            } else {
                me.emit(NodeEvent::PeerOffline { device: id });
            }
        });
    }

    /// Closes every session (discovery off, no trusted network, stop). Devices that
    /// aren't discovered either are reported offline here; the rest by the caller.
    pub(crate) fn close_sessions(&self) {
        let all: Vec<_> = self.sessions.lock().expect("poisoned").drain().collect();
        let peers = self.peers.lock().expect("poisoned");
        for (id, s) in all {
            s.close(code::BYE, "discovery off");
            if !peers.contains_key(&id) {
                self.emit(NodeEvent::PeerOffline { device: id });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use localdrop_core::{GroupConfig, GroupKey, KdfParams};
    use localdrop_discovery::net::network_id;
    use localdrop_discovery::{InterfaceClass, NetInterface, NetworkKind, Subnet};
    use tokio::sync::mpsc;

    use super::*;
    use crate::api::NetworkTrust;
    use crate::settings::Settings;

    fn key(pw: &str) -> GroupKey {
        GroupKey::derive(
            &GroupConfig::new("home").unwrap(),
            pw,
            KdfParams::INSECURE_TEST,
        )
        .unwrap()
    }

    /// A node whose only network is loopback, trusted.
    fn node(pw: &str, name: &str) -> (Arc<Shared>, mpsc::UnboundedReceiver<NodeEvent>) {
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
        let mut settings = Settings::new(name, "home");
        settings.set_trust(
            network_id(&lo).key,
            "lo",
            NetworkKind::Ethernet,
            true,
            NetworkTrust::Trusted,
        );
        // Don't announce: the test connects by address, discovery would race with it.
        settings.visible = false;
        let (tx, rx) = mpsc::unbounded_channel();
        let shared = Arc::new(
            Shared::new(
                settings,
                None,
                tx,
                DeviceIdentity::generate(),
                key(pw),
                Arc::new(localdrop_store::MemoryStore::new()),
            )
            .with_lister(move || vec![lo.clone()]),
        );
        (shared, rx)
    }

    async fn next(
        rx: &mut mpsc::UnboundedReceiver<NodeEvent>,
        f: impl Fn(&NodeEvent) -> bool,
    ) -> NodeEvent {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let e = rx.recv().await.expect("events");
                if f(&e) {
                    return e;
                }
            }
        })
        .await
        .expect("event in time")
    }

    fn port(s: &Shared) -> u16 {
        s.endpoint().unwrap().local_addr().unwrap().port()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn members_connect_ping_and_go_offline_with_discovery() {
        let (a, mut ea) = node("pw", "Laptop");
        let (b, mut eb) = node("pw", "Desk PC");
        a.start().await;
        b.start().await;

        let id = a
            .connect_address(&format!("127.0.0.1:{}", port(&b)))
            .await
            .unwrap();
        assert_eq!(id, b.device_id);
        let e = next(&mut ea, |e| matches!(e, NodeEvent::PeerOnline { .. })).await;
        assert_eq!(
            e,
            NodeEvent::PeerOnline {
                device: b.device_id,
                name: "Desk PC".into()
            }
        );
        next(&mut eb, |e| {
            matches!(e, NodeEvent::PeerOnline { device, name } if *device == a.device_id && name == "Laptop")
        })
        .await;
        a.session(&b.device_id).unwrap().ping().await.unwrap();

        // The key is pinned in the store.
        let known = a.store.device(&b.device_id).unwrap().unwrap();
        assert_eq!(known.public_key, b.identity.public_key());

        // A second connection to the same device is not kept twice.
        b.connect_address(&format!("127.0.0.1:{}", port(&a)))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(a.sessions.lock().unwrap().len(), 1);
        assert!(a.is_connected(&b.device_id) && b.is_connected(&a.device_id));

        // Discovery off on B: nothing is sent or received, A sees B go offline.
        b.discovery_on.store(false, Ordering::SeqCst);
        b.reconcile().await;
        next(
            &mut ea,
            |e| matches!(e, NodeEvent::PeerOffline { device } if *device == b.device_id),
        )
        .await;
        // And B no longer accepts connections.
        assert!(
            a.connect_address(&format!("127.0.0.1:{}", port(&b)))
                .await
                .is_err()
        );
        a.stop().await;
        b.stop().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn after_a_password_change_only_the_new_password_gets_in() {
        let (a, mut ea) = node("old-password", "Laptop");
        let (b, _eb) = node("old-password", "Desk PC");
        a.start().await;
        b.start().await;
        let addr = format!("127.0.0.1:{}", port(&a));
        b.connect_address(&addr).await.unwrap();
        next(&mut ea, |e| matches!(e, NodeEvent::PeerOnline { .. })).await;

        a.change_key(key("new-password")).await;
        next(
            &mut ea,
            |e| matches!(e, NodeEvent::PeerOffline { device } if *device == b.device_id),
        )
        .await;
        let err = b.connect_address(&addr).await.unwrap_err();
        assert!(matches!(err, Error::AuthFailed), "{err:?}");

        b.change_key(key("new-password")).await;
        b.connect_address(&addr).await.unwrap();
        next(
            &mut ea,
            |e| matches!(e, NodeEvent::PeerOnline { device, .. } if *device == b.device_id),
        )
        .await;
        a.stop().await;
        b.stop().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wrong_password_never_comes_online() {
        let (a, mut ea) = node("right", "Laptop");
        let (b, _eb) = node("wrong", "Stranger");
        a.start().await;
        b.start().await;
        let err = b
            .connect_address(&format!("127.0.0.1:{}", port(&a)))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::AuthFailed), "{err:?}");
        tokio::time::sleep(Duration::from_millis(200)).await;
        while let Ok(e) = ea.try_recv() {
            assert!(!matches!(e, NodeEvent::PeerOnline { .. }), "{e:?}");
        }
        a.stop().await;
        b.stop().await;
    }

    #[tokio::test]
    async fn addresses() {
        assert_eq!(
            resolve("192.168.1.42").await.unwrap(),
            "192.168.1.42:47474".parse().unwrap()
        );
        assert_eq!(
            resolve(" 10.0.0.2:5000 ").await.unwrap(),
            "10.0.0.2:5000".parse().unwrap()
        );
        assert_eq!(resolve("localhost:6000").await.unwrap().port(), 6000);
        assert!(resolve("").await.is_err());
    }
}
