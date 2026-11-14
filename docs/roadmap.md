# Roadmap

## M0 — Foundation ✅

- Cargo workspace, crates and public types.
- Group key derivation, device identity, accept policy.
- Encrypted discovery announcement (seal/open) with tests.
- Control messages, framing, file-name sanitizing with tests.
- In-memory store (known devices with key pinning, outbox) and `Node::queue_send`.
- CLI skeleton, design documentation (incl. shares, jobs/units, conflicts, backups).

Known gaps against the current design, fixed in the milestones below:
`ControlMessage` still has the single-offer shape (`Offer` with per-file hashes) instead
of jobs/manifests/units; `queue_send` hashes files at enqueue time and handles files only
(no folders, no destination); store types say "transfer" instead of "job".

## M1 — Discovery

- mDNS backend (`mdns-sd`), broadcast backend, merge + `Lost` timeouts.
- Persistent config + identity (`localdrop init` saves state).
- `localdrop run` / `localdrop peers` show group members on the LAN.
- Interface enumeration and classification (ignore virtual/VPN), per-interface mDNS and
  directed broadcast, discovery on/off, restart on network change.
- Network fingerprints and remembered-network rules (Discover / Don't / Ask); SSID via
  platform hooks later in M6. See [networks.md](networks.md).

## M2 — Authenticated connections

- QUIC transport (`quinn` + `rustls`) with self-signed device certificates.
- SPAKE2 membership handshake bound to the TLS exporter; failure rate limiting.
- Known-device registration and key pinning on real connections.

## M3 — Transfer engine

- `localdrop-engine`: jobs, manifests, unit planning (≥ 100 MiB alone, small files in
  ~100 MiB bundles), per-direction pair queues with one unit in flight.
- Streaming send/receive pipelines with read-ahead, on-the-fly BLAKE3, trailer hashes,
  resume.
- Folders, inbox and deferred inbox, accept policy, queue progress, cancel/pause.
- `localdrop send` (files and folders), `localdrop queue`, `localdrop bench`.
- **Acceptance:** ≥ 90 % of link payload rate memory-to-memory on 100 Mbit/s and
  1 Gbit/s Ethernet; for 10 000 × 10 KiB files no worse than 2× the time of one file
  of the same total size.

## M4 — Shares and conflicts

- `localdrop-fs`: whitelist entries, audiences, permission inheritance, safe path
  resolution.
- Remote requests: list shares, list dir, download, upload into shares, delete
  (`Trash` / `Permanent`), make dir.
- Conflict detection from manifests; modes `KeepExisting`, `ReplaceWithBackup`,
  `ReplaceConflicting`, `Mirror`; `Ask` dialog on both sides, `Auto(mode)` per share.
- Backups and app trash: browser, restore, clear, retention.

## M5 — Send ahead

- `redb` persistent store for jobs and settings.
- Deferred delivery loop, `SourceChanged` handling, mobile cache copies, expiry.
- Deferred uploads into cached peer shares.

## M6 — Apps

- `flutter_rust_bridge` bindings, Flutter UI (device list, queue, share browser,
  settings, backups).
- Desktop: drag & drop, tray. Android: share target, SAF, foreground service.
  iOS: share extension, Bonjour shim, security-scoped bookmarks.

## Later / ideas

- Relaying queued jobs through an always-on group device.
- Multiple groups per device.
- TCP+TLS fallback transport if QUIC underperforms on some networks.
- Clipboard / text sharing.
