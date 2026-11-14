# Receiving: writing, finalizing and recovering files

How the receiver turns incoming bytes into files. The rule: **a file appears under its
real name only when it is complete, verified and on disk.** Until then it is a temporary
file that the user and other apps do not see.

## Temporary files

### Where

The temporary file is created **in the final directory, next to the target**:

```text
<dest dir>/.<name>.ldpart          e.g.  Photos/2024/.IMG_0001.jpg.ldpart
```

Why next to the target and not in a separate `.localdrop/partial/` folder:

| Concern | Next to target | Separate staging folder |
|---|---|---|
| Final rename is atomic and instant | ✔ always same directory → same volume | ✘ subfolders may be other volumes (mount points, junctions) → copy instead of rename |
| Android SAF | ✔ `renameDocument` in one directory works with most providers | ✘ `moveDocument` between directories is optional for providers |
| Works for the inbox and for shares alike | ✔ | needs a staging folder per volume |
| Clutter | hidden name; one file per in-flight file only | ✔ nothing in user folders |

With one unit in flight per peer, there are at most a few hundred `.ldpart` files at a
time (one bundle), usually just one.

### Naming and hiding

- Name: `.` + final name + `.ldpart`. If that exceeds 255 bytes, use
  `.ld-<job_id>-<index>.ldpart`.
- Hidden: leading dot (Linux, macOS, Android, iOS) plus `FILE_ATTRIBUTE_HIDDEN` on
  Windows; on macOS also `UF_HIDDEN`.
- Windows: also `FILE_ATTRIBUTE_NOT_CONTENT_INDEXED` so the search indexer ignores it.
- `.ldpart` files are excluded from share listings, conflict checks, and "identical file"
  comparisons, and can never be targeted by remote requests.
- Android **MediaStore** (the default inbox `Download/LocalDrop`): instead of a dot file
  the entry is created with `IS_PENDING = 1` under its final name; MediaStore hides it from
  other apps until it is cleared. This is the platform's own "temp then publish".

### Cloud sync folders

If the destination is inside Dropbox/OneDrive/iCloud/Google Drive, their clients may
start uploading the partial file. Hidden files are skipped by some clients but not all.
The app detects well-known sync roots and shows a hint; it does not try to work around
the sync client.

## Lifecycle of one file

```text
 manifest accepted
        │
        ▼
 create .name.ldpart (exclusive create), pre-allocate size
        │
        ▼
 write chunks ──▶ hash per segment ──▶ at each 64 MiB: verify, flush range,
        │                              persist "verified up to segment n"
        ▼
 EntryTrailer: content hash matches
        │
        ▼
 set mtime from manifest (and exec bit on Unix if sent)
        │
        ▼
 file flushed (fsync / FlushFileBuffers) ── batched per unit, see below
        │
        ▼
 apply conflict mode, then rename .name.ldpart → name   (atomic)
        │
        ▼
 directory entry flushed (fsync dir, Unix) ── batched per unit
        │
        ▼
 counted as done; UnitDone sent at the end of the unit
```

### Writing while receiving

