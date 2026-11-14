//! Stored records.

use std::path::PathBuf;

use localdrop_core::{AcceptPolicy, DeviceId, PublicKey, TransferId};
use serde::{Deserialize, Serialize};

/// A group member we have authenticated before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownDevice {
    /// Device id (derived from `public_key`).
    pub id: DeviceId,
    /// Pinned public key.
    pub public_key: PublicKey,
    /// Last announced human-readable name.
    pub name: String,
    /// Unix time (seconds) of the last successful authentication.
    pub last_seen_unix: u64,
    /// Per-device accept policy override.
    pub accept_override: Option<AcceptPolicy>,
}

/// Where the sender reads a queued file from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileSource {
    /// Desktop: reference to the original file. If size/mtime/hash differ at send
    /// time, the transfer is paused and the user is warned.
    Reference {
        /// Absolute path.
        path: PathBuf,
        /// Size at enqueue time.
        size: u64,
        /// Modification time at enqueue time (Unix seconds).
        mtime_unix: u64,
    },
    /// Mobile: copy inside the app's cache directory, deleted after delivery.
    Cached {
        /// Absolute path of the cached copy.
        path: PathBuf,
        /// Size of the copy.
        size: u64,
    },
}

/// One file in a queued transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedFile {
    /// Name presented to the receiver.
    pub name: String,
    /// Source of the bytes.
    pub source: FileSource,
    /// BLAKE3 hash computed at enqueue time.
    pub blake3: [u8; 32],
}

/// State of a queued transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutboxState {
    /// Waiting for the target to come online.
    Waiting,
    /// Offer delivered; receiver asked its user (manual accept policy).
    AwaitingAccept,
    /// Bytes are being transferred.
    InProgress,
    /// Source file changed or disappeared since enqueue; needs user attention.
    SourceChanged,
    /// All files delivered and verified.
    Completed,
    /// Receiver rejected the transfer.
    Rejected,
    /// Cancelled by the sender.
    Cancelled,
}

impl OutboxState {
    /// Whether the transfer still needs delivery attempts.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Waiting | Self::AwaitingAccept | Self::InProgress
        )
    }
}

/// A transfer waiting in the outbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedTransfer {
    /// Transfer id (reused for every delivery attempt).
    pub id: TransferId,
    /// Target device; must be in [`KnownDevices`](crate::KnownDevices).
    pub target: DeviceId,
    /// Files.
    pub files: Vec<QueuedFile>,
    /// Unix time (seconds) of enqueue.
    pub created_unix: u64,
    /// Optional expiry (Unix seconds) after which the transfer is dropped.
    pub expires_unix: Option<u64>,
    /// Current state.
    pub state: OutboxState,
}
