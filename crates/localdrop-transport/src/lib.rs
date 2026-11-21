//! Authenticated transport between group members (see `docs/security.md`).
//!
//! QUIC (`quinn`) over UDP with `rustls` (TLS 1.3 only, ALPN `localdrop/1`). Each side
//! presents a self-signed certificate made from its
//! [`DeviceIdentity`](localdrop_core::DeviceIdentity), so TLS gives encryption and
//! proves possession of the device key. The [`handshake`] then proves knowledge of the
//! group password, bound to the TLS session, and both sides exchange `Hello` on the
//! control stream. The result is a [`Session`].

pub mod cert;
pub mod handshake;
pub mod limit;
mod quic;
mod session;

pub use handshake::Role;
pub use quic::{ALPN, Endpoint, HANDSHAKE_TIMEOUT, Incoming};
pub use session::{PING_TIMEOUT, PeerInfo, Session, code};

/// Port used by *Connect by address* when none is given: devices that want to be
/// reachable that way set it as their fixed port (Settings › General). Discovery itself
/// uses a random port by default.
pub const DEFAULT_FIXED_PORT: u16 = 47_474;
