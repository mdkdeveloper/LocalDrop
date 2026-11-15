//! Length-prefixed `postcard` framing: `u32 BE length || postcard(message)`.

use localdrop_core::{Error, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Maximum size of a single frame payload (1 MiB). Larger frames are a protocol error.
pub const MAX_FRAME_LEN: usize = 1024 * 1024;

/// Encodes a message into a frame.
pub fn encode_frame<T: Serialize>(msg: &T) -> Result<Vec<u8>> {
    let body = postcard::to_stdvec(msg).map_err(|e| Error::Protocol(e.to_string()))?;
    if body.len() > MAX_FRAME_LEN {
        return Err(Error::Protocol("frame too large".into()));
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Tries to decode one frame from the start of `buf`.
///
/// Returns `Ok(None)` if more bytes are needed, or `Ok(Some((msg, consumed)))`.
pub fn decode_frame<T: DeserializeOwned>(buf: &[u8]) -> Result<Option<(T, usize)>> {
    let Some(len_bytes) = buf.get(..4) else {
        return Ok(None);
    };
    let len = u32::from_be_bytes(len_bytes.try_into().expect("4 bytes")) as usize;
    if len > MAX_FRAME_LEN {
        return Err(Error::Protocol("frame too large".into()));
    }
    let Some(body) = buf.get(4..4 + len) else {
        return Ok(None);
    };
    let msg = postcard::from_bytes(body).map_err(|e| Error::Protocol(e.to_string()))?;
    Ok(Some((msg, 4 + len)))
}

#[cfg(test)]
mod tests {
    use localdrop_core::{DeviceId, JobId, ShareId};
    use serde::Deserialize;

    use super::*;
    use crate::*;

    fn roundtrip_all<T>(msgs: &[T])
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let mut buf = Vec::new();
        for m in msgs {
            buf.extend(encode_frame(m).unwrap());
        }
        let mut pos = 0;
        let mut decoded = Vec::new();
        while let Some((m, n)) = decode_frame::<T>(&buf[pos..]).unwrap() {
            decoded.push(m);
            pos += n;
        }
        assert_eq!(pos, buf.len());
        assert_eq!(decoded, msgs);
    }

    #[test]
    fn control_roundtrip() {
        let job_id = JobId([9; 16]);
        roundtrip_all(&[
            ControlMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: DeviceId([3; 16]),
                device_name: "laptop".into(),
                capabilities: 0,
            },
            ControlMessage::JobOffer {
                job_id,
                name: "Holiday photos".into(),
                destination: Destination::Share {
                    share_id: ShareId([1; 16]),
                    path: "2024/summer".into(),
                },
                total_files: 2,
                total_bytes: 5,
                deferred: true,
            },
            ControlMessage::ManifestChunk {
                job_id,
                entries: vec![
                    ManifestEntry {
                        index: 0,
                        path: "a".into(),
                        kind: EntryKind::Dir,
                        size: 0,
                        mtime_unix_ms: 1,
                        executable: false,
                    },
                    ManifestEntry {
                        index: 1,
                        path: "a/b.txt".into(),
                        kind: EntryKind::File,
                        size: 5,
                        mtime_unix_ms: -1,
                        executable: true,
                    },
                ],
                last: true,
            },
            ControlMessage::JobPending {
                job_id,
                reason: PendingReason::LowSpace { free: 1, needed: 2 },
            },
            ControlMessage::ConflictReport {
                job_id,
                conflicts: vec![Conflict {
                    index: 1,
                    existing_size: 3,
                    existing_mtime_unix_ms: 4,
                }],
                allowed_modes: vec![ConflictMode::KeepExisting, ConflictMode::ReplaceWithBackup],
                last: true,
            },
            ControlMessage::ConflictDecision {
                job_id,
                mode: ConflictMode::Mirror,
            },
            ControlMessage::JobAccept {
                job_id,
                skip: vec![0],
                resume: Some(ResumePoint {
                    unit: 1,
                    file_index: 1,
                    segment: 160,
                }),
            },
            ControlMessage::SegmentFailed {
                job_id,
                file_index: 1,
                segment: 3,
            },
            ControlMessage::FileSkipped {
                job_id,
                file_index: 1,
                reason: SkipReason::SourceLocked,
            },
            ControlMessage::UnitDone {
                job_id,
                unit: 0,
                files_ok: vec![0],
                files_failed: vec![FailedFile {
                    index: 1,
                    reason: FileFailReason::TargetLocked,
                }],
            },
            ControlMessage::FileRetry {
                job_id,
                file_indices: vec![1],
            },
            ControlMessage::Progress {
                job_id,
                bytes_written: 4,
            },
            ControlMessage::JobReject {
                job_id,
                reason: RejectReason::NoAnswer,
            },
            ControlMessage::JobPause { job_id },
            ControlMessage::JobResume { job_id },
            ControlMessage::JobCancel { job_id },
            ControlMessage::JobDone { job_id },
            ControlMessage::Ping,
            ControlMessage::Pong,
        ]);
    }

    #[test]
    fn unit_frames_roundtrip() {
        let job_id = JobId([2; 16]);
        roundtrip_all(&[UnitHeader {
            job_id,
            unit: 3,
            kind: UnitKind::Large,
            first_segment: 7,
        }]);
        roundtrip_all(&[EntryHeader {
            index: 1,
            offset: 7 * SEGMENT_LEN,
            len: 1,
        }]);
        roundtrip_all(&[SegmentTrailer {
            segment: 7,
            blake3: [5; 32],
        }]);
        roundtrip_all(&[EntryTrailer {
            index: 1,
            content_hash: [6; 32],
        }]);
    }

    #[test]
    fn requests_roundtrip() {
        let share_id = ShareId([4; 16]);
        roundtrip_all(&[
            Request::ListShares,
            Request::ListDir {
                share_id,
                path: String::new(),
                cursor: Some(vec![1, 2]),
            },
            Request::Stat {
                share_id,
                paths: vec!["a".into()],
            },
            Request::Search {
                share_ids: vec![],
                query: "img".into(),
                cursor: None,
            },
            Request::Download {
                share_id,
                paths: vec!["a".into()],
            },
            Request::Delete {
                share_id,
                paths: vec!["a".into()],
            },
            Request::MakeDir {
                share_id,
                path: "new".into(),
            },
        ]);
        let entry = DirEntry {
            name: "a".into(),
            kind: EntryKind::Symlink,
            size: 0,
            mtime_unix_ms: 0,
        };
        roundtrip_all(&[
            Response::Shares(vec![ShareInfo {
                share_id,
                name: "Photos".into(),
                permissions: EffectivePermissions {
                    browse: true,
                    download: true,
                    upload: false,
                    delete: DeleteMode::Trash,
                },
            }]),
            Response::DirPage {
                entries: vec![entry.clone()],
                next_cursor: None,
            },
            Response::Stats(vec![StatEntry {
                path: "a".into(),
                entry: Some(entry),
            }]),
            Response::SearchPage {
                hits: vec![SearchHit {
                    share_id,
                    path: "a/img.jpg".into(),
                    kind: EntryKind::File,
                    size: 1,
                    mtime_unix_ms: 2,
                }],
                next_cursor: Some(vec![3]),
            },
            Response::DownloadStarted {
                job_id: JobId([1; 16]),
            },
            Response::DeleteResult(vec![PathResult {
                path: "a".into(),
                error: Some("in use".into()),
            }]),
            Response::Ok,
            Response::Error("exists".into()),
            Response::Denied,
            Response::NotFound,
        ]);
    }

    #[test]
    fn partial_needs_more() {
        let frame = encode_frame(&ControlMessage::Pong).unwrap();
        assert!(
            decode_frame::<ControlMessage>(&frame[..2])
                .unwrap()
                .is_none()
        );
        assert!(
            decode_frame::<ControlMessage>(&frame[..frame.len() - 1])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn oversize_rejected() {
        let buf = (u32::MAX).to_be_bytes();
        assert!(decode_frame::<ControlMessage>(&buf).is_err());
        let huge = ControlMessage::FileRetry {
            job_id: JobId([0; 16]),
            file_indices: vec![u32::MAX; MAX_FRAME_LEN / 4],
        };
        assert!(encode_frame(&huge).is_err());
    }
}
