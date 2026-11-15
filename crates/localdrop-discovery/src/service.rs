//! Runs the backends, merges what they find by `DeviceId` and decides when a peer is
//! lost (see `docs/discovery.md#peer-lifecycle`).

use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use localdrop_core::{DeviceId, GroupKey};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

use crate::broadcast::ANNOUNCE_INTERVAL;
use crate::{
    ANNOUNCE_ROTATION_SECS, ActiveInterface, Announcement, BackendConfig, BroadcastDiscovery,
    DiscoveredPeer, Discovery, DiscoveryEvent, DiscoverySource, MdnsDiscovery,
};

/// A broadcast-only peer is lost after this long without an announcement.
pub const LOST_AFTER: Duration = Duration::from_secs(3 * ANNOUNCE_INTERVAL.as_secs());

/// Settings of a [`DiscoveryService`].
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Our device id (our own announcements are ignored).
    pub device_id: DeviceId,
    /// Group key: seals our announcement and opens others'.
    pub key: GroupKey,
    /// Port we announce (the QUIC listener).
    pub port: u16,
    /// Active interfaces.
    pub interfaces: Vec<ActiveInterface>,
    /// Announce ourselves, or only browse ("Visible to your group" off).
    pub announce: bool,
    /// Run the mDNS backend.
    pub mdns: bool,
    /// Run the broadcast backend; `Some(port)` overrides the port (tests).
    pub broadcast: Option<u16>,
}

impl ServiceConfig {
    /// Both backends on their standard ports.
    pub fn new(
        device_id: DeviceId,
        key: GroupKey,
        port: u16,
        interfaces: Vec<ActiveInterface>,
    ) -> Self {
        Self {
            device_id,
            key,
            port,
            interfaces,
            announce: true,
            mdns: true,
            broadcast: Some(crate::BROADCAST_PORT),
        }
    }
}

enum Command {
    SetInterfaces(Vec<ActiveInterface>),
    SetAnnounce(bool),
    Rotate,
}

/// Handle to the running discovery task. Dropping it stops discovery.
#[derive(Debug)]
pub struct DiscoveryService {
    commands: UnboundedSender<Command>,
    task: JoinHandle<()>,
}

impl DiscoveryService {
    /// Starts discovery; events go to `out`. Must be called inside a tokio runtime.
    pub fn spawn(cfg: ServiceConfig, out: UnboundedSender<DiscoveryEvent>) -> Self {
        let (commands, rx) = mpsc::unbounded_channel();
        let task = tokio::spawn(run(cfg, rx, out));
        Self { commands, task }
    }

    /// Restarts on another set of interfaces (network change). Peers only reachable
    /// through removed interfaces are reported lost.
    pub fn set_interfaces(&self, interfaces: Vec<ActiveInterface>) {
        let _ = self.commands.send(Command::SetInterfaces(interfaces));
    }

    /// Switches announcing on or off; browsing continues.
    pub fn set_announce(&self, announce: bool) {
        let _ = self.commands.send(Command::SetAnnounce(announce));
    }

    /// Generates a new announcement now (also done every 15 minutes).
    pub fn rotate(&self) {
        let _ = self.commands.send(Command::Rotate);
    }

    /// Stops discovery and its backends (sends mDNS goodbyes).
    pub async fn stop(self) {
        drop(self.commands);
        let _ = self.task.await;
    }
}

#[derive(Default)]
struct Peer {
    broadcast: Option<(Vec<SocketAddr>, Instant)>,
    mdns: Option<Vec<SocketAddr>>,
    reported: BTreeSet<SocketAddr>,
}

impl Peer {
    fn addrs(&self) -> BTreeSet<SocketAddr> {
        self.broadcast
            .iter()
            .flat_map(|(a, _)| a.iter())
            .chain(self.mdns.iter().flatten())
            .copied()
            .collect()
    }
}

struct State {
    cfg: ServiceConfig,
    backends: Vec<Box<dyn Discovery>>,
    backend_tx: UnboundedSender<DiscoveryEvent>,
    peers: HashMap<DeviceId, Peer>,
    out: UnboundedSender<DiscoveryEvent>,
}

impl State {
    fn restart(&mut self) {
        for b in &mut self.backends {
            b.stop();
        }
        self.backends.clear();
        if self.cfg.interfaces.is_empty() {
            return; // no active network: send nothing, listen to nothing
        }
        let bcfg = BackendConfig {
            device_id: self.cfg.device_id,
            key: self.cfg.key.clone(),
            port: self.cfg.port,
            interfaces: self.cfg.interfaces.clone(),
            announce: self.cfg.announce,
            announcement: Announcement::seal(&self.cfg.key, &self.cfg.device_id, self.cfg.port),
        };
        let mut backends: Vec<Box<dyn Discovery>> = Vec::new();
        if self.cfg.mdns {
            backends.push(Box::new(MdnsDiscovery::new()));
        }
        if let Some(port) = self.cfg.broadcast {
            backends.push(Box::new(BroadcastDiscovery::with_port(port)));
        }
        for mut b in backends {
            match b.start(&bcfg, self.backend_tx.clone()) {
                Ok(()) => self.backends.push(b),
                Err(e) => tracing::warn!(error = %e, "discovery backend failed to start"),
            }
        }
        tracing::info!(
            interfaces = self.cfg.interfaces.len(),
            backends = self.backends.len(),
            announce = self.cfg.announce,
            "discovery (re)started"
        );
    }

