//! Control and data-stream messages.

use localdrop_core::{DeviceId, TransferId};
use serde::{Deserialize, Serialize};

/// Protocol version spoken by this build. Peers with a different major version
/// refuse the connection in [`ControlMessage::Hello`].
pub const PROTOCOL_VERSION: u16 = 1;

/// Metadata of one file inside an offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMeta {
    /// Index of the file within the transfer (0-based).
    pub index: u32,
    /// File name or relative path using `/` separators. The receiver MUST sanitize it
    /// with [`sanitize_file_name`](crate::sanitize_file_name).
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// BLAKE3 hash of the full content, verified by the receiver after writing.
    pub blake3: [u8; 32],
}

/// How much of a file the receiver already has (for resuming).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileResume {
    /// File index.
    pub index: u32,
    /// Bytes already received and kept; sender continues from here.
    pub offset: u64,
}

/// Why a receiver rejected an offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    /// The user declined.
    Declined,
    /// Not enough free space.
    NoSpace,
    /// Other / unspecified.
    Other,
}

/// Messages on the control stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlMessage {
    /// First message from both sides after the membership handshake.
    Hello {
        /// Sender's [`PROTOCOL_VERSION`].
        protocol_version: u16,
        /// Sender's device id (must match the authenticated key).
        device_id: DeviceId,
        /// Human-readable device name, e.g. "Dmytro's Pixel".
        device_name: String,
    },
    /// Sender proposes a transfer.
    Offer {
        /// Transfer id (stable across retries of a deferred transfer).
        transfer_id: TransferId,
        /// Files in the transfer.
        files: Vec<FileMeta>,
    },
    /// Receiver is waiting for the user to decide (manual accept policy).
    /// The sender keeps the transfer queued and may disconnect.
    Pending {
        /// Transfer id.
        transfer_id: TransferId,
    },
    /// Receiver accepts; includes resume offsets for partially received files.
    Accept {
        /// Transfer id.
        transfer_id: TransferId,
        /// Files with a non-zero resume offset (missing = start from 0).
        resume: Vec<FileResume>,
    },
    /// Receiver rejects the offer.
    Reject {
        /// Transfer id.
        transfer_id: TransferId,
        /// Reason.
        reason: RejectReason,
    },
    /// Receiver confirms that a file was fully received and its hash verified.
    FileDone {
        /// Transfer id.
        transfer_id: TransferId,
        /// File index.
        index: u32,
    },
    /// Either side cancels the transfer.
    Cancel {
        /// Transfer id.
        transfer_id: TransferId,
    },
    /// Keep-alive / liveness probe.
    Ping,
    /// Reply to [`ControlMessage::Ping`].
    Pong,
}

/// Header at the start of every per-file data stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStreamHeader {
    /// Transfer id.
    pub transfer_id: TransferId,
    /// File index.
    pub index: u32,
    /// Offset of the first byte that follows.
    pub offset: u64,
}
