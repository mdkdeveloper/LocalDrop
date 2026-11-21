//! Shared error type.

use crate::identity::DeviceId;

/// Convenience alias used across LocalDrop crates.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors produced by LocalDrop crates.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The group name is empty or otherwise unusable.
    #[error("invalid group name: {0}")]
    InvalidGroupName(&'static str),

    /// The group password is empty or otherwise unusable.
    #[error("invalid group password: {0}")]
    InvalidPassword(&'static str),

    /// Password-based key derivation failed.
    #[error("key derivation failed: {0}")]
    Kdf(String),

    /// Key material has the wrong length or format.
    #[error("invalid key material")]
    InvalidKey,

    /// A discovery announcement could not be parsed or does not belong to our group.
    #[error("invalid or foreign announcement")]
    InvalidAnnouncement,

    /// A protocol message could not be encoded or decoded.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// A known device presented a different public key than the one we pinned.
    #[error("public key mismatch for device {0}")]
    KeyMismatch(DeviceId),

    /// The membership handshake failed: the other side doesn't know the group password
    /// (or we don't know theirs).
    #[error("the other device is not in this group (wrong group name or password)")]
    AuthFailed,

    /// The peer speaks another major protocol version.
    #[error("the other device uses protocol version {0}; update LocalDrop on both")]
    IncompatibleVersion(u16),

    /// Too many failed handshakes from this address; try again later.
    #[error("too many failed attempts; try again in a minute")]
    RateLimited,

    /// Connecting or talking to a peer failed.
    #[error("connection failed: {0}")]
    Transport(String),

    /// The device is already connected; the new connection was dropped in favour of it.
    #[error("already connected to {0}")]
    AlreadyConnected(DeviceId),

    /// The peer did not answer in time.
    #[error("timed out")]
    Timeout,

    /// The requested device is not known (never seen / not paired).
    #[error("unknown device {0}")]
    UnknownDevice(DeviceId),

    /// An identifier string is not 32 hex characters.
    #[error("invalid identifier")]
    InvalidId,

    /// The requested item does not exist.
    #[error("not found")]
    NotFound,

    /// The action is not possible in the item's current state.
    #[error("not possible in the current state")]
    InvalidState,

    /// The feature is part of the planned design but not implemented yet.
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),

    /// Underlying I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
