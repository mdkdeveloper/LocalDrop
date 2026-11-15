//! Stored records.

use std::path::PathBuf;

use localdrop_core::{AcceptPolicy, DeviceId, JobId, PublicKey};
use localdrop_protocol::{Destination, ManifestEntry};
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

/// Which pair queue a job is in, seen from this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    /// This device sends to the peer.
    Outgoing,
    /// The peer sends to this device.
    Incoming,
}

/// How the sender keeps a job's files until they are sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceStorage {
    /// Desktop: the original files are read at send time. Size and mtime in the
    /// manifest are compared then; a difference marks the job `SourceChanged`.
    Reference,
    /// Mobile: copies in this cache folder, deleted when the job ends.
    Cached {
        /// Folder holding the job's copies.
        dir: PathBuf,
    },
}

/// One manifest entry plus where the sender reads it from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobEntry {
    /// What the receiver sees.
    pub manifest: ManifestEntry,
    /// Sender: absolute path of the original or of the cached copy.
    /// Receiver: absolute path of the target once known.
    pub local_path: Option<PathBuf>,
}

/// State of a job (see `docs/deferred-send.md#state-machine` and `docs/transfers.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    /// Waiting for the peer to come online (deferred) or for its turn in the queue.
    Waiting,
    /// The receiver's user has to accept.
    AwaitingAccept,
    /// A conflict dialog is open on both sides.
    AwaitingConflictDecision,
    /// Paused by the receiver because its destination is almost full.
    LowSpace,
    /// Units are being sent.
    InProgress,
    /// Paused by a user; keeps its place in the queue.
    Paused,
    /// Sender: source files changed or disappeared before the job started.
    SourceChanged,
    /// The accept request timed out (live jobs; deferred ones return to `Waiting`).
    NoAnswer,
    /// All files delivered and verified.
    Completed,
    /// Finished, but some files failed (they can be retried).
    CompletedWithErrors,
    /// The receiver rejected the job.
    Rejected,
    /// The job failed as a whole.
    Failed,
    /// Cancelled on either side.
    Cancelled,
    /// The optional expiry passed before delivery.
    Expired,
}

impl JobState {
    /// Whether the job still occupies its pair queue (offered or resumed on connect).
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Waiting
                | Self::AwaitingAccept
                | Self::AwaitingConflictDecision
                | Self::LowSpace
                | Self::InProgress
                | Self::Paused
                | Self::SourceChanged
        )
    }

    /// Whether the job has ended and only shows in history.
    pub fn is_finished(self) -> bool {
        !self.is_active() && self != Self::NoAnswer
    }
}

/// A job in a pair queue, in either direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// Job id (stable for the job's whole life).
    pub id: JobId,
    /// The other device.
    pub peer: DeviceId,
    /// Which pair queue.
    pub direction: Direction,
    /// Name shown in the queue, e.g. "Holiday photos".
    pub name: String,
    /// Where the files go on the receiver.
    pub destination: Destination,
    /// Created while the peer was offline (goes to the deferred inbox).
    pub deferred: bool,
    /// Manifest with local paths.
    pub entries: Vec<JobEntry>,
    /// Sender only: how the files are kept until sent.
    pub storage: SourceStorage,
    /// Sum of all file sizes.
    pub total_bytes: u64,
    /// Bytes written by the receiver so far.
    pub bytes_done: u64,
    /// Queue position; lower goes first. Ties are broken by `created_unix`.
    pub position: u64,
    /// Unix time (seconds) of creation.
    pub created_unix: u64,
    /// Optional expiry (Unix seconds) after which the job is dropped.
    pub expires_unix: Option<u64>,
    /// Current state.
    pub state: JobState,
}
