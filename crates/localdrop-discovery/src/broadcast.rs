//! UDP broadcast fallback backend (see `docs/discovery.md#udp-broadcast-fallback`).
//!
//! For networks where multicast is filtered. Every [`ANNOUNCE_INTERVAL`] (with jitter) the
//! raw announcement goes to the directed broadcast address of each active interface (e.g.
//! `192.168.1.255:47473`) from a socket bound to that interface, never to
//! `255.255.255.255`. One listener on [`BROADCAST_PORT`] accepts packets only from
//! active subnets.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use localdrop_core::{Error, Result};
use rand::Rng;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::announcement::{Announcement, ENCODED_LEN};
use crate::{BackendConfig, DiscoveredPeer, Discovery, DiscoveryEvent, DiscoverySource};

/// UDP port used for broadcast announcements.
pub const BROADCAST_PORT: u16 = 47_473;

/// How often each interface announces.
pub const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(5);

/// UDP broadcast discovery backend.
#[derive(Debug, Default)]
pub struct BroadcastDiscovery {
    tasks: Vec<JoinHandle<()>>,
    /// Port to listen and send to; [`BROADCAST_PORT`] unless changed for tests.
    port: Option<u16>,
}

impl BroadcastDiscovery {
    /// Creates the backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Uses another port than [`BROADCAST_PORT`] (tests on one machine).
    pub fn with_port(port: u16) -> Self {
        Self {
            tasks: Vec::new(),
            port: Some(port),
        }
    }
}

fn reuse_socket(bind: SocketAddrV4) -> std::io::Result<std::net::UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    s.set_reuse_address(true)?;
    #[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))]
    s.set_reuse_port(true)?;
    s.set_broadcast(true)?;
    s.set_nonblocking(true)?;
    s.bind(&SocketAddr::V4(bind).into())?;
    Ok(s.into())
}

impl Discovery for BroadcastDiscovery {
    fn start(
        &mut self,
        cfg: &BackendConfig,
        events: UnboundedSender<DiscoveryEvent>,
    ) -> Result<()> {
        self.stop();
        let port = self.port.unwrap_or(BROADCAST_PORT);
        let listener = UdpSocket::from_std(reuse_socket(SocketAddrV4::new(
            Ipv4Addr::UNSPECIFIED,
            port,
        ))?)?;
        let key = cfg.key.clone();
        let me = cfg.device_id;
        let subnets: Vec<_> = cfg.interfaces.iter().map(|i| i.subnet).collect();
        self.tasks.push(tokio::spawn(async move {
            let mut buf = [0u8; 512];
            loop {
                let Ok((n, from)) = listener.recv_from(&mut buf).await else {
                    continue;
                };
                let SocketAddr::V4(from) = from else { continue };
                if n != ENCODED_LEN || !subnets.iter().any(|s| s.contains(*from.ip())) {
                    continue;
                }
                let Ok(opened) = Announcement::from_bytes(&buf[..n]).and_then(|a| a.open(&key))
                else {
                    continue; // not our group
                };
                if opened.device_id == me {
                    continue;
                }
                let peer = DiscoveredPeer {
                    device_id: opened.device_id,
                    addrs: vec![SocketAddr::V4(SocketAddrV4::new(*from.ip(), opened.port))],
                    source: DiscoverySource::Broadcast,
                };
                if events.send(DiscoveryEvent::Found(peer)).is_err() {
                    break;
                }
            }
        }));

        if cfg.announce {
            for iface in &cfg.interfaces {
                let sock = match reuse_socket(SocketAddrV4::new(iface.subnet.addr, 0)) {
                    Ok(s) => UdpSocket::from_std(s)?,
                    Err(e) => {
                        tracing::warn!(iface = %iface.name, error = %e, "can't announce on interface");
                        continue;
                    }
                };
                let to = SocketAddrV4::new(iface.subnet.broadcast(), port);
                let bytes = cfg.announcement.as_bytes().to_vec();
                self.tasks.push(tokio::spawn(async move {
                    loop {
                        if let Err(e) = sock.send_to(&bytes, to).await {
                            tracing::debug!(%to, error = %e, "broadcast failed");
                        }
                        let jitter = rand::thread_rng().gen_range(0..1000);
                        tokio::time::sleep(ANNOUNCE_INTERVAL + Duration::from_millis(jitter)).await;
                    }
                }));
            }
        }
        if self.tasks.is_empty() {
            return Err(Error::Io(std::io::Error::other(
                "broadcast backend did not start",
            )));
        }
        Ok(())
    }

    fn stop(&mut self) {
        for t in self.tasks.drain(..) {
            t.abort();
        }
    }
}

impl Drop for BroadcastDiscovery {
    fn drop(&mut self) {
        self.stop();
    }
}
