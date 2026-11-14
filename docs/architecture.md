# Architecture

## Goals

1. Transfer files **and folders** between a user's own devices on the same LAN, on
   Android, iOS, Windows, Linux and macOS, as fast as the link (or the disk) allows.
2. Devices outside the user's group must not be able to discover, identify, or connect
   to group devices.
3. Allow queuing files for an offline device ("send ahead") and deliver them
   automatically once it appears.
4. Let devices browse, download from, upload to and delete in each other's
   **whitelisted folders**, with per-folder permissions.
5. One Rust core shared by every platform; thin UI per front-end.

Non-goals (for now): Internet/relay transfers, multiple groups per device, continuous
folder sync, delivery through a third device when the sender is offline.

## Concepts

| Term | Meaning |
|---|---|
| **Device** | One install of LocalDrop. Owns a persistent Ed25519 keypair. |
| **DeviceId** | 16-byte id derived from the device's public key. |
| **Group** | Name + shared password. A device belongs to exactly one group. |
| **GroupKey** | Keys derived from name + password (Argon2id → HKDF). The password is not stored. |
| **Member** | A device that proved knowledge of the group password in the handshake. |
| **Known device** | A member we have authenticated at least once; its public key is pinned. |
| **Inbox** | Default receive folder; a separate one may be set for deferred jobs. |
| **Share** | A whitelisted folder with an audience and permissions (browse / download / upload / delete). See [shares.md](shares.md). |
| **Job** | One user action (drop, Share, download request): files and folders with a manifest. What the queue UI shows. |
| **Unit** | What is sent at once: one large file (≥ 100 MiB) or a bundle of small files (~100 MiB). See [transfers.md](transfers.md). |
| **Pair queue** | FIFO of jobs for one direction (this device → peer); one unit in flight. |
| **Deferred job** | A job in a pair queue whose peer is offline. See [deferred-send.md](deferred-send.md). |
| **Backup set** | Old versions saved by `ReplaceWithBackup`, browsable and clearable locally. |

## Layers and crates

```text
            ┌──────────────┐   ┌───────────────────────────┐
            │ localdrop-cli│   │ Flutter app (app/, later) │
            └──────┬───────┘   └─────────────┬─────────────┘
                   │      flutter_rust_bridge│
                   ▼                         ▼
     ┌──────────────────────────────────────────────────────────┐
     │                      localdrop-node                      │  public API, events,
     │  start/stop · send · queue · browse · settings · backups │  wiring
     └──┬──────────┬───────────┬───────────┬───────────┬────────┘
        ▼          ▼           ▼           ▼           ▼
 ┌───────────┐┌──────────┐┌──────────┐┌──────────┐┌──────────┐
 │ discovery ││ transport││  engine  ││    fs    ││  store   │
 │ mDNS, UDP ││ QUIC+TLS,││ jobs,    ││ shares,  ││ devices, │
 │ broadcast ││ SPAKE2   ││ units,   ││ perms,   ││ jobs,    │
 │           ││          ││ pair     ││ conflicts││ settings │
 │           ││          ││ queues,  ││ backups, ││          │
 │           ││          ││ pipeline ││ trash    ││          │
 └─────┬─────┘└────┬─────┘└────┬─────┘└────┬─────┘└────┬─────┘
       │           │      ┌────┴─────┐     │           │
       │           │      │ protocol │     │           │
       │           │      └────┬─────┘     │           │
       └───────────┴───────────┴─────┬─────┴───────────┘
                                     ▼
                            ┌─────────────────┐
                            │ localdrop-core  │
                            └─────────────────┘
```

- **core** — pure types and crypto derivations; no I/O, no async runtime.
- **discovery** — produces `DiscoveryEvent::{Found, Lost}` for *claimed* group members.
  Discovery never grants trust.
- **transport** — opens/accepts QUIC connections and runs the membership handshake.
  Output is an authenticated peer (public key + address).
- **protocol** — serializable messages and framing; no I/O.
- **engine** *(planned crate `localdrop-engine`)* — job planning (manifest → units),
  pair queues, send/receive pipelines, resume, progress.
- **fs** *(planned crate `localdrop-fs`)* — `FileAccess` abstraction (paths on desktop,
  URIs/fds on mobile), shares and permission resolution, safe path resolution, conflict
  detection and modes, temp-file finalizing and job journal
  ([receiving.md](receiving.md)), backups and app trash.
- **store** — persistence behind traits (`KnownDevices`, `Outbox` → jobs, settings);
  `MemoryStore` now, `redb` planned.
- **node** — the only crate front-ends talk to.

## Main flows

### Joining a group

1. User enters group name and password.
2. `GroupKey::derive` (Argon2id, ~0.3–1 s). The UI shows the **fingerprint**
   (`3fa2-91c0`) so the user can check that all devices show the same value.
3. Node stores the group name, `GroupKey` and device identity; the password is discarded.

### Seeing a device and sending a folder

```text
A (sender)                                    B (receiver / owner)
  │ announce: sealed(DeviceId_A)  ───────────▶ │ opens with k_disc → "A, port p"
  │ ◀─────────── announce: sealed(DeviceId_B)  │
  │ QUIC connect (mutual self-signed certs) ─▶ │
  │ ◀──────── SPAKE2 membership handshake ───▶ │
  │ pin/check public key of B                  │ pin/check public key of A
  │ Hello ◀──────────────────────────────────▶ │
  │ JobOffer + Manifest ─────────────────────▶ │ accept policy, permissions,
  │                                            │ free space, conflicts
  │ ◀──── ConflictReport (dialog on A and B, first answer wins)
  │ ◀──────────────────────── JobAccept{skip}  │
  │ unit 1: large file ──────────────────────▶ │ write, verify BLAKE3
  │ ◀───────────────────────────── UnitDone    │
  │ unit 2: bundle of small files ───────────▶ │ split stream, write each file
  │ ◀───────────────────────────── UnitDone    │
  │ ◀───────────────────────────── JobDone     │
```

### Browsing a share

`ListShares` → `ListDir` on a request stream; a download becomes a job in the owner's
pair queue to the requester; delete requests are applied by the owner according to its
delete mode. See [shares.md](shares.md).

### Send ahead

See [deferred-send.md](deferred-send.md). In short: `queue_send` stores a job in the
pair queue for an offline known device; whenever that device authenticates, the node
offers every active job for it in queue order.

## Concurrency model

- `tokio` multi-threaded runtime inside the node; file I/O on the blocking pool with
  read-ahead and double buffering ([transfers.md](transfers.md#performance)).
- One actor task per peer connection drives that peer's two pair queues (outgoing:
  send units one at a time; incoming: receive and write). Different peers run in parallel.
- A node-level task reacts to discovery events, owns the store and dispatches jobs to
  peer actors, so store access is serialized.
- Front-ends receive `NodeEvent`s from a channel (FFI-friendly: no callbacks into Dart).

## FFI boundary

The Flutter app will use `flutter_rust_bridge` against `localdrop-node` (possibly via a
thin `localdrop-ffi` crate). Rules for the public API:

- owned types only (`String`, `Vec`, plain structs/enums); ids exposed as hex strings
  at the FFI layer;
- async functions and a `Stream<NodeEvent>`;
- no platform code in Rust for things the OS must own (file pickers, notifications,
  foreground services, secure storage) — the app does those and passes results in.
