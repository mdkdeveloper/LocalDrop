//! LocalDrop wire protocol (see `docs/protocol.md`).
//!
//! A connection between two authenticated group members carries:
//! - one bidirectional **control stream** with [`ControlMessage`]s, framed by [`codec`];
//! - one unidirectional **data stream per file**, starting with a framed
//!   [`FileStreamHeader`] followed by raw file bytes.

pub mod codec;
pub mod messages;
pub mod names;

pub use codec::{MAX_FRAME_LEN, decode_frame, encode_frame};
pub use messages::{
    ControlMessage, FileMeta, FileResume, FileStreamHeader, PROTOCOL_VERSION, RejectReason,
};
pub use names::sanitize_file_name;
