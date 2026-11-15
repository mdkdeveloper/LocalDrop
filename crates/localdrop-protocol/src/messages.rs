//! Control, unit-data and request messages (see `docs/protocol.md`).

use localdrop_core::{DeviceId, JobId, ShareId};
use serde::{Deserialize, Serialize};

/// Protocol version spoken by this build. Peers with a different major version
/// refuse the connection after [`ControlMessage::Hello`].
pub const PROTOCOL_VERSION: u16 = 1;

/// Optional features, exchanged in [`ControlMessage::Hello`] as a bit set so that new
/// features don't need a version bump. No bits are defined in v1.
pub type Capabilities = u64;

// ---------------------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------------------

/// Where a job's files go on the receiver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Destination {
    /// The receiver's inbox (or deferred inbox for deferred jobs).
    Inbox,
    /// A folder inside one of the receiver's shares (needs `upload`).
    Share {
        /// Share id.
        share_id: ShareId,
        /// Relative, `/`-separated folder inside the share; empty = share root.
        path: String,
    },
}

/// Kind of a manifest or directory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    /// Regular file.
    File,
    /// Folder (created from the manifest, even if empty).
    Dir,
    /// Symbolic link: listed in shares, never followed or transferred.
    Symlink,
}

/// One file or folder of a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Index within the job (0-based, dense). Referenced by every later message.
    pub index: u32,
    /// Relative, `/`-separated path under the destination. The receiver MUST sanitize
    /// it with [`sanitize_file_name`](crate::sanitize_file_name).
    pub path: String,
    /// File or folder.
    pub kind: EntryKind,
    /// Size in bytes (0 for folders).
    pub size: u64,
    /// Modification time, Unix milliseconds.
    pub mtime_unix_ms: i64,
    /// Unix executable bit (ignored where the file system has none).
    pub executable: bool,
}

/// Why the receiver holds a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingReason {
    /// The receiver's user has to accept the job (accept policy `Ask`).
    AwaitingAccept,
    /// A conflict dialog is open on both sides.
    AwaitingConflictDecision,
    /// The destination is running out of space; the receiver continues on its own.
    LowSpace {
        /// Free bytes on the destination volume.
        free: u64,
        /// Bytes still to write for this job.
        needed: u64,
    },
}

/// How existing files are handled (see `docs/shares.md#conflicts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictMode {
    /// Write only new files; conflicting ones are skipped.
    KeepExisting,
    /// Keep both: the incoming file is renamed to `name (n).ext` (inbox only).
    KeepBoth,
    /// Replace, moving the old versions into the share's backup area.
    ReplaceWithBackup,
    /// Replace; old versions are removed according to the share's delete mode.
    ReplaceConflicting,
    /// Replace and remove files missing from the upload (target becomes a copy).
    Mirror,
}

/// An incoming file whose target already exists with different content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// Manifest index of the incoming file.
    pub index: u32,
    /// Size of the existing file.
    pub existing_size: u64,
    /// Modification time of the existing file, Unix milliseconds.
    pub existing_mtime_unix_ms: i64,
}

/// Where a job continues after a pause, disconnect or restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumePoint {
    /// First unit that is not acknowledged.
    pub unit: u32,
    /// First file inside that unit that is not finalized.
    pub file_index: u32,
    /// First unverified segment of that file.
    pub segment: u64,
}

/// Why a receiver rejected a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    /// The user declined.
    Declined,
    /// The accept request timed out (`ACCEPT_TIMEOUT`).
    NoAnswer,
    /// Not enough free space, or a file is too large for the destination file system.
    NoSpace,
    /// The sender lacks permission for the destination.
    Denied,
    /// Other / unspecified.
    Other,
}

/// Why the sender could not send one file of the current unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    /// Size or mtime changed while sending.
    SourceChanged,
    /// The source is locked by another program.
    SourceLocked,
    /// The source was deleted or moved.
    SourceMissing,
    /// Any other read error.
    ReadError,
}

/// Why the receiver could not finish one file of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileFailReason {
    /// A segment hash did not match (re-sent by the sender, up to 3 times).
    HashMismatch,
    /// The target is open in another program and can't be replaced.
    TargetLocked,
    /// Any other write error.
    WriteError,
}

/// A file the receiver could not finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedFile {
    /// Manifest index.
    pub index: u32,
    /// Reason.
    pub reason: FileFailReason,
}

// ---------------------------------------------------------------------------------------
// Control stream
// ---------------------------------------------------------------------------------------

