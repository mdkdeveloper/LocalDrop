//! Events reported to the front-end through the node's event channel.

use localdrop_core::{DeviceId, JobId};
use localdrop_protocol::ConflictMode;
use localdrop_store::JobState;

use crate::api::{Destination, JobView};

/// Something the UI may want to show. Delivered in order on one channel
/// (FFI-friendly: no callbacks into Dart).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NodeEvent {
    /// Discovery was switched on or off, or the set of active networks changed.
    DiscoveryChanged {
        /// Switch state.
        on: bool,
        /// Networks discovery runs on now (0 = no trusted network connected).
        active_networks: u32,
    },
    /// Connected to a network that isn't trusted ("Trust “Office-5G”?").
    UntrustedNetwork {
        /// Network fingerprint, hex.
        key: String,
        /// Label, e.g. the Wi-Fi name.
        label: String,
    },

    /// A group member's announcement was seen.
    PeerFound {
        /// Device id.
        device: DeviceId,
    },
    /// A group member authenticated.
    PeerOnline {
        /// Device id.
        device: DeviceId,
        /// Device name from `Hello`.
        name: String,
    },
    /// A group member went away.
    PeerOffline {
        /// Device id.
        device: DeviceId,
    },
    /// A known device presented a different key and was refused.
    KeyMismatch {
        /// Device id.
        device: DeviceId,
        /// Name we know it by.
        name: String,
    },

    /// A job was added to a queue (either direction).
    JobAdded {
        /// The new row.
        job: JobView,
    },
    /// A job changed state.
    JobStateChanged {
        /// Job id.
        id: JobId,
        /// New state.
        state: JobState,
    },
    /// Progress of the job in flight (~4 per second).
    JobProgress {
        /// Job id.
        id: JobId,
        /// Bytes written by the receiver.
        bytes_done: u64,
        /// Total bytes.
        bytes_total: u64,
        /// Current speed, bytes per second.
        speed_bps: u64,
    },
    /// A job was removed from the list.
    JobRemoved {
        /// Job id.
        id: JobId,
    },

    /// An incoming job needs the user's decision (accept policy `Ask`).
    IncomingRequest {
        /// Job id.
        id: JobId,
        /// Sender.
        from: DeviceId,
        /// Job name.
        name: String,
        /// Number of files.
        files: u32,
        /// Total size.
        bytes: u64,
        /// Where it would go.
        destination: Destination,
    },
    /// A conflict dialog should open (on both sides; the first answer wins).
    ConflictAsked {
        /// Job id.
        id: JobId,
        /// Number of conflicting files.
        conflicts: u32,
        /// Modes that may be chosen.
        allowed_modes: Vec<ConflictMode>,
    },
    /// The open accept or conflict dialog was answered elsewhere; close it.
    DecisionMade {
        /// Job id.
        id: JobId,
    },
    /// Sender: files of a queued job changed or disappeared before it started.
    SourceChanged {
        /// Job id.
        id: JobId,
        /// Relative paths that changed.
        changed: Vec<String>,
        /// Relative paths that are gone.
        missing: Vec<String>,
    },
    /// The receiver's destination is almost full; the job is paused.
    LowSpace {
        /// Job id.
        id: JobId,
        /// Free bytes.
        free: u64,
        /// Bytes still needed.
        needed: u64,
    },

    /// Something failed outside a job (shown as a banner or toast).
    Error {
        /// Short description.
        message: String,
    },
}
