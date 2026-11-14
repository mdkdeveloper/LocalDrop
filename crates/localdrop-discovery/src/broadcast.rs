//! UDP broadcast fallback backend (stub).
//!
//! For networks where multicast/mDNS is filtered. Each device periodically sends its
//! [`Announcement`](crate::Announcement) to `255.255.255.255:BROADCAST_PORT` (and to
//! subnet broadcast addresses of every interface) and listens on the same port.

use localdrop_core::{Error, GroupKey, Result};
use tokio::sync::mpsc::UnboundedSender;

use crate::{Discovery, DiscoveryEvent};

/// UDP port used for broadcast announcements.
pub const BROADCAST_PORT: u16 = 47_473;

/// UDP broadcast discovery backend.
#[derive(Debug)]
pub struct BroadcastDiscovery {
    #[allow(dead_code)] // used once implemented
    key: GroupKey,
}

impl BroadcastDiscovery {
    /// Creates the backend for the given group.
    pub fn new(key: GroupKey) -> Self {
        Self { key }
    }
}

impl Discovery for BroadcastDiscovery {
    fn start(&mut self, _local_port: u16, _events: UnboundedSender<DiscoveryEvent>) -> Result<()> {
        Err(Error::NotImplemented("UDP broadcast discovery"))
    }

    fn stop(&mut self) {}
}