/// Messages on the control stream, in both directions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlMessage {
    /// First message from both sides after the membership handshake.
    Hello {
        /// Sender's [`PROTOCOL_VERSION`].
        protocol_version: u16,
        /// Sender's device id (must match the authenticated key).
        device_id: DeviceId,
        /// Human-readable device name, e.g. "Work laptop".
        device_name: String,
        /// Optional features.
        capabilities: Capabilities,
    },
    /// Liveness probe.
    Ping,
    /// Reply to [`ControlMessage::Ping`].
    Pong,

    /// Sender proposes a job. Followed by one or more [`ControlMessage::ManifestChunk`]s.
    JobOffer {
        /// Job id (stable for the job's whole life).
        job_id: JobId,
        /// Name shown in the queue, e.g. "Holiday photos".
        name: String,
        /// Where the files go.
        destination: Destination,
        /// Number of manifest entries.
        total_files: u32,
        /// Sum of all file sizes.
        total_bytes: u64,
        /// The job comes from the sender's offline queue.
        deferred: bool,
    },
    /// Part of a job's manifest.
    ManifestChunk {
        /// Job id.
        job_id: JobId,
        /// Entries, in index order.
        entries: Vec<ManifestEntry>,
        /// This is the last chunk.
        last: bool,
    },
    /// The receiver holds the job; the sender pauses it and serves other jobs.
    JobPending {
        /// Job id.
        job_id: JobId,
        /// Why.
        reason: PendingReason,
    },
    /// Files that already exist and differ, with the modes the owner allows.
    ConflictReport {
        /// Job id.
        job_id: JobId,
        /// Conflicting entries (may be split across several reports).
        conflicts: Vec<Conflict>,
        /// Modes the owner's permissions allow.
        allowed_modes: Vec<ConflictMode>,
        /// This is the last report for the job.
        last: bool,
    },
    /// A conflict decision from either side; the first valid one wins.
    ConflictDecision {
        /// Job id.
        job_id: JobId,
        /// Chosen mode.
        mode: ConflictMode,
    },
    /// Start or continue sending.
    JobAccept {
        /// Job id.
        job_id: JobId,
        /// Manifest indices not to send (identical or `KeepExisting`).
        skip: Vec<u32>,
        /// Where to continue; `None` = from the start.
        resume: Option<ResumePoint>,
    },
    /// A segment hash did not match; the sender re-sends that segment.
    SegmentFailed {
        /// Job id.
        job_id: JobId,
        /// Manifest index.
        file_index: u32,
        /// Segment number.
        segment: u64,
    },
    /// The receiver rejects the job.
    JobReject {
        /// Job id.
        job_id: JobId,
        /// Reason.
        reason: RejectReason,
    },
    /// The sender could not send a file of the current unit.
    FileSkipped {
        /// Job id.
        job_id: JobId,
        /// Manifest index.
        file_index: u32,
        /// Reason.
        reason: SkipReason,
    },
    /// A unit was written, synced and verified.
    UnitDone {
        /// Job id.
        job_id: JobId,
        /// Unit number.
        unit: u32,
        /// Manifest indices finalized in this unit.
        files_ok: Vec<u32>,
        /// Files that failed.
        files_failed: Vec<FailedFile>,
    },
    /// The user retries failed files; they come as a new unit at the end of the job.
    FileRetry {
        /// Job id.
        job_id: JobId,
        /// Manifest indices.
        file_indices: Vec<u32>,
    },
    /// All files written; mirror deletions (if any) done.
    JobDone {
        /// Job id.
        job_id: JobId,
    },
    /// Pause a job (keeps its place in the queue).
    JobPause {
        /// Job id.
        job_id: JobId,
    },
    /// Resume a paused job.
    JobResume {
        /// Job id.
        job_id: JobId,
    },
    /// Cancel a job; final on both sides.
    JobCancel {
        /// Job id.
        job_id: JobId,
    },
    /// Bytes written by the receiver (~4 per second).
    Progress {
        /// Job id.
        job_id: JobId,
        /// Bytes written so far for the whole job.
        bytes_written: u64,
    },
}

// ---------------------------------------------------------------------------------------
// Unit data streams
// ---------------------------------------------------------------------------------------

/// Kind of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitKind {
    /// One file of at least `LARGE_FILE_THRESHOLD` bytes.
    Large,
    /// Many small files back to back.
    Bundle,
}

/// First frame of a unit data stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitHeader {
    /// Job id.
    pub job_id: JobId,
    /// Unit number within the job.
    pub unit: u32,
    /// Large file or bundle.
    pub kind: UnitKind,
    /// First segment sent (non-zero when a large file resumes).
    pub first_segment: u64,
}

/// Starts one file inside a unit stream; raw segments follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryHeader {
    /// Manifest index.
    pub index: u32,
    /// Offset of the first byte that follows (always a segment boundary).
    pub offset: u64,
    /// Number of bytes that follow for this entry.
    pub len: u64,
}

