# Roadmap

## M0 — Foundation ✅

- Cargo workspace, crates and public types.
- Group key derivation, device identity, accept policy.
- Encrypted discovery announcement (seal/open) with tests.
- Control messages, framing, file-name sanitizing with tests.
- In-memory store (known devices with key pinning, outbox) and `Node::queue_send`.
- CLI skeleton, design documentation (incl. shares, jobs/units, conflicts, backups).

Known gaps against the current design, fixed in P0 below (done):
`ControlMessage` still has the single-offer shape (`Offer` with per-file hashes) instead
of jobs/manifests/units; `queue_send` hashes files at enqueue time and handles files only
(no folders, no destination); store types say "transfer" instead of "job".

Build order, parallel tracks, screens per milestone and exit criteria:
[development-plan.md](development-plan.md).

## P0 — Groundwork ✅

- CI on Windows, Linux and macOS (`fmt`, `clippy -D warnings`, tests, `cargo deny`).
- Protocol v1 shape: job, manifest, unit and request messages from
  [protocol.md](protocol.md), replacing the single `Offer`.
- Store: "transfer" → "job"; job store with manifests and states.
- `queue_send` without hashing at enqueue, with folders and a destination.
- Node API draft (commands + `NodeEvent`), documented for the FFI.
- Fake node emitting scripted events, so the app can be built in parallel.
- App shell: Flutter project in `app/`, theme generated from `design/tokens.json`,
  phone and desktop navigation, a Dart fake node, first screens (see
  [app/README.md](../app/README.md)).

## M1 — Discovery ✅

- ✅ mDNS backend (`mdns-sd`), broadcast backend, merge + `Lost` timeouts.
- ✅ Persistent config + identity (`localdrop init` saves state).
- ✅ `localdrop run` / `localdrop peers` show group members on the LAN; also
  `localdrop networks` and `localdrop trust`.
- ✅ Interface enumeration and classification (ignore virtual/VPN), per-interface mDNS and
  directed broadcast, discovery on/off, restart on network change (polling every 2 s;
  OS change notifications later).
- ✅ Network fingerprints and trusted networks (none trusted by default; trust on join,
  ask on new networks, optional "trust every network"), discovery-at-launch setting;
  SSID via platform hooks later in M6. See [networks.md](networks.md).
- ✅ App screens against the fake node: Settings › Networks (`networks`, `d-networks`),
  network menu from the discovery pill (`d-net-menu`), *Trust this network?* as sheet,
  main-area card or banner (`new-net`, `d-new-net`), discovery off with paused transfers
  (`disc-off`, `d-disc-off`), looking for devices (`looking`, `d-looking`) and the
  *Connect by address* form (`connect`, `d-connect`; the node side needs the M2
  handshake). `NetworkView` carries kind, address and last seen for these screens.
- Later: listing ignored virtual adapters with an *Include* switch in `d-networks`, and
  *Show Wi-Fi names* (needs the M6 platform hooks).
- Moved to M2: *Connect by address* (a manual address has no announcement, so only the
  handshake can tell who is there).

## M2 — Authenticated connections (core connections ✅, rest open)

- ✅ QUIC transport (`quinn` + `rustls`, ring, TLS 1.3 only, ALPN `localdrop/1`) with
  self-signed Ed25519 device certificates; flow-control windows from
  [transfers.md](transfers.md#performance), keep-alive 5 s, idle timeout 20 s.
- ✅ SPAKE2 membership handshake bound to the TLS exporter and both public keys, with key
  confirmation; failed handshakes rate-limited per source IP (5 per minute).
- ✅ `Hello` (device id must match the certificate key, same protocol version) and
  `Ping`/`Pong` on the control stream.
- ✅ Node: endpoint on the announced port, accepts only from active trusted subnets; dials
  discovered members (the smaller device id dials, the other after 3 s), keeps one
  connection per device, pins keys in the store, `PeerOnline` / `KeyMismatch` events,
  closes everything when discovery is off. *Connect by address* (remembered addresses are
  retried at start).
- ✅ CLI: `localdrop peers` shows online / found, `localdrop ping <device>`,
  `localdrop connect <address>`.
- Verified on one machine with two CLI nodes over Wi-Fi discovery; a wrong password is
  refused on both sides.
- ✅ Change password (current password checked by deriving it again with the stored
  Argon2id parameters; at least 10 characters; connections and announcements restart
  with the new key) and leave group (key deleted; optional: cancel the queue, forget
  devices). CLI `localdrop password`, `localdrop leave`.
- ✅ `flutter_rust_bridge` 2.13 bridge (`app/rust`, in the Cargo workspace): flat DTOs
  over `localdrop-node`, its own tokio runtime, events as a Dart stream; `RustNode`
  implements `NodeClient`, the app opens an already joined state at launch and uses the
  real node by default (`--dart-define=NODE=fake` for the demo).
- ✅ Windows desktop app on the real node, checked against a CLI node on the same
  machine: both find each other over Wi-Fi discovery and come online; the discovery
  switch in the app closes the connection and turning it back on reconnects by itself.
  (Building with plugins on Windows needs Developer Mode.)
- Open: Android build and an app-data state folder on mobile (`path_provider`);
  screens `group`, `change-pass`, `leave`, `device`, `known`, `key`; a fixed-port setting
  for *Connect by address* without a port.

## M3 — Transfer engine

- `localdrop-engine`: jobs, manifests, unit planning (≥ 100 MiB alone, small files in
  ~100 MiB bundles), per-direction pair queues with one unit in flight.
- Streaming send/receive pipelines with read-ahead, on-the-fly BLAKE3, trailer hashes,
  resume.
- `redb` persistent store for jobs, receiver job state and the job journal (needed for
  resume after a crash, see [receiving.md](receiving.md)).
- Folders, inbox and deferred inbox, accept policy, queue progress, cancel/pause,
  history of finished jobs.
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

- Deferred delivery loop, `SourceChanged` handling, mobile cache copies, expiry.
- Deferred uploads into cached peer shares.

## M6 — Apps

The Flutter UI is built alongside M1–M5 (against the fake node, then the real one via
`flutter_rust_bridge` from M2); M6 covers the platform integrations and final wiring.

- Settings screens, per-device overrides, notifications.
- Desktop: drag & drop, tray. Android: share target, SAF, foreground service.
  iOS: share extension, Bonjour shim, security-scoped bookmarks.

## Later / ideas

- Relaying queued jobs through an always-on group device.
- Multiple groups per device.
- TCP+TLS fallback transport if QUIC underperforms on some networks.
- Clipboard / text sharing.
