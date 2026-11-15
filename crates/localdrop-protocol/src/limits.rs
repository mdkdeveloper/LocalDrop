//! Protocol limits and sizes (see `docs/protocol.md#limits`), plus helpers that enforce
//! them: path validation, the content hash and manifest splitting.

use localdrop_core::{Error, JobId, Result};

use crate::{ControlMessage, ManifestEntry};

const MIB: u64 = 1024 * 1024;

/// Files of at least this size are sent as their own unit.
pub const LARGE_FILE_THRESHOLD: u64 = 100 * MIB;
/// Target size of a bundle of small files.
pub const BUNDLE_TARGET: u64 = 100 * MIB;
/// Maximum number of files in one bundle.
pub const BUNDLE_MAX_FILES: usize = 10_000;
/// Length of a segment; every segment is followed by a [`SegmentTrailer`](crate::SegmentTrailer).
pub const SEGMENT_LEN: u64 = 64 * MIB;
/// Maximum number of manifest entries in one job.
pub const MAX_MANIFEST_ENTRIES: u32 = 1_000_000;
/// Maximum length of a relative path in bytes (UTF-8).
pub const MAX_PATH_LEN: usize = 1024;
/// Maximum length of one path component in bytes (UTF-8).
pub const MAX_COMPONENT_LEN: usize = 255;
/// Maximum entries in one directory page.
pub const DIR_PAGE_LEN: usize = 1000;
/// Encoded-size budget for one manifest chunk, well below `MAX_FRAME_LEN`.
pub const MANIFEST_CHUNK_BYTES: usize = 512 * 1024;

/// Checks a peer-supplied relative path against the length limits.
///
/// This does not sanitize; use [`sanitize_file_name`](crate::sanitize_file_name) for that.
pub fn check_path_len(path: &str) -> Result<()> {
    if path.len() > MAX_PATH_LEN {
        return Err(Error::Protocol("path too long".into()));
    }
    if path.split('/').any(|c| c.len() > MAX_COMPONENT_LEN) {
        return Err(Error::Protocol("path component too long".into()));
    }
    Ok(())
}

/// Number of segments of a file of `size` bytes (a small or empty file is one segment).
pub fn segment_count(size: u64) -> u64 {
    size.div_ceil(SEGMENT_LEN).max(1)
}

/// The content hash of a file from its segment hashes:
/// `BLAKE3-derive-key("localdrop/v1/content", seg_hash_0 || seg_hash_1 || …)`.
pub fn content_hash<'a>(segment_hashes: impl IntoIterator<Item = &'a [u8; 32]>) -> [u8; 32] {
    let mut h = blake3::Hasher::new_derive_key("localdrop/v1/content");
    for seg in segment_hashes {
        h.update(seg);
    }
    *h.finalize().as_bytes()
}

/// Splits a manifest into [`ControlMessage::ManifestChunk`]s that each fit in a frame.
///
/// Always returns at least one chunk (an empty job still sends `last = true`).
pub fn manifest_chunks(job_id: JobId, entries: Vec<ManifestEntry>) -> Vec<ControlMessage> {
    // Per-entry overhead: index, kind, size, mtime, flag and the path length prefix
    // are at most ~30 bytes as postcard varints.
    const ENTRY_OVERHEAD: usize = 32;
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut bytes = 0;
    for e in entries {
        let cost = e.path.len() + ENTRY_OVERHEAD;
        if !current.is_empty() && bytes + cost > MANIFEST_CHUNK_BYTES {
            chunks.push(std::mem::take(&mut current));
            bytes = 0;
        }
        bytes += cost;
        current.push(e);
    }
    chunks.push(current);
    let n = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, entries)| ControlMessage::ManifestChunk {
            job_id,
            entries,
            last: i + 1 == n,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntryKind, MAX_FRAME_LEN, encode_frame};

    fn entry(index: u32, path: String) -> ManifestEntry {
        ManifestEntry {
            index,
            path,
            kind: EntryKind::File,
            size: u64::MAX,
            mtime_unix_ms: i64::MIN,
            executable: true,
        }
    }

    #[test]
    fn path_limits() {
        assert!(check_path_len("a/b/c.txt").is_ok());
        assert!(check_path_len(&"a".repeat(MAX_COMPONENT_LEN)).is_ok());
        assert!(check_path_len(&"a".repeat(MAX_COMPONENT_LEN + 1)).is_err());
        let long = vec!["a".repeat(200); 6].join("/");
        assert!(check_path_len(&long).is_err());
    }

    #[test]
    fn segments() {
        assert_eq!(segment_count(0), 1);
        assert_eq!(segment_count(1), 1);
        assert_eq!(segment_count(SEGMENT_LEN), 1);
        assert_eq!(segment_count(SEGMENT_LEN + 1), 2);
        assert_eq!(segment_count(10_000_000_000), 150);
    }

    #[test]
    fn content_hash_is_keyed_over_segments() {
        let s0 = *blake3::hash(b"part one").as_bytes();
        let s1 = *blake3::hash(b"part two").as_bytes();
        let two = content_hash([&s0, &s1]);
        assert_ne!(two, content_hash([&s0]));
        assert_ne!(content_hash([&s0]), s0, "derive-key context must apply");
        let mut joined = s0.to_vec();
        joined.extend_from_slice(&s1);
        assert_eq!(two, blake3::derive_key("localdrop/v1/content", &joined));
    }

    #[test]
    fn manifest_chunks_fit_in_frames() {
        let job = JobId::random();
        let path = vec!["x".repeat(MAX_COMPONENT_LEN); 4].join("/");
        let entries: Vec<_> = (0..5_000).map(|i| entry(i, path.clone())).collect();
        let chunks = manifest_chunks(job, entries);
        assert!(chunks.len() > 1);
        let mut seen = 0;
        for (i, c) in chunks.iter().enumerate() {
            assert!(encode_frame(c).unwrap().len() <= MAX_FRAME_LEN + 4);
            let ControlMessage::ManifestChunk { entries, last, .. } = c else {
                panic!("not a chunk");
            };
            assert_eq!(*last, i + 1 == chunks.len());
            seen += entries.len();
        }
        assert_eq!(seen, 5_000);
    }

    #[test]
    fn empty_manifest_has_one_last_chunk() {
        let chunks = manifest_chunks(JobId::random(), vec![]);
        assert!(matches!(
            chunks.as_slice(),
            [ControlMessage::ManifestChunk { last: true, .. }]
        ));
    }
}
