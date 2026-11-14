# Wire protocol (v1)

Transport: one QUIC connection per peer (ALPN `localdrop/1`), established after
discovery and authenticated by the membership handshake ([security.md](security.md)).
The connection is symmetric: both sides can send jobs and make requests.

> The message types in `localdrop-protocol` were written before jobs, bundles and
> shares were designed and will be reworked to match this document in M3 (see
> [roadmap.md](roadmap.md)).

## Streams

| Stream | Opened by | Content |
|---|---|---|
| Handshake | client, first bidi | SPAKE2 messages + key confirmation |
| Control | client, second bidi | framed `ControlMessage`s, both directions, for the whole connection |
| Unit data | sender of a job, uni | one per [unit](transfers.md#jobs-and-units): framed header, then entries |
| Request | requester, bidi | one per remote request (list, delete, …): request frame → response frame(s) |

Each side runs one outgoing pair queue, so at most **one unit data stream per direction**
is open at a time ([transfers.md](transfers.md#pair-queues)).

## Framing

```text
frame = u32_be(len) || postcard(message)      len ≤ 1 MiB
```

`encode_frame` / `decode_frame` are incremental. Large lists (manifests, directory
listings) are split across several frames.

## Control messages

### Session

| Message | Meaning |
|---|---|
| `Hello{protocol_version, device_id, device_name, capabilities}` | First message from both sides. `device_id` must match the authenticated key. |
| `Ping` / `Pong` | Liveness. |

### Jobs

| Message | From | Meaning |
|---|---|---|
| `JobOffer{job_id, name, destination, total_files, total_bytes, deferred}` | sender | Proposes a job. `destination = Inbox \| Share{share_id, path}`. `deferred` marks jobs from the offline queue (go to `deferred_inbox_dir` when `Inbox`). |
| `ManifestChunk{job_id, entries[], last}` | sender | Manifest: `{index, path, kind: File\|Dir, size, mtime}`. Paths relative, `/`-separated. |
| `JobPending{job_id, reason: AwaitingAccept\|AwaitingConflictDecision\|LowSpace{free, needed}}` | receiver | Receiver waits for a human (accept requests time out after 10 min, see [security.md](security.md#accept-policy)) or for free space ([transfers.md](transfers.md#running-out-of-space)). Sender pauses the job and serves other jobs. A later `JobAccept{resume}` continues it. |
| `ConflictReport{job_id, conflicts[], allowed_modes[]}` | receiver | Files that already exist (and differ), and the modes the owner's permissions allow. Shown in the sender's dialog. |
| `ConflictDecision{job_id, mode}` | either | First valid decision wins; the receiver confirms with `JobAccept`. Either side may also answer `JobCancel`. |
| `JobAccept{job_id, skip[], resume}` | receiver | Start/continue sending. `skip`: manifest indices not to send (identical or `KeepExisting`). `resume`: `{unit, file_index, segment}` to continue from. |
| `SegmentFailed{job_id, file_index, segment}` | receiver | Segment hash mismatch; sender re-sends that segment after the current unit (as a one-segment large unit). |
| `JobReject{job_id, reason}` | receiver | `Declined`, `NoAnswer` (accept request timed out), `NoSpace`, `Denied` (no permission), `Other`. |
| `FileSkipped{job_id, file_index, reason}` | sender | The sender could not send a file of the current unit: `SourceChanged`, `SourceLocked`, `SourceMissing`, `ReadError`. The receiver drops its temp file; the rest of the unit continues. |
| `UnitDone{job_id, unit, files_ok[], files_failed[{index, reason}]}` | receiver | Unit written, synced and verified. `reason`: `HashMismatch` (re-sent by the sender, up to 3 times), `TargetLocked`, `WriteError`. |
| `FileRetry{job_id, file_indices[]}` | sender | User pressed *Retry* for failed files; they are sent as a new unit at the end of the job. |
| `JobDone{job_id}` | receiver | All files written; mirror deletions (if any) done. |
| `JobPause` / `JobResume` / `JobCancel {job_id}` | either | User actions from the queue UI. Cancel is final on both sides. |
| `Progress{job_id, bytes_written}` | receiver | Optional, ~4 per second, so the sender shows the real written progress. |

Receiver state is keyed by `job_id`, so a deferred or interrupted job that is offered
again resumes where it stopped and decisions made while the sender was away
(accept, conflict mode) are applied immediately.

## Unit data streams

```text
uni stream:
  frame(UnitHeader{job_id, unit, kind: Large|Bundle, first_segment})
  entry*
entry:
  frame(EntryHeader{index, offset, len})
  segment*                                    // offset is always a segment boundary
  frame(EntryTrailer{index, content_hash})
segment:
  bytes[min(SEGMENT_LEN, remaining)]          // SEGMENT_LEN = 64 MiB, raw bytes, no framing
  frame(SegmentTrailer{segment, blake3})      // hash of this segment only
```

- `content_hash = BLAKE3-derive-key("localdrop/v1/content", seg_hash_0 || … || seg_hash_n)`.
  A small file is one segment.
- A **large unit** has one entry for one file. On resume `first_segment > 0`: the sender
  seeks to `first_segment × SEGMENT_LEN`, the receiver continues the same partial file
  using its stored hashes of the already verified segments (no re-hashing of the prefix).
- After each `SegmentTrailer` matches, the receiver flushes that range and persists
  "verified up to segment n". A mismatching segment is reported in `SegmentFailed` and
  re-sent; the stream continues with the following segments meanwhile.
- A **bundle** has many entries back to back, each a whole small file — the
  "in-memory archive": no compression, no temp files, streamed both ways.
- Entries appear in manifest order; skipped indices are simply absent.
- The receiver writes each entry to a hidden `.<name>.ldpart` next to its target, checks
  the trailer hash, then renames it into place (applying the conflict mode); see
  [receiving.md](receiving.md). After the last entry it
  syncs and sends `UnitDone`.

## Remote requests (shares)

Each request uses its own short-lived bidi stream, independent of the queues, so
browsing stays responsive during a large transfer.

| Request | Response | Permission |
|---|---|---|
| `ListShares` | `Shares[{share_id, name, effective_permissions}]` | — |
| `ListDir{share_id, path, cursor}` | `DirPage{entries[{name, kind, size, mtime}], next_cursor}` | `browse` |
| `Stat{share_id, paths[]}` | `Stats[...]` | `browse` |
| `Search{share_ids[], query, cursor}` | `SearchPage{hits[{share_id, path, kind, size, mtime}], next_cursor}` | `browse` on each share |
| `Download{share_id, paths[]}` | `DownloadStarted{job_id}` — the owner creates a job to the requester in its outgoing queue | `browse` + `download` |
| `Delete{share_id, paths[]}` | `DeleteResult[{path, ok \| error}]` | `delete ≠ Deny`; the owner applies `Trash`/`Permanent` |
| `MakeDir{share_id, path}` | `Ok \| Error` | `upload` |

`Search` matches **file and folder names only** (case-insensitive substring), runs on
the owner, and only inside shares the requester may browse; an empty `share_ids` means
all of them. The owner caps work per request (time and hits per page) so a search can't
stall it.

Any request can be answered with `Denied` or `NotFound`. Uploads are not a request:
they are a normal job with `destination = Share{..}`.

## File names and paths

Peer-supplied names and paths are sanitized with `sanitize_file_name` (no absolute
paths, no `..`, no reserved Windows names or invalid characters) and then resolved
inside the destination root as described in [shares.md](shares.md#path-safety).
Name collisions caused by sanitizing (e.g. `a:b` and `ab`) are handled like conflicts.

## Limits

| Limit | Value |
|---|---|
| Frame payload | 1 MiB |
| Manifest entries per job | 1 000 000 |
| Path length | 1 024 bytes (UTF-8), 255 per component |
| Large-file threshold / bundle target | 100 MiB / ~100 MiB or 10 000 files |
| Segment length | 64 MiB |
| Maximum file size | 2⁶³ bytes (protocol); in practice limited by the destination file system |
| Directory page | 1 000 entries |

## Versioning

- `PROTOCOL_VERSION` in `Hello`; peers with different major versions refuse to connect
  and the UI asks to update.
- `capabilities` (bit set) allows optional features without a version bump.
- Announcement format and key derivation are versioned separately (announcement byte 4,
  `v1` in HKDF labels).
