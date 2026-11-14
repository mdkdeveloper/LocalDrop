# Transfers: jobs, queues and performance

## Starting a transfer

| Platform | How the user sends |
|---|---|
| Windows / Linux / macOS | Drag & drop files **and folders** onto the app window or onto a device in the device list; "Send…" file/folder picker; OS integration later ("Send to" menu on Windows, Services/Finder extension on macOS). |
| Android | System **Share** sheet (`ACTION_SEND` / `ACTION_SEND_MULTIPLE`) → pick a device; in-app file picker; in-app **folder** picker (SAF `ACTION_OPEN_DOCUMENT_TREE`), because Share cannot pass folders. |
| iOS | **Share extension** → pick a device; in-app document picker (files and folders, security-scoped URLs). |

After choosing files the user picks:

- **target device** (online or known-offline);
- **destination**: the device's inbox (default) or one of its shares with `upload`
  permission ([shares.md](shares.md)) and a subfolder in it.

If the device is offline the job goes into the deferred queue
([deferred-send.md](deferred-send.md)); otherwise into the live queue. Both are the same
pair queue below — "deferred" just means it waits for the peer.

## Jobs and units

```text
Job  (what the user sees: "Holiday photos → Laptop", 3.2 GB, 1 820 files)
 └── Manifest: every file with relative path, size, mtime
 └── Units   (what goes over the wire, one at a time)
      ├── Large unit: one file ≥ 100 MiB
      ├── Bundle: many small files, ~100 MiB total
      └── ...
```

- A **job** is created by one user action (one drop, one Share, one download request).
  A folder keeps its structure: relative paths are preserved under the destination.