- The writer runs on the blocking pool and receives 1 MiB chunks from the network task
  through a bounded channel (backpressure, see [transfers.md](transfers.md#large-files-multi-gb)).
- Writes are sequential, at increasing offsets; the file is pre-allocated so a full disk
  is detected at create time.
- The temp file is opened with exclusive create (`O_EXCL` / `CREATE_NEW`) so it never
  clobbers anything, and with share mode "read" on Windows so antivirus can scan it.

### Finalizing a unit (batched)

Flushing every small file one by one would be slow (thousands of `fsync`s). Per unit:

1. All files of the unit have been written and verified.
2. Flush all their temp files (in parallel on the blocking pool).
3. Rename all of them to their final names (in manifest order).
4. Flush each touched directory once (Unix; on Windows the rename is journaled by NTFS).
5. Persist "unit done" in the job state, then send `UnitDone`.

Flushing data *before* renaming matters: renaming first could leave a zero-length or
truncated file under the real name after a power loss (known ext4/APFS behaviour with
delayed allocation).

For a large unit the same steps happen for a single file; segment checkpoints during
the transfer already flushed most of it, so the final flush is short.

### The rename

| Platform | Replace existing target | Notes |
|---|---|---|
| Linux / Android (files) | `rename()` — atomic replace | `renameat2(RENAME_NOREPLACE)` when the target must not exist |
| macOS / iOS | `rename()` / `renamex_np(RENAME_EXCL)` | |
| Windows | `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` (`std::fs::rename`) | Fails if another process has the target open without delete sharing → see "Locked targets" |
| Android SAF | `DocumentsContract.renameDocument` | Provider may append ` (1)` if the name exists → delete or back up the old document first, then rename, then verify the resulting name |
| Android MediaStore | update `IS_PENDING = 0` (and name) | |

When the target **must not exist** (new file, `KeepExisting`, "keep both"), the no-replace
variant is used; if a file appeared meanwhile, it is handled as a late conflict
(` (1)` name, or according to the conflict mode).

## Interaction with conflict modes

| Mode | Steps at finalize | Crash window handled by |
|---|---|---|
| New file / `KeepExisting` | rename no-replace | — |
| `ReplaceConflicting` (delete = Permanent) | rename with replace (atomic) | — |
| `ReplaceConflicting` (delete = Trash) | move old to trash, then rename | journal |
| `ReplaceWithBackup` | move old to backup set, then rename | journal |
| `Mirror` | as above per file; after `JobDone`, remove files not in the manifest | journal |
| "keep both" (inbox) | rename no-replace to `name (n).ext` | — |

Moving the old file away and renaming the new one into place are two steps. Before them
the receiver appends an intent to the **job journal** (`backup old → path`, `rename tmp →
target`); on startup an unfinished journal is completed or rolled back, so a crash
never loses both versions.

## Locked targets (mainly Windows)

If the target is open in another program (e.g. a document in Word), it cannot be
replaced:

- retry with backoff for up to ~5 s (covers antivirus and indexer);
- then the file is marked `Locked`, the rest of the job continues, and the UI offers
  *Retry* or *Keep both*. The verified `.ldpart` is kept until the user decides.

## Names on the receiving file system

Beyond [sanitizing](protocol.md#file-names-and-paths):

- **Case-insensitive file systems** (Windows, macOS, Android shared storage): manifest
  entries that differ only by case (`Readme.md` and `README.md` from Linux) collide; the
  second becomes `README (1).md` and the user sees a note in the job details.
- **Unicode normalization**: names are compared in NFC (macOS may store NFD), so the same
  name typed on different systems is recognized as the same file.
- **Long paths on Windows**: verbatim `\\?\` paths are used so paths > 260 chars work.
- **Metadata**: mtime is preserved; creation time, ACLs, owners and extended attributes
  are not transferred. On Unix the executable bit is carried by the manifest.

## Resume, cancel, crash recovery

The persistent job state on the receiver stores per file: temp path, verified segment
count, segment hashes, state (`Writing`, `Verified`, `Finalized`, `Locked`).

| Event | What happens to `.ldpart` |
|---|---|
| Disconnect / pause | Kept. On resume, truncated to the last verified segment boundary, writing continues there. |
| Receiver restart / crash | Same as above; if the temp file is missing or shorter than the verified length, the file restarts from what is actually there. |
| Hash mismatch | Bad segment is overwritten in place when re-sent (the file is pre-allocated). |
| Cancel | Temp files of the job deleted; already finalized files stay. |
| Job finished | No temp files remain. |
| Orphans | On startup, temp files recorded in job states of jobs that no longer exist are deleted. Unknown `.ldpart` files are never deleted automatically (they might belong to another install); the receive-folder settings show them with a "clean up" action. |

## Opening files while they are being received

Not supported: the file only becomes visible when complete. This keeps other apps (photo
galleries, media scanners, sync clients) from processing half-written files. A future
option could expose a read-only "preview while receiving" for media inside the app.
