//! LocalDrop wire protocol v1 (see `docs/protocol.md`).
//!
//! A connection between two authenticated group members carries:
//! - one bidirectional **control stream** with [`ControlMessage`]s, framed by [`codec`];
//! - at most one unidirectional **unit data stream** per direction: a [`UnitHeader`], then
//!   entries of [`EntryHeader`], raw segments each followed by a [`SegmentTrailer`], and an
//!   [`EntryTrailer`];
//! - short-lived bidirectional **request streams** carrying one [`Request`] and its
//!   [`Response`]s.

pub mod codec;
pub mod limits;
pub mod messages;
pub mod names;

pub use codec::{MAX_FRAME_LEN, decode_frame, encode_frame};
pub use limits::{
    BUNDLE_MAX_FILES, BUNDLE_TARGET, DIR_PAGE_LEN, LARGE_FILE_THRESHOLD, MAX_COMPONENT_LEN,
    MAX_MANIFEST_ENTRIES, MAX_PATH_LEN, SEGMENT_LEN, check_path_len, content_hash, manifest_chunks,
    segment_count,
};
pub use messages::{
    Capabilities, Conflict, ConflictMode, ControlMessage, Cursor, DeleteMode, Destination,
    DirEntry, EffectivePermissions, EntryHeader, EntryKind, EntryTrailer, FailedFile,
    FileFailReason, ManifestEntry, PROTOCOL_VERSION, PathResult, PendingReason, RejectReason,
    Request, Response, ResumePoint, SearchHit, SegmentTrailer, ShareInfo, SkipReason, StatEntry,
    UnitHeader, UnitKind,
};
pub use names::sanitize_file_name;
