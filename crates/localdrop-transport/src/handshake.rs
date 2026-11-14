//! Membership handshake (planned).
//!
//! Runs on the first QUIC bidirectional stream right after TLS completes:
//!
//! 1. Both sides compute `ctx = TLS-Exporter("EXPORTER-localdrop-v1", 32)` and
//!    `ids = sort(pk_client, pk_server)`.
//! 2. Symmetric SPAKE2 using `k_auth` as the password and `ctx || ids` as identity.
//! 3. Each side sends `confirm = HMAC(spake_key, role || ctx)`; mismatch → close.
//!
//! An attacker without the password gets exactly one online guess per connection and
//! learns nothing useful for an offline dictionary attack. Binding to the TLS exporter
//! prevents relaying the handshake over another connection.

use localdrop_core::{Error, GroupKey, Result};

/// Label used with the TLS keying-material exporter.
pub const EXPORTER_LABEL: &[u8] = b"EXPORTER-localdrop-v1";

/// Which side of the connection we are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Connection initiator.
    Client,
    /// Connection acceptor.
    Server,
}

/// Runs the membership handshake. Not implemented yet.
pub fn run(_key: &GroupKey, _role: Role, _tls_exporter: &[u8; 32]) -> Result<()> {
    Err(Error::NotImplemented("membership handshake"))
}