    fn on_backend(&mut self, ev: DiscoveryEvent) {
        match ev {
            DiscoveryEvent::Found(p) => {
                let peer = self.peers.entry(p.device_id).or_default();
                match p.source {
                    DiscoverySource::Broadcast => peer.broadcast = Some((p.addrs, Instant::now())),
                    DiscoverySource::Mdns | DiscoverySource::Manual => peer.mdns = Some(p.addrs),
                }
                self.report(p.device_id, p.source);
            }
            DiscoveryEvent::Lost(id) => {
                if let Some(peer) = self.peers.get_mut(&id) {
                    peer.mdns = None;
                }
                self.report(id, DiscoverySource::Mdns);
            }
        }
    }

    /// Emits `Found` when a peer is new or its addresses changed, `Lost` when it has
    /// no source left.
    fn report(&mut self, id: DeviceId, source: DiscoverySource) {
        let Some(peer) = self.peers.get_mut(&id) else {
            return;
        };
        let addrs = peer.addrs();
        if addrs.is_empty() {
            self.peers.remove(&id);
            let _ = self.out.send(DiscoveryEvent::Lost(id));
        } else if addrs != peer.reported {
            peer.reported = addrs.clone();
            let _ = self.out.send(DiscoveryEvent::Found(DiscoveredPeer {
                device_id: id,
                addrs: addrs.into_iter().collect(),
                source,
            }));
        }
    }

    fn expire(&mut self) {
        let now = Instant::now();
        let stale: Vec<_> = self
            .peers
            .iter_mut()
            .filter_map(|(id, p)| match p.broadcast {
                Some((_, seen)) if now.duration_since(seen) > LOST_AFTER => {
                    p.broadcast = None;
                    Some(*id)
                }
                _ => None,
            })
            .collect();
        for id in stale {
            self.report(id, DiscoverySource::Broadcast);
        }
    }

    fn set_interfaces(&mut self, interfaces: Vec<ActiveInterface>) {
        let subnets: Vec<_> = interfaces.iter().map(|i| i.subnet).collect();
        let reachable = |a: &SocketAddr| match a {
            SocketAddr::V4(v4) => subnets.iter().any(|s| s.contains(*v4.ip())),
            SocketAddr::V6(_) => false,
        };
        let ids: Vec<_> = self.peers.keys().copied().collect();
        for id in ids {
            let p = self.peers.get_mut(&id).expect("listed above");
            if let Some((a, _)) = &mut p.broadcast {
                a.retain(reachable);
                if a.is_empty() {
                    p.broadcast = None;
                }
            }
            if let Some(a) = &mut p.mdns {
                a.retain(reachable);
                if a.is_empty() {
                    p.mdns = None;
                }
            }
            self.report(id, DiscoverySource::Broadcast);
        }
        self.cfg.interfaces = interfaces;
        self.restart();
    }

    fn shutdown(&mut self) {
        for b in &mut self.backends {
            b.stop();
        }
        self.backends.clear();
    }
}

async fn run(
    cfg: ServiceConfig,
    mut commands: UnboundedReceiver<Command>,
    out: UnboundedSender<DiscoveryEvent>,
) {
    let (backend_tx, mut backend_rx) = mpsc::unbounded_channel();
    let mut st = State {
        cfg,
        backends: Vec::new(),
        backend_tx,
        peers: HashMap::new(),
        out,
    };
    st.restart();
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    let rotation = Duration::from_secs(ANNOUNCE_ROTATION_SECS);
    let mut rotate_at = tokio::time::Instant::now() + rotation;
    loop {
        tokio::select! {
            cmd = commands.recv() => match cmd {
                None => break,
                Some(Command::SetInterfaces(v)) => st.set_interfaces(v),
                Some(Command::SetAnnounce(on)) => {
                    st.cfg.announce = on;
                    st.restart();
                }
                Some(Command::Rotate) => {
                    st.restart();
                    rotate_at = tokio::time::Instant::now() + rotation;
                }
            },
            Some(ev) = backend_rx.recv() => st.on_backend(ev),
            _ = tick.tick() => st.expire(),
            _ = tokio::time::sleep_until(rotate_at) => {
                st.restart();
                rotate_at = tokio::time::Instant::now() + rotation;
            }
        }
    }
    st.shutdown();
}
