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
