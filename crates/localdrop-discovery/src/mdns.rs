//! mDNS / DNS-SD backend (stub).
//!
//! Planned implementation: `mdns-sd` crate. Service type [`MDNS_SERVICE_TYPE`], instance
//! name = random per rotation (never the device name), TXT `a=<base64url(announcement)>`.

use localdrop_core::{Error, GroupKey, Result};
use tokio::sync::mpsc::UnboundedSender;

use crate::{Discovery, DiscoveryEvent};

/// DNS-SD service type used by LocalDrop.
pub const MDNS_SERVICE_TYPE: &str = "_localdrop._udp.local.";

/// mDNS discovery backend.
#[derive(Debug)]
pub struct MdnsDiscovery {
    #[allow(dead_code)] // used once implemented
    key: GroupKey,
}

impl MdnsDiscovery {
    /// Creates the backend for the given group.
    pub fn new(key: GroupKey) -> Self {
        Self { key }
    }
}

impl Discovery for MdnsDiscovery {
    fn start(&mut self, _local_port: u16, _events: UnboundedSender<DiscoveryEvent>) -> Result<()> {
        Err(Error::NotImplemented("mDNS discovery"))
    }

    fn stop(&mut self) {}
}
