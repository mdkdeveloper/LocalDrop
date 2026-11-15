//! mDNS / DNS-SD backend (see `docs/discovery.md#mdns--dns-sd-primary`), using `mdns-sd`.
//!
//! Service type [`MDNS_SERVICE_TYPE`]; the instance name is random and changes with every
//! rotation (never the device name); TXT `v=1`, `a=<base64url(announcement)>`. The daemon
//! is restricted to the active interfaces.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, SocketAddrV4};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use localdrop_core::{DeviceId, Error, Result};
use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent, ServiceInfo};
use rand::RngCore;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::announcement::Announcement;
use crate::{BackendConfig, DiscoveredPeer, Discovery, DiscoveryEvent, DiscoverySource};

/// DNS-SD service type used by LocalDrop.
pub const MDNS_SERVICE_TYPE: &str = "_localdrop._udp.local.";

/// mDNS discovery backend.
#[derive(Default)]
pub struct MdnsDiscovery {
    daemon: Option<ServiceDaemon>,
    task: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for MdnsDiscovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MdnsDiscovery")
            .field("running", &self.daemon.is_some())
            .finish()
    }
}

impl MdnsDiscovery {
    /// Creates the backend.
    pub fn new() -> Self {
        Self::default()
    }
}

fn mdns_err(e: mdns_sd::Error) -> Error {
    Error::Io(std::io::Error::other(format!("mDNS: {e}")))
}

/// Encodes an announcement for the TXT record.
pub fn encode_txt(a: &Announcement) -> String {
    URL_SAFE_NO_PAD.encode(a.as_bytes())
}

/// Decodes the TXT value back into an announcement.
pub fn decode_txt(s: &str) -> Option<Announcement> {
    let bytes = URL_SAFE_NO_PAD.decode(s).ok()?;
    Announcement::from_bytes(&bytes).ok()
}

impl Discovery for MdnsDiscovery {
    fn start(
        &mut self,
        cfg: &BackendConfig,
        events: UnboundedSender<DiscoveryEvent>,
    ) -> Result<()> {
        self.stop();
        let daemon = ServiceDaemon::new().map_err(mdns_err)?;
        daemon.disable_interface(IfKind::All).map_err(mdns_err)?;
        for iface in &cfg.interfaces {
            daemon
                .enable_interface(IfKind::Addr(IpAddr::V4(iface.subnet.addr)))
                .map_err(mdns_err)?;
        }

        if cfg.announce && !cfg.interfaces.is_empty() {
            let mut rnd = [0u8; 8];
            rand::rngs::OsRng.fill_bytes(&mut rnd);
            let instance: String = rnd.iter().map(|b| format!("{b:02x}")).collect();
            let ips: Vec<String> = cfg
                .interfaces
                .iter()
                .map(|i| i.subnet.addr.to_string())
                .collect();
            let txt = encode_txt(&cfg.announcement);
            let info = ServiceInfo::new(
                MDNS_SERVICE_TYPE,
                &instance,
                &format!("{instance}.local."),
                ips.join(","),
                cfg.port,
                &[("v", "1"), ("a", txt.as_str())][..],
            )
            .map_err(mdns_err)?;
            daemon.register(info).map_err(mdns_err)?;
        }

        let rx = daemon.browse(MDNS_SERVICE_TYPE).map_err(mdns_err)?;
        let key = cfg.key.clone();
        let me = cfg.device_id;
        let subnets: Vec<_> = cfg.interfaces.iter().map(|i| i.subnet).collect();
        self.task = Some(tokio::spawn(async move {
            // fullname -> device, so a goodbye can be mapped back.
            let mut names: HashMap<String, DeviceId> = HashMap::new();
            while let Ok(ev) = rx.recv_async().await {
                let out = match ev {
                    ServiceEvent::ServiceResolved(svc) => {
                        let Some(opened) = svc
                            .get_property_val_str("a")
                            .and_then(decode_txt)
                            .and_then(|a| a.open(&key).ok())
                        else {
                            continue; // not our group
                        };
                        if opened.device_id == me {
                            continue;
                        }
                        let addrs: Vec<SocketAddr> = svc
                            .get_addresses_v4()
                            .into_iter()
                            .filter(|ip| subnets.iter().any(|s| s.contains(*ip)))
                            .map(|ip| SocketAddr::V4(SocketAddrV4::new(ip, opened.port)))
                            .collect();
                        if addrs.is_empty() {
                            continue;
                        }
                        names.insert(svc.get_fullname().to_owned(), opened.device_id);
                        DiscoveryEvent::Found(DiscoveredPeer {
                            device_id: opened.device_id,
                            addrs,
                            source: DiscoverySource::Mdns,
                        })
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => match names.remove(&fullname) {
                        // Only lost if no other instance (after a rotation) still maps to it.
                        Some(id) if !names.values().any(|v| *v == id) => DiscoveryEvent::Lost(id),
                        _ => continue,
                    },
                    _ => continue,
                };
                if events.send(out).is_err() {
                    break;
                }
            }
        }));
        self.daemon = Some(daemon);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
        if let Some(d) = self.daemon.take() {
            // Sends goodbyes for registered services, then stops the daemon thread.
            let _ = d.shutdown();
        }
    }
}

impl Drop for MdnsDiscovery {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use localdrop_core::{GroupConfig, GroupKey, KdfParams};

    use super::*;

    #[test]
    fn txt_roundtrip() {
        let key = GroupKey::derive(
            &GroupConfig::new("home").unwrap(),
            "pw",
            KdfParams::INSECURE_TEST,
        )
        .unwrap();
        let a = Announcement::seal(&key, &DeviceId([7; 16]), 4242);
        let txt = encode_txt(&a);
        assert!(txt.len() < 80, "fits a TXT string");
        let back = decode_txt(&txt).unwrap().open(&key).unwrap();
        assert_eq!((back.device_id, back.port), (DeviceId([7; 16]), 4242));
        assert!(decode_txt("not base64!").is_none());
    }
}
