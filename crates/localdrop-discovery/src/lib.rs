//! Password-scoped LAN discovery.
//!
//! Devices announce themselves on the LAN with an opaque, encrypted
//! [`Announcement`]. Only members of the same group (i.e. holders of the same
//! [`GroupKey`](localdrop_core::GroupKey)) can open it and learn the [`DeviceId`] behind
//! it; everyone else sees random-looking bytes that change on every rotation.
//!
//! Two mechanisms carry announcements (see `docs/discovery.md`):
//! - [`MdnsDiscovery`]: DNS-SD service `_localdrop._udp.local`, announcement in a TXT record.
//! - [`BroadcastDiscovery`]: UDP broadcast fallback on [`BROADCAST_PORT`].
//!
//! Discovery is only a hint. Being discovered never grants access: every connection
//! still runs the membership handshake in `localdrop-transport`.

pub mod announcement;
mod broadcast;
mod mdns;

use std::net::SocketAddr;

pub use announcement::{Announcement, OpenedAnnouncement};
pub use broadcast::{BROADCAST_PORT, BroadcastDiscovery};
use localdrop_core::{DeviceId, Result};
pub use mdns::{MDNS_SERVICE_TYPE, MdnsDiscovery};
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
    /// Mechanism that found it.
    pub source: DiscoverySource,
}

/// Events emitted by a [`Discovery`] backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryEvent {
    /// A peer appeared or its addresses changed.
    Found(DiscoveredPeer),
    /// A peer has not been seen for a while or withdrew its announcement.
    Lost(DeviceId),
}

/// A discovery backend.
pub trait Discovery: Send {
    /// Starts announcing `local_port` for our device and listening for peers.
    fn start(&mut self, local_port: u16, events: UnboundedSender<DiscoveryEvent>) -> Result<()>;

    /// Stops announcing and listening.
    fn stop(&mut self);
}
