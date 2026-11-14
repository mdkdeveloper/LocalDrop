//! Authenticated transport between group members (see `docs/security.md`).
//!
//! Planned stack: QUIC (`quinn`) over UDP with `rustls`, each side presenting a
//! self-signed certificate made from its [`DeviceIdentity`](localdrop_core::DeviceIdentity).
//! TLS gives encryption and proves possession of the device key; the
//! [`handshake`] then proves knowledge of the group password.

pub mod handshake;
mod quic;

use std::net::SocketAddr;

use localdrop_core::{PublicKey, Result};
pub use quic::QuicTransport;

/// Default QUIC listening port (0 = pick any free port; the real port is announced).
pub const DEFAULT_PORT: u16 = 0;

/// A peer that completed both TLS and the membership handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedPeer {
    /// Peer's public key from its TLS certificate.
    pub public_key: PublicKey,
    /// Remote address.
    pub addr: SocketAddr,
}

/// A transport that can accept and open authenticated connections.
///
/// Concrete types and async methods will be introduced together with the QUIC
/// implementation; this trait captures the intended responsibilities.
pub trait Transport: Send + Sync {
    /// Local address the transport listens on.
    fn local_addr(&self) -> Result<SocketAddr>;
}
