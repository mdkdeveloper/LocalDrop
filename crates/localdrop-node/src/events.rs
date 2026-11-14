//! Events reported to the front-end.

use localdrop_core::{DeviceId, TransferId};

/// Something the UI may want to show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NodeEvent {
    /// A group member came online (authenticated).
    PeerOnline {
        /// Device id.
        device: DeviceId,
        /// Device name.
        name: String,
    },
    /// A group member went offline.
    PeerOffline {
        /// Device id.
        device: DeviceId,
    },
    /// A transfer was added to the outbox.
    TransferQueued {
        /// Transfer id.
        id: TransferId,
        /// Target device.
        target: DeviceId,
    },
    /// An incoming offer needs the user's decision (manual accept policy).
    IncomingOffer {
        /// Transfer id.
        id: TransferId,
        /// Sender.
        from: DeviceId,
    },
    /// Transfer progress.
    Progress {
        /// Transfer id.
        id: TransferId,
        /// Bytes done.
        bytes_done: u64,
        /// Bytes total.
        bytes_total: u64,
    },
    /// Transfer finished successfully.
    TransferCompleted {
        /// Transfer id.
        id: TransferId,
    },
    /// Transfer failed.
    TransferFailed {
        /// Transfer id.
        id: TransferId,
        /// Error description.
        reason: String,
    },
}
