//! Building a job manifest from local files and folders.
//!
//! Nothing is read or hashed here: hashes are computed while sending
//! (`docs/transfers.md#performance`). Only metadata is collected, so queueing a huge
//! folder is fast.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use localdrop_core::{Error, Result};
use localdrop_protocol::{EntryKind, MAX_MANIFEST_ENTRIES, ManifestEntry, check_path_len};
use localdrop_store::JobEntry;

/// The result of scanning the sources of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scanned {
    pub name: String,
    pub entries: Vec<JobEntry>,
    pub total_bytes: u64,
}

/// Scans files and folders. Each source becomes a top-level item named after itself;
/// folders keep their structure. Symbolic links are skipped, never followed.
pub(crate) fn scan(sources: &[PathBuf]) -> Result<Scanned> {
    if sources.is_empty() {
        return Err(Error::Io(std::io::Error::other("nothing to send")));
    }
    let mut out = Vec::new();
    let mut top_names = Vec::new();
    for src in sources {
        let path = fs::canonicalize(src)?;
        let name = file_name(&path)?;
        if top_names.contains(&name) {
            return Err(Error::Io(std::io::Error::other(format!(
                "two items are both named \"{name}\""
            ))));
        }
        top_names.push(name.clone());
        walk(&path, &name, &mut out)?;
    }
    let name = match top_names.as_slice() {
        [one] => one.clone(),
        [first, rest @ ..] => format!("{first} and {} more", rest.len()),
        [] => unreachable!("sources is not empty"),
    };
    let total_bytes = out.iter().map(|e| e.manifest.size).sum();
    Ok(Scanned {
        name,
        entries: out,
        total_bytes,
    })
}

fn walk(path: &Path, rel: &str, out: &mut Vec<JobEntry>) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    let kind = if meta.file_type().is_symlink() {
        return Ok(());
    } else if meta.is_dir() {
        EntryKind::Dir
    } else if meta.is_file() {
        EntryKind::File
    } else {
        return Ok(()); // sockets, devices, …
    };
    check_path_len(rel)?;
    if out.len() >= MAX_MANIFEST_ENTRIES as usize {
        return Err(Error::Protocol(format!(
            "a job can hold at most {MAX_MANIFEST_ENTRIES} files"
        )));
    }
    out.push(JobEntry {
        manifest: ManifestEntry {
            index: out.len() as u32,
            path: rel.to_owned(),
            kind,
            size: if kind == EntryKind::File {
                meta.len()
            } else {
                0
            },
            mtime_unix_ms: mtime_ms(&meta),
            executable: is_executable(&meta),
        },
        local_path: Some(path.to_owned()),
    });
    if kind == EntryKind::Dir {
        let mut children: Vec<_> = fs::read_dir(path)?.collect::<std::io::Result<_>>()?;
        children.sort_by_key(|c| c.file_name());
        for child in children {
            let child_name = child.file_name().to_string_lossy().into_owned();
            walk(&child.path(), &format!("{rel}/{child_name}"), out)?;
        }
    }
    Ok(())
}

fn file_name(path: &Path) -> Result<String> {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or(Error::NotFound)
}

fn mtime_ms(meta: &fs::Metadata) -> i64 {
    match meta.modified().map(|t| t.duration_since(UNIX_EPOCH)) {
        Ok(Ok(d)) => d.as_millis() as i64,
        Ok(Err(before_epoch)) => -(before_epoch.duration().as_millis() as i64),
        Err(_) => 0,
    }
}

#[cfg(unix)]
fn is_executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.is_file() && meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_meta: &fs::Metadata) -> bool {
    false
}

/// Copies the job's files into `dir`, keeping relative paths, and points the entries
/// at the copies (`OutboxStorage::CopyToCache`).
pub(crate) fn copy_to_cache(entries: &mut [JobEntry], dir: &Path) -> Result<()> {
    for e in entries {
        let target = e
            .manifest
            .path
            .split('/')
            .fold(dir.to_path_buf(), |p, c| p.join(c));
        match e.manifest.kind {
            EntryKind::Dir => fs::create_dir_all(&target)?,
            EntryKind::File => {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                let src = e.local_path.as_ref().ok_or(Error::NotFound)?;
                fs::copy(src, &target)?;
            }
            EntryKind::Symlink => continue,
        }
        e.local_path = Some(target);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn scans_files_and_folders() {
        let root = tempdir();
        fs::create_dir_all(root.join("Holiday/day 1")).unwrap();
        fs::create_dir_all(root.join("Holiday/empty")).unwrap();
        fs::write(root.join("Holiday/day 1/a.jpg"), b"aaaa").unwrap();
        fs::write(root.join("Holiday/b.jpg"), b"bb").unwrap();
        fs::write(root.join("notes.txt"), b"n").unwrap();

        let s = scan(&[root.join("Holiday"), root.join("notes.txt")]).unwrap();
        assert_eq!(s.name, "Holiday and 1 more");
        assert_eq!(s.total_bytes, 7);
        let paths: Vec<_> = s
            .entries
            .iter()
            .map(|e| (e.manifest.index, e.manifest.path.as_str(), e.manifest.kind))
            .collect();
        assert_eq!(
            paths,
            [
                (0, "Holiday", EntryKind::Dir),
                (1, "Holiday/b.jpg", EntryKind::File),
                (2, "Holiday/day 1", EntryKind::Dir),
                (3, "Holiday/day 1/a.jpg", EntryKind::File),
                (4, "Holiday/empty", EntryKind::Dir),
                (5, "notes.txt", EntryKind::File),
            ]
        );
        assert!(s.entries.iter().all(|e| e.manifest.mtime_unix_ms > 0));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn single_item_and_errors() {
        let root = tempdir();
        fs::write(root.join("a.txt"), b"x").unwrap();
        assert_eq!(scan(&[root.join("a.txt")]).unwrap().name, "a.txt");
        assert!(scan(&[]).is_err());
        assert!(scan(&[root.join("missing")]).is_err());
        fs::create_dir_all(root.join("other")).unwrap();
        fs::write(root.join("other/a.txt"), b"y").unwrap();
        assert!(scan(&[root.join("a.txt"), root.join("other/a.txt")]).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cache_copy_keeps_structure() {
        let root = tempdir();
        fs::create_dir_all(root.join("src/sub")).unwrap();
        fs::write(root.join("src/sub/f.bin"), b"data").unwrap();
        let mut s = scan(&[root.join("src")]).unwrap();
        copy_to_cache(&mut s.entries, &root.join("cache")).unwrap();
        let copied = root.join("cache/src/sub/f.bin");
        assert_eq!(fs::read(&copied).unwrap(), b"data");
        assert_eq!(s.entries[2].local_path.as_deref(), Some(copied.as_path()));
        fs::remove_dir_all(root).unwrap();
    }

    pub(crate) fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "localdrop-test-{}",
            localdrop_core::JobId::random()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