/// Follows every segment's raw bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentTrailer {
    /// Segment number within the file.
    pub segment: u64,
    /// BLAKE3 of this segment's bytes.
    pub blake3: [u8; 32],
}

/// Ends one file inside a unit stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryTrailer {
    /// Manifest index.
    pub index: u32,
    /// [`content_hash`](crate::content_hash) of the whole file.
    pub content_hash: [u8; 32],
}

// ---------------------------------------------------------------------------------------
// Remote requests (shares)
// ---------------------------------------------------------------------------------------

/// What happens to deleted or replaced files in a share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteMode {
    /// No deletes, no replacing modes.
    Deny,
    /// OS trash or the app trash.
    Trash,
    /// Gone for good.
    Permanent,
}

/// Effective permissions of the requester on a share. Hints for the peer's UI only:
/// every request is checked again on the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectivePermissions {
    /// List folders and see metadata.
    pub browse: bool,
    /// Pull files.
    pub download: bool,
    /// Push files and create folders.
    pub upload: bool,
    /// Delete requests and replacing conflict modes.
    pub delete: DeleteMode,
}

/// A share visible to the requester.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareInfo {
    /// Share id.
    pub share_id: ShareId,
    /// Name shown to peers.
    pub name: String,
    /// Requester's effective permissions.
    pub permissions: EffectivePermissions,
}

/// One entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    /// File or folder name (one component).
    pub name: String,
    /// Kind.
    pub kind: EntryKind,
    /// Size in bytes (0 for folders).
    pub size: u64,
    /// Modification time, Unix milliseconds.
    pub mtime_unix_ms: i64,
}

/// Result of a `Stat` for one path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatEntry {
    /// Requested path.
    pub path: String,
    /// Metadata, or `None` if the path does not exist.
    pub entry: Option<DirEntry>,
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    /// Share the hit is in.
    pub share_id: ShareId,
    /// Relative path inside the share.
    pub path: String,
    /// Kind.
    pub kind: EntryKind,
    /// Size in bytes.
    pub size: u64,
    /// Modification time, Unix milliseconds.
    pub mtime_unix_ms: i64,
}

/// Outcome of deleting one path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathResult {
    /// Requested path.
    pub path: String,
    /// `None` on success, otherwise a short error description.
    pub error: Option<String>,
}

/// Opaque pagination cursor issued by the owner.
pub type Cursor = Vec<u8>;

/// A remote request, one per request stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    /// Shares whose audience includes the requester.
    ListShares,
    /// One page of a folder (needs `browse`).
    ListDir {
        /// Share id.
        share_id: ShareId,
        /// Relative folder path; empty = share root.
        path: String,
        /// Cursor from the previous page.
        cursor: Option<Cursor>,
    },
    /// Metadata of some paths (needs `browse`).
    Stat {
        /// Share id.
        share_id: ShareId,
        /// Relative paths.
        paths: Vec<String>,
    },
    /// Case-insensitive name search (needs `browse` on each share).
    Search {
        /// Shares to search; empty = all browsable shares.
        share_ids: Vec<ShareId>,
        /// Substring to match in file and folder names.
        query: String,
        /// Cursor from the previous page.
        cursor: Option<Cursor>,
    },
    /// Pull files or folders (needs `browse` + `download`).
    Download {
        /// Share id.
        share_id: ShareId,
        /// Relative paths.
        paths: Vec<String>,
    },
    /// Delete files or folders (needs `delete ≠ Deny`).
    Delete {
        /// Share id.
        share_id: ShareId,
        /// Relative paths.
        paths: Vec<String>,
    },
    /// Create a folder (needs `upload`).
    MakeDir {
        /// Share id.
        share_id: ShareId,
        /// Relative path of the new folder.
        path: String,
    },
}

/// Answer to a [`Request`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    /// Answer to `ListShares`.
    Shares(Vec<ShareInfo>),
    /// Answer to `ListDir`.
    DirPage {
        /// Entries, folders first.
        entries: Vec<DirEntry>,
        /// Cursor for the next page, `None` on the last page.
        next_cursor: Option<Cursor>,
    },
    /// Answer to `Stat`.
    Stats(Vec<StatEntry>),
    /// Answer to `Search`.
    SearchPage {
        /// Hits.
        hits: Vec<SearchHit>,
        /// Cursor for the next page, `None` when done.
        next_cursor: Option<Cursor>,
    },
    /// Answer to `Download`: the owner queued a job to the requester.
    DownloadStarted {
        /// The new job.
        job_id: JobId,
    },
    /// Answer to `Delete`.
    DeleteResult(Vec<PathResult>),
    /// Success without data (`MakeDir`).
    Ok,
    /// Failure with a short description (`MakeDir`).
    Error(String),
    /// The requester lacks the permission.
    Denied,
    /// The share or path does not exist.
    NotFound,
}
