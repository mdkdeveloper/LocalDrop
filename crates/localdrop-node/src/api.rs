//! Plain data types of the node API (see `docs/node-api.md`).
//!
//! Everything here is owned and FFI-friendly: no references, no trait objects, no
//! callbacks. The bridge crate exposes the ids as hex strings.

use std::path::PathBuf;

use localdrop_core::{AcceptPolicy, DeviceId, JobId};
pub use localdrop_discovery::NetworkKind;
pub use localdrop_protocol::{ConflictMode, Destination};
use localdrop_store::{Direction, Job, JobState};

/// The group this device belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupInfo {
    /// Normalized group name.
    pub name: String,
    /// Fingerprint to compare across devices, e.g. `3fa2-91c0`.
    pub fingerprint: String,
}

/// Presence of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// Authenticated and connected.
    Online,
    /// Announced but not connected yet.
    Found,
    /// Not on the network.
    Offline,
}

/// A device of the group, as the device list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceView {
    /// Device id.
    pub id: DeviceId,
    /// Name shown in the UI.
    pub name: String,
    /// Online / found / offline.
    pub presence: Presence,
    /// Unix time (seconds) of the last authentication; 0 = never.
    pub last_seen_unix: u64,
    /// Per-device accept override; `None` = app default.
    pub accept_override: Option<AcceptPolicy>,
    /// Jobs waiting in the outgoing queue to this device.
    pub queued_jobs: u32,
}

/// A job row in the queue UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobView {
    /// Job id.
    pub id: JobId,
    /// The other device.
    pub peer: DeviceId,
    /// Outgoing or incoming.
    pub direction: Direction,
    /// Name, e.g. "Holiday photos".
    pub name: String,
    /// Where the files go on the receiver.
    pub destination: Destination,
    /// Queued while the peer was offline.
    pub deferred: bool,
    /// Number of files (folders not counted).
    pub files_total: u32,
    /// Sum of all file sizes.
    pub bytes_total: u64,
    /// Bytes written by the receiver.
    pub bytes_done: u64,
    /// State.
    pub state: JobState,
    /// Unix time (seconds) of creation.
    pub created_unix: u64,
}

impl From<&Job> for JobView {
    fn from(j: &Job) -> Self {
        Self {
            id: j.id,
            peer: j.peer,
            direction: j.direction,
            name: j.name.clone(),
            destination: j.destination.clone(),
            deferred: j.deferred,
            files_total: j
                .entries
                .iter()
                .filter(|e| e.manifest.kind == localdrop_protocol::EntryKind::File)
                .count() as u32,
            bytes_total: j.total_bytes,
            bytes_done: j.bytes_done,
            state: j.state,
            created_unix: j.created_unix,
        }
    }
}

/// What the user wants to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendRequest {
    /// Target device (online, or known and offline).
    pub target: DeviceId,
    /// Files and folders (desktop paths; on mobile, paths the app resolved).
    pub sources: Vec<PathBuf>,
    /// Inbox or a share with `upload`.
    pub destination: Destination,
}

/// A queue action from the job's ⋮ menu (`docs/transfers.md#job-actions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobAction {
    /// Pause; keeps the job's place in the queue.
    Pause,
    /// Resume a paused job.
    Resume,
    /// Put the job right after the unit in flight.
    MoveToTop,
    /// Cancel on both sides; finished files stay, partial files are deleted.
    Cancel,
    /// Send the failed files again.
    RetryFailed,
    /// Offer a job that got no answer again now.
    OfferAgain,
    /// Remove a finished or unanswered job from the list.
    Clear,
}

/// The user's answer to an incoming request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptAnswer {
    /// Accept or decline.
    pub accept: bool,
    /// "Always accept from this device" was ticked.
    pub always: bool,
}

/// The sender's choice when queued files changed (`docs/deferred-send.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceChangedChoice {
    /// Send the current versions; missing files are skipped.
    SendCurrent,
    /// Skip the changed files.
    SkipChanged,
    /// Cancel the job.
    Cancel,
    /// Ask again later.
    Later,
}

/// Trust state of a remembered network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NetworkTrust {
    /// Discovery runs here.
    Trusted,
    /// Discovery stays off; asked again next time.
    NotTrusted,
    /// Discovery stays off; never asked again.
    NeverAsk,
}

/// A connected or remembered network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkView {
    /// Network fingerprint, hex.
    pub key: String,
    /// Label, e.g. the Wi-Fi name or "Ethernet 192.168.1.x".
    pub label: String,
    /// Wi-Fi or Ethernet.
    pub kind: NetworkKind,
    /// This device's address on it while connected, e.g. `192.168.1.23/24`.
    pub address: Option<String>,
    /// When it was last connected (Unix seconds); 0 if never remembered.
    pub last_seen_unix: u64,
    /// Currently connected.
    pub connected: bool,
    /// Discovery currently runs on it.
    pub active: bool,
    /// Trust.
    pub trust: NetworkTrust,
    /// Identified by subnet and gateway only (other networks may look the same).
    pub weak_identity: bool,
}