- The **manifest** is sent first. It lets the receiver check permissions, free space and
  [conflicts](shares.md#conflicts) for the whole job before any data moves, and drives
  progress for the whole job.
- **Units** are planned by the sender:
  - a file **≥ 100 MiB** (`LARGE_FILE_THRESHOLD`) is its own unit;
  - smaller files are packed in manifest order into **bundles** of up to ~100 MiB
    (`BUNDLE_TARGET`), or up to 10 000 files, whichever comes first;
  - empty folders are created from the manifest; zero-byte files need no data.
- Bundles are **not compressed and never written to disk**: the sender streams file
  after file into one data stream; the receiver splits the stream and writes each file
  directly to its destination. "Archive" here is just framing on the wire (see
  [protocol.md](protocol.md#data-streams)), so there is no tar/zip step, no temp file
  and no extra memory beyond the I/O buffers.

Why bundle at all: with one file in flight at a time, per-file round trips (open stream,
header, `FileDone`) dominate for thousands of small files. A bundle costs one stream and
one acknowledgement for hundreds of files.

## Pair queues

- There is **one outgoing queue per (this device → peer)** direction. A→B and B→A are
  independent and run at the same time (the link is full-duplex).
- In each queue **exactly one unit is in flight**. Jobs are processed in FIFO order;
  a job's units are sent in order.
- A download from a peer's share is a job in the **peer's** outgoing queue to us.
- Different peers are served in parallel (one in-flight unit per peer).
- A job paused for a decision (accept policy `Ask`, conflict dialog) is skipped; the
  queue continues with the next job and returns to it when the decision arrives.
- Jobs can be reordered by the user (move up / to top).

## Queue UI

For each peer, both directions:

- list of jobs with name, destination, files done / total, bytes done / total, speed,
  ETA, state (`Waiting for device`, `Waiting for decision`, `Sending`, `Paused`,
  `Paused · receiver low on space`, `No answer`, `Completed`, `Completed with errors`,
  `Failed`);
- a folder split into many units is **one row**; expanding shows files (optional);
- actions: **Cancel** job, Pause / Resume job, Retry failed, Move up, Clear completed.

### Job actions

Each job row has a **⋮** menu (desktop: also right-click; phone: a bottom sheet with
the job's name and progress on top). It lists only what fits the job's state;
destructive entries come last, in red:

| State | Actions |
|---|---|
| Sending / Receiving | Pause, Show details, Show in folder¹, **Cancel** |
| Next in queue / Waiting for device | Pause, Move to top, Show details, Show in folder¹, **Remove from queue** |
| Paused | Resume, Move to top, Show details, **Cancel** |
| Waiting for decision | Resolve (opens the conflict or accept dialog), Show details, **Cancel** |
| Paused · low on space | Show details, Open folder², Put the rest elsewhere…², **Cancel** |
| No answer | Offer again now, Show details, **Remove** |
| Completed with errors / Failed | Retry failed, Show details, Show in folder¹, Clear |
| Completed / Cancelled | Show in folder¹, Clear |

¹ Sender: where the files are sent from; receiver: where they landed.
² Receiver only.

*Pause* keeps the job's place in the queue; the peer serves its other jobs meanwhile.
*Move to top* puts the job right after the unit in flight (it is never interrupted).
Cancelling a running job asks nothing; finished files stay, the partial file is deleted.

**Transfer details** (click a job): totals, speed, ETA, banners (low space on the
receiver), and the file list with a filter (*All / Failed / Waiting / Done*). Each file
shows its state from the table below; failed files have **Retry**, and the job has
**Retry failed**.

### File states

| State | Side | Cause | What happens |
|---|---|---|---|
| Waiting / Sending / Done | — | normal flow | — |
| Skipped (identical) | receiver | same content already there, or `KeepExisting` | not sent |
| Failed · *changed while sending* | sender | size or mtime changed during the unit ([below](#source-file-changes-during-sending)) | `FileSkipped{SourceChanged}`; rest of the unit continues; Retry sends the current version |
| Failed · *in use by another program* | sender | the source can't be opened for reading (Windows exclusive lock) | `FileSkipped{SourceLocked}`; retried once after the unit, then Failed |
| Failed · *in use by another program* | receiver | the target can't be replaced ([receiving.md](receiving.md#locked-targets-mainly-windows)) | verified `.ldpart` kept; Retry or Keep both |
| Failed · *missing* | sender | file deleted or moved after the job was created | `FileSkipped{SourceMissing}` |
| Failed · *corrupted* | receiver | segment hash failed 3 times | Retry starts the file again |

A job with some failed files ends as **Completed with errors**; the failed files stay
listed until retried or the job is cleared.

Cancel semantics: the in-flight unit is aborted, files that were already fully written
stay, partial files of this job are deleted. A cancel from either side cancels the job
on both. Cancelling a deferred job that has not started just removes it.

## Performance

Goal: saturate the link (e.g. ~11–12 MB/s of payload on 100 Mbit/s, close to line rate
on gigabit) unless the disk is the bottleneck.

**Transport**

- One QUIC connection per peer, reused for all jobs and both directions.
- Large flow-control windows: connection and stream receive windows ≥ 16 MiB, send
  window to match; this covers the bandwidth-delay product of gigabit Wi-Fi/Ethernet
  with margin. Congestion control: BBR or Cubic (benchmark; keep the faster on Wi-Fi).
- UDP socket buffers raised (`SO_RCVBUF` / `SO_SNDBUF` ≈ 4–8 MiB); GSO/GRO used where
  the OS supports it (`quinn-udp` does this on Linux and Windows).
- TLS uses AES-GCM where the CPU has AES-NI/ARMv8 crypto, ChaCha20 otherwise.
- QUIC packet size: path MTU discovery enabled (start 1200, typically 1452 on LAN).
- Fallback if QUIC is throttled on a network (rare on LAN): TCP + TLS 1.3 with the same
  application protocol (later milestone, only if benchmarks show the need).

**Pipeline (sender)**

```text
disk read (blocking pool, 1 MiB chunks, read-ahead of next unit)
   → BLAKE3 update (same pass, no separate hashing read)
   → QUIC stream write
```

- Reading runs on a dedicated blocking thread with two or more buffers in flight, so
  disk reads, hashing, encryption and sending overlap.
- The next unit's first files are pre-opened/read ahead while the current unit finishes,
  so there is no gap between units even with "one at a time".
- No pre-scan hashing of the whole job: hashes are computed while sending and sent in
  segment and per-file trailers (see [Large files](#large-files-multi-gb)).

**Pipeline (receiver)**

- Writes in 1 MiB chunks via a blocking pool; files are pre-allocated to their size
  (where supported) to reduce fragmentation; BLAKE3 is updated while writing.
- No `fsync` per small file; one sync at the end of each unit before acknowledging it.
- The manifest is checked once (permissions, free space, conflicts); per-file work
  during transfer is just create → write → verify → rename.

**Measuring**

- `localdrop bench <device>` (planned): memory-to-memory throughput (network only) and
  disk-to-disk, to tell network and disk bottlenecks apart.
- Throughput targets are part of the M3 acceptance criteria in [roadmap.md](roadmap.md).

## Large files (multi-GB)

A file of any size (10 GB, 100 GB) is **streamed**; it is never loaded into memory.
Memory use is bounded by fixed buffers and does not depend on file size.

### Memory budget per direction (one unit in flight)

| Where | Desktop | Mobile |
|---|---|---|
| Sender read-ahead buffers (chunks of 1 MiB) | 8 × 1 MiB | 4 × 1 MiB |
| QUIC send buffer (bounded by peer's flow-control window) | ≤ 16 MiB | ≤ 8 MiB |
| Receiver QUIC receive window | 16 MiB | 8 MiB |
| Receiver write buffers | 8 × 1 MiB | 4 × 1 MiB |
| **Total, both sides** | **≈ 50 MiB** | **≈ 25 MiB** |

Backpressure is end-to-end: if the receiver's disk is slow, its write queue fills, it
stops reading the QUIC stream, the flow-control window closes, the sender's `write`
waits and its reader stops reading the disk. Nothing grows without bound.

A 16 MiB window is far above the LAN bandwidth-delay product (1 Gbit/s × 5 ms ≈ 0.6 MB),
so the window size does not limit speed.

### Segments

Large files are split logically into **segments of 64 MiB** (`SEGMENT_LEN`); a 10 GB
file has ~160. Segments are not separate streams or messages for the data; the bytes
flow continuously, and after every segment the sender inserts a small
`SegmentTrailer{segment, blake3}` frame ([protocol.md](protocol.md#unit-data-streams)).

- **Verification per segment.** The receiver hashes while writing and compares at each
  segment boundary. A corrupted segment is re-requested alone (64 MiB), not the whole
  10 GB file.
- **Durable checkpoints.** After a segment is verified, the receiver flushes it to disk
  (`fdatasync` / `FlushFileBuffers` for that range) and records "segments 0..n verified"
  in its job state. This also limits dirty page-cache build-up during long writes.
- **Resume without re-reading.** After a disconnect, crash or pause, the receiver
  answers `JobAccept` with the next unverified segment; the sender seeks there. At most
  one segment (64 MiB) is lost, and the 10 GB prefix is not re-hashed.
- **File hash.** The content hash of every file is
  `BLAKE3-derive-key("localdrop/v1/content", seg_hash_0 || seg_hash_1 || …)`, so both
  sides can compute it from segment hashes without one long-lived hasher (which cannot be
  persisted across a restart). Small files are simply one segment.

### Disk and file-system checks (before any data is sent)

- **Free space** for the whole job on the destination volume, plus a safety margin
  (max(1 GiB, 2 %)). The temporary file is created next to the target
  ([receiving.md](receiving.md)), so finishing is a rename, never a 10 GB copy.
- **File-size limits** of the destination file system: FAT32 (SD cards, USB sticks)
  cannot hold files ≥ 4 GiB → rejected up front with a clear error, not after 4 GB of
  transfer.
- **Pre-allocation** of the full size (`fallocate` / `SetEndOfFile` / `F_PREALLOCATE`)
  so a full disk is detected at start and the file is not fragmented.

### Running out of space

Other programs can use the space up while a long job runs. The receiver re-checks free
space before each unit and at each segment checkpoint. If
`free − bytes still to write` drops below the safety margin, it **pauses the job before
the disk is full**:

- receiver sends `JobPending{LowSpace{free, needed}}`; the sender's job shows *Paused ·
  receiver low on space* and a banner in the transfer details ("Laptop is running out of
  space: 1.2 GB free, 3.4 GB needed"); the receiver shows the same with a link to the
  destination folder;
- already written files and verified segments are kept;
- the receiver re-checks every 30 s and continues on its own (`JobAccept{resume}`) when
  enough space is free; either side can also Cancel, or the receiver can change the
  destination for the rest of the job.

On the receiver the job shows *Paused · low on space* and a banner with the numbers
("Paused: drive C: is almost full · 14.2 GB still needed, 0.9 GB free"):

- desktop, in the transfer details: **Open folder** and **Put the rest elsewhere…**;
  phone, on top of Transfers: **Free up space** (opens the system storage settings) and
  **Other folder**; a notification is shown too;
- *Put the rest elsewhere…* changes the destination for the files not yet finalized.
  Finished files stay where they are; the remaining files keep their relative paths
  under the new folder. The partial file of the paused file is deleted and that file
  starts again in the new folder (moving it could mean copying gigabytes between
  drives). The new folder gets the usual free-space and file-size checks first.

### Reading and writing efficiently

- Sequential-access hints: `posix_fadvise(SEQUENTIAL)` and `DONTNEED` on already-sent
  ranges (Linux/Android), `FILE_FLAG_SEQUENTIAL_SCAN` (Windows), `F_RDAHEAD` (macOS/iOS),
  so a 10 GB transfer does not evict the rest of the OS page cache.
- On the receiver, `DONTNEED` after each flushed segment for the same reason.

### Source file changes during sending

The sender records size and mtime when the unit starts and checks them at every segment
boundary and at the end. If the file changed, the sender stops sending it
(`FileSkipped{SourceChanged}`) instead of delivering a mix of old and new content; the
receiver deletes its temp file and the rest of the unit continues. The file is shown as
*Failed · changed while sending* with Retry. (For a deferred job checked before it
starts, see [deferred-send.md](deferred-send.md).)

### Keeping devices awake

A multi-GB transfer takes minutes (10 GB ≈ 15 min at 100 Mbit/s):

- Desktop: prevent idle sleep while a unit is in flight (`SetThreadExecutionState`,
  `IOPMAssertion`, `systemd-inhibit`/logind inhibitor).
- Android: foreground service + `WifiLock` (high-performance / low-latency mode) +
  partial `WakeLock` for the duration of the transfer.
- iOS: transfers run while the app is in the foreground; the app disables the idle timer
  during transfers and shows that locking the screen will pause them. Resume makes a
  pause cheap.

### Progress for large files

Progress is reported by bytes (`Progress` messages from the receiver, ~4/s), so a single
10 GB file shows smooth progress, speed and ETA. Only verified segments count as
"safely received" in the persistent job state.

## Resume and integrity

- Every file is verified by segment hashes and its content hash; a failed segment is
  re-sent (up to 3 times per segment, then the job fails with an error).
- After a disconnect, the job resumes at the first unacknowledged unit. Large units
  resume from the first unverified segment; bundles resume from the first file not yet
  completed inside them.
- Each file is written to a hidden `.<name>.ldpart` next to its target and renamed only
  after it is verified and flushed, so a half-written file never appears under its real
  name. Details: [receiving.md](receiving.md).
- Cancel deletes partial files of the job; pause keeps them.
