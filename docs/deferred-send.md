# Deferred send ("send ahead")

Queue files or folders for one of your devices while it is not on the network. They are
delivered automatically when it appears. Deferred jobs are ordinary
[jobs](transfers.md#jobs-and-units) in the pair queue to that device; they simply wait
until the device is online.

## Rules

- **Target must be a known device**: one that authenticated with us at least once, so we
  have its pinned public key. `queue_send` returns `UnknownDevice` otherwise.
- **Destination**:
  - the receiver's `deferred_inbox_dir` (default; falls back to `inbox_dir`), see
    [shares.md](shares.md#receive-folders), or
  - one of the receiver's shares with `upload` permission, chosen from the share list
    cached at the last connection. Permission is re-checked on delivery; if it was
    revoked the job fails with `Denied`.
- **The queue lives on the sender.** Delivery happens when *both* devices are online on
  the same LAN and LocalDrop is running on both (see background limits below).
- Each job keeps one `job_id` for its whole life, so retries and resumes are idempotent
  on the receiver.
- The receiver's accept policy and conflict policy apply as usual. With auto-accept and
  `Auto(mode)` conflicts, delivery is fully unattended; otherwise both users get the
  dialog when the job is delivered and the job waits in `AwaitingDecision` without
  blocking the rest of the queue.

## Storage of queued files

| Platform | Mode | Behaviour |
|---|---|---|
| Windows, Linux, macOS | `OutboxStorage::Reference` | Store paths plus size and mtime of every file in the manifest. Nothing is read at enqueue time (fast even for huge folders). At send time a changed or missing file marks the job `SourceChanged` and the user chooses: send current version, skip changed files, or cancel. Files added to a queued folder later are **not** picked up (the manifest is fixed at enqueue). |
| Android, iOS | `OutboxStorage::CopyToCache` | Copy into the app cache, because content URIs / security-scoped URLs from Share and pickers can expire. The copy is deleted when the job ends. Large folders: the UI shows the size and asks before copying. |

Integrity is not checked at enqueue time: BLAKE3 hashes are computed during sending
([transfers.md](transfers.md#performance)).

Outbox metadata (jobs, manifests, state, progress) is stored in the node database
(`redb`, planned; `MemoryStore` today).

## State machine

```text
              queue_send
                  │
                  ▼
             ┌─────────┐   peer online + JobOffer
  ┌─────────▶│ Waiting │──────────────────────────┐
  │          └─────────┘                          ▼
  │                                   ┌──────────────────────┐
  │                                   │  AwaitingDecision    │  (accept: Ask,
  │                                   │  (dialog on A and B) │   conflicts: Ask)
  │                                   └──────────┬───────────┘
  │  disconnect / error                          │ JobAccept
  │                                              ▼
  │                                        ┌────────────┐
  └────────────────────────────────────────│ InProgress │ ◀─ Pause/Resume ─▶ Paused
                                           └─────┬──────┘
                                                 │ JobDone
                                                 ▼
                                           ┌───────────┐
                                           │ Completed │
                                           └───────────┘

  any active state ── JobReject ──▶ Rejected
  AwaitingDecision ── JobReject{NoAnswer} (10 min) ──▶ Waiting (offered again on next connect)
  InProgress ── JobPending{LowSpace} ──▶ AwaitingDecision ── space freed ──▶ InProgress
  any active state ── Cancel (either side) ──▶ Cancelled
  Waiting ── source changed/missing ──▶ SourceChanged ── user choice ──▶ Waiting | Cancelled
  any active state ── expires_unix passed ──▶ Expired (optional expiry)
```

Active states are retried whenever the target authenticates. The queue to a device is
FIFO (user may reorder); one unit at a time per direction.

## Queue UI

Deferred jobs appear in the same per-device queue as live ones, with state
"Waiting for <device>", total size and file count, and actions *Cancel*, *Move up*,
*Change destination* (while still `Waiting`). See
[transfers.md](transfers.md#queue-ui).

## Background limits

Delivery needs the app running on both sides:

- **Desktop:** run in the tray / as a login item.
- **Android:** a foreground service (with a persistent notification) while there are
  pending jobs or while the user enabled "always receive". Requires a multicast lock.
- **iOS:** no long-running background networking. Deliveries happen while the app is in
  the foreground, plus short `BGTaskScheduler` windows that are not guaranteed. The UI
  must make this clear ("open LocalDrop on your iPhone to receive").

## Not in scope (yet)

- Relaying through another group device when the sender is offline.
- Queuing for a device that was never seen, "any device", or the whole group.
