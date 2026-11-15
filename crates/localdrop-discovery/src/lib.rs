//! Password-scoped LAN discovery.
//!
//! Devices announce themselves on the LAN with an opaque, encrypted
//! [`Announcement`]. Only members of the same group (i.e. holders of the same
//! [`GroupKey`]) can open it and learn the [`DeviceId`] behind it; everyone else sees
//! random-looking bytes that change on every rotation.
//!
//! Two backends carry announcements (see `docs/discovery.md`):
//! - [`MdnsDiscovery`]: DNS-SD service `_localdrop._udp.local`, announcement in a TXT record.
//! - [`BroadcastDiscovery`]: UDP broadcast fallback on [`BROADCAST_PORT`].
//!
//! [`DiscoveryService`] runs both on the active interfaces ([`net`]) and merges what they
//! find. Discovery is only a hint: being discovered never grants access, every connection
//! still runs the membership handshake in `localdrop-transport`.

pub mod announcement;
mod broadcast;
pub mod mdns;
pub mod net;
pub mod service;

use std::net::SocketAddr;

pub use announcement::{Announcement, OpenedAnnouncement};
pub use broadcast::{ANNOUNCE_INTERVAL, BROADCAST_PORT, BroadcastDiscovery};
use localdrop_core::{DeviceId, GroupKey, Result};
pub use mdns::{MDNS_SERVICE_TYPE, MdnsDiscovery};
pub use net::{InterfaceClass, NetInterface, NetworkKey, NetworkKind, Subnet};
pub use service::{DiscoveryService, LOST_AFTER, ServiceConfig};
use tokio::sync::mpsc::UnboundedSender;

/// How long an announcement nonce is reused before rotating (seconds).
pub const ANNOUNCE_ROTATION_SECS: u64 = 15 * 60;

/// Where a peer was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverySource {
    /// mDNS / DNS-SD.
    Mdns,
    /// UDP broadcast fallback.
    Broadcast,
    /// Address entered manually by the user.
    Manual,
}

/// A group member seen on the network (not yet authenticated).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    /// Claimed device id (verified later by the handshake).
    pub device_id: DeviceId,
    /// Addresses the peer can be reached at.
    pub addrs: Vec<SocketAddr>,
    /// Mechanism that found it (the latest one, after merging).
    pub source: DiscoverySource,
}

/// Events emitted by discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryEvent {
    /// A peer appeared or its addresses changed.
    Found(DiscoveredPeer),
    /// A peer has not been seen for a while or withdrew its announcement.
    Lost(DeviceId),
}

/// An interface discovery runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveInterface {
    /// OS interface name (for logs).
    pub name: String,
    /// Its IPv4 address and prefix.
    pub subnet: Subnet,
}

impl ActiveInterface {
    /// One entry per IPv4 subnet of `iface`.
    pub fn from_interface(iface: &NetInterface) -> Vec<Self> {
        iface
            .subnets
            .iter()
            .map(|s| Self {
                name: iface.name.clone(),
                subnet: *s,
            })
            .collect()
    }
}

/// What a backend needs to run.
#[derive(Debug, Clone)]
pub struct BackendConfig {
    /// Our device id (own announcements are ignored).
    pub device_id: DeviceId,
    /// Opens other members' announcements.
    pub key: GroupKey,
    /// Port in our announcement.
    pub port: u16,
    /// Interfaces to announce and listen on.
    pub interfaces: Vec<ActiveInterface>,
    /// Announce, or only listen.
    pub announce: bool,
    /// Our current (sealed) announcement.
    pub announcement: Announcement,
}

/// A discovery backend.
pub trait Discovery: Send {
    /// Starts announcing and listening; restarts if already running.
    fn start(&mut self, cfg: &BackendConfig, events: UnboundedSender<DiscoveryEvent>)
    -> Result<()>;

    /// Stops announcing and listening.
    fn stop(&mut self);
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::time::Duration;

    use localdrop_core::{GroupConfig, KdfParams};
    use tokio::sync::mpsc;

    use super::*;

    fn key(group: &str) -> GroupKey {
        GroupKey::derive(
            &GroupConfig::new(group).unwrap(),
            "pw",
            KdfParams::INSECURE_TEST,
        )
        .unwrap()
    }

    /// Loopback stands in for a LAN: 127.0.0.0/8 broadcast is 127.255.255.255, which the
    /// OS delivers to the listeners on the same machine.
    fn lo() -> Vec<ActiveInterface> {
        vec![ActiveInterface {
            name: "lo".into(),
            subnet: Subnet {
                addr: Ipv4Addr::LOCALHOST,
                prefix: 8,
            },
        }]
    }

    fn cfg(id: u8, group: &str, port: u16) -> ServiceConfig {
        ServiceConfig {
            device_id: DeviceId([id; 16]),
            key: key(group),
            port: 40_000 + u16::from(id),
            interfaces: lo(),
            announce: true,
            mdns: false,
            broadcast: Some(port),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn members_find_each_other_and_outsiders_stay_hidden() {
        let port = 47_000 + (std::process::id() % 400) as u16;
        let (tx_a, mut rx_a) = mpsc::unbounded_channel();
        let (tx_c, mut rx_c) = mpsc::unbounded_channel();
        let a = DiscoveryService::spawn(cfg(1, "home", port), tx_a);
        let b = DiscoveryService::spawn(cfg(2, "home", port), mpsc::unbounded_channel().0);
        let c = DiscoveryService::spawn(cfg(3, "office", port), tx_c);

        let found = tokio::time::timeout(Duration::from_secs(10), rx_a.recv()).await;
        let Ok(Some(DiscoveryEvent::Found(p))) = found else {
            // Some CI sandboxes drop loopback broadcast; nothing to assert then.
            eprintln!("no loopback broadcast on this machine: {found:?}");
            return;
        };
        assert_eq!(p.device_id, DeviceId([2; 16]));
        assert_eq!(
            p.addrs,
            vec![SocketAddr::from((Ipv4Addr::LOCALHOST, 40_002))]
        );
        assert_eq!(p.source, DiscoverySource::Broadcast);

        // The other group's device sees nothing it can open.
        assert!(
            tokio::time::timeout(Duration::from_secs(1), rx_c.recv())
                .await
                .is_err()
        );

        // B stops: A reports it lost after the timeout.
        b.stop().await;
        let lost = tokio::time::timeout(LOST_AFTER + Duration::from_secs(4), async {
            loop {
                if let Some(DiscoveryEvent::Lost(id)) = rx_a.recv().await {
                    return id;
                }
            }
        })
        .await
        .expect("lost in time");
        assert_eq!(lost, DeviceId([2; 16]));

        a.stop().await;
        c.stop().await;
    }

    #[tokio::test]
    async fn no_interfaces_means_silence() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut c = cfg(9, "home", 47_499);
        c.interfaces.clear();
        let s = DiscoveryService::spawn(c, tx);
        assert!(
            tokio::time::timeout(Duration::from_millis(300), rx.recv())
                .await
                .is_err()
        );
        s.stop().await;
    }
}
