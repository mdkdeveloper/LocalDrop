# Development plan

How to get from the current foundation to shipping apps, based on the design documents in
`docs/` and the wireframes / styled screens in `design/`. [roadmap.md](roadmap.md) lists
*what* each milestone contains; this document says *in which order* to build it, what can
run in parallel, which screens belong to which step, and when a step is done.

## Where we are (November 2026)

| Area | State |
|---|---|
| Documentation | Complete for v1: architecture, security, discovery, networks, transfers, receiving, shares, protocol, deferred send, platforms. |
| Design | Wireframes for every screen, phone and desktop, as a clickable prototype (`design/tools/wf.py`, ~90 screens). Styled screens in Light and Dark, colour tokens in `design/tokens.json`, icon set in `styled.py`. |
| Rust code | ~2 000 lines. Done: group key derivation, device identity, accept policy types, sealed announcements, framing, file-name sanitizing, in-memory store. Stubs returning `NotImplemented`: mDNS, broadcast, QUIC, handshake, `Node::start`, direct send, `localdrop run`. |
| App | Not started (`app/` holds only a README). |
| CI / packaging | None. |

Known mismatches between code and docs (from the roadmap): `ControlMessage` has the old
single-offer shape, `queue_send` hashes at enqueue time and handles files only, store types
say "transfer" instead of "job".

## Principles

1. **Vertical slices, CLI first.** Every core milestone ends with something runnable from
   `localdrop` on two real machines. The CLI is the test harness for the core long before
   the app exists.
2. **Freeze the node API early.** The app only talks to `localdrop-node`
   ([architecture.md](architecture.md#ffi-boundary)). Its commands and `NodeEvent`s are
   drafted in phase 0, so the Flutter work can start against a fake node in parallel.
3. **Wire changes go with docs.** As in [CONTRIBUTING.md](../CONTRIBUTING.md): any change
   to bytes on the wire updates the matching document in the same change.
4. **Persistence before resume.** Crash-safe resume and the job journal
   ([receiving.md](receiving.md#resume-cancel-crash-recovery)) need a persistent store, so
   `redb` is part of M3 (see [changes to the roadmap](#changes-to-the-roadmap)).
5. **Every screen in the app maps to a screen in the design.** New states found while
   building are drawn in `wf.py` first (and linked into the prototype), then built.

## Tracks

Work runs in three tracks that meet at fixed points:

```text
          P0        M1          M2           M3              M4             M5        M6/Release
Core   ─[API,proto]─[discovery]─[QUIC+SPAKE]─[engine+redb]───[shares/fs]────[send ahead]─────────
App           └─[shell, theme, fake node]──[join/devices UI]─[queue UI]─────[browser UI]─[FRB live]─[polish]
Platform                         └─[Android/iOS spikes]────────[SAF/bookmarks]─[share targets, services]─[packaging]
```

- **Core** (Rust): the critical path. One person can own it end to end.
- **App** (Flutter): starts after phase 0 against a fake node that replays scripted
  `NodeEvent`s; switches to the real node through `flutter_rust_bridge` once M2 is in.
- **Platform**: early spikes for the risky parts (iOS Bonjour shim, Android SAF file
  descriptors, multicast lock), then the integrations in M6.

## Phase 0 — Groundwork (≈ 1–2 weeks)

Goal: remove the known mismatches and fix the contracts both other tracks depend on.

- **CI**: GitHub Actions on Windows, Linux, macOS running `cargo fmt --check`, `clippy -D
  warnings`, `cargo test`. Add `cargo deny` for licenses/advisories.
- **Protocol v1 shape**: replace `Offer` with the job messages from
  [protocol.md](protocol.md#control-messages) (`JobOffer`, `ManifestChunk`, `JobPending`,
  `ConflictReport`, `ConflictDecision`, `JobAccept`, `UnitDone`, …), `UnitHeader` /
  `EntryHeader` / `EntryTrailer` / `SegmentTrailer`, and the request messages. Round-trip
  tests and size-limit tests for each.
- **Store rename**: "transfer" → "job"; `Outbox` becomes the job store with manifests and
  states from [deferred-send.md](deferred-send.md#state-machine).
- **`queue_send`**: no hashing at enqueue, folders, `destination = Inbox | Share{..}`.
- **Node API draft** (`localdrop-node`): commands (join, leave, change password, discovery
  on/off, networks, send, queue ops, browse, settings, backups) and the full `NodeEvent`
  enum, all FFI-friendly (owned types, hex ids). Document it in `docs/node-api.md`.
- **Fake node**: a `localdrop-node` feature (or small crate) that emits scripted events
  for the app track and for UI tests.

Done when: CI is green on three OSes, `docs/protocol.md` no longer carries the "will be
reworked" note, `node-api.md` exists and the app track has a fake node to build against.

## M1 — Discovery and networks (≈ 3–4 weeks)

Core:

- Persistent config and identity (`localdrop init` saves state; key in a `0600` file on
  desktop for now, secure storage comes with the app).
- mDNS backend (`mdns-sd`) and UDP broadcast backend; merge by `DeviceId`, `Lost` timeouts,
  rotation every 15 min and on network change ([discovery.md](discovery.md)).
- Interface enumeration and classification (Wi-Fi / Ethernet / virtual / VPN / cellular),
  per-interface binding, directed broadcast, change watching with 2 s debounce.
- Network fingerprints, `KnownNetwork`, trust rules, discovery switch and
  discovery-at-launch ([networks.md](networks.md)).
- Manual address with *Remember this address*.
- CLI: `localdrop init`, `run`, `peers`, `networks`, `trust`.

App (fake node): Flutter project in `app/`, theme generated from `design/tokens.json`
(Light/Dark), icon set exported from `styled.py`, navigation shell for phone and desktop
layouts, first screens.

Screens: `join` / `d-join`, `join-error` / `d-join-error`, `perms`, `looking` /
`d-looking`, `connect` / `d-connect`, `disc-off` / `d-disc-off`, `d-net-menu`, `new-net` /
`d-new-net`, `networks` / `d-networks`, `devices`.

Done when: two laptops and an Android phone (via CLI in Termux or a debug build) in the same
group see each other within 5 s; a third device in another group sees nothing it can
decode; discovery stays silent on an untrusted network; WSL/Docker adapters are ignored.

## M2 — Authenticated connections (≈ 2–3 weeks)

Core:

- QUIC (`quinn` + `rustls`), self-signed Ed25519 device certificates, ALPN `localdrop/1`,
  one endpoint per active interface, flow-control windows from
  [transfers.md](transfers.md#performance).
- SPAKE2 membership handshake bound to the TLS exporter, key confirmation, rate limiting
  per source IP ([security.md](security.md#connection-authentication)).
- Key pinning on real connections, `KeyMismatch` surfaced as an event; `Hello`, `Ping`.
- Change password (with current-password check) and leave group.
- CLI: `localdrop peers` shows online/authenticated; `localdrop ping <device>`.
- **FFI bring-up**: `flutter_rust_bridge` crate in `app/rust`, builds for desktop and
  Android; the app can switch between fake and real node.

Screens: `device` / `d-device`, `known` / `d-devices`, `key` / `d-key`, `group`,
`change-pass` / `d-change-pass`, `leave` / `d-leave`.

Done when: handshake tests cover wrong password, relayed handshake and swapped key; the
desktop app (Windows) shows real peers from a Linux CLI node.

## M3 — Transfer engine (≈ 6–8 weeks, the largest step)

Split into three slices, each shippable on its own:

**M3a — Files over the wire.** `localdrop-engine` crate: job planning (manifest → units,
100 MiB threshold, bundles of ~100 MiB / 10 000 files), pair queues with one unit in
flight, sender and receiver pipelines with read-ahead and on-the-fly BLAKE3, segment
trailers, `.ldpart` temp files with batched finalize ([receiving.md](receiving.md)).
`localdrop send`, `localdrop bench`.

**M3b — Persistence and resume.** `redb` store for jobs, receiver job state and the job
journal; resume from the first unverified segment after disconnect or crash; segment
re-send; free-space and FAT32 checks, low-space pause and *Put the rest elsewhere*;
source-changed / locked / missing handling; keep-awake hooks on desktop.

**M3c — Queue features.** Folders and `job_subfolder`, inbox and deferred inbox, accept
policy with 10-minute timeout and *Always accept*, pause / resume / cancel / move to top /
retry failed, progress events (~4/s), history.

Screens: `d-drop`, `send` / `d-send`, `transfers` / `d-transfers`, `details` /
`d-details`, `job-menu` / `d-job-menu`, `incoming` / `d-incoming`, `recv` / `d-recv`,
`low-space` / `d-low-space`, `empty-transfers` / `d-empty-transfers`, `history`.

Done when the M3 acceptance criteria in [roadmap.md](roadmap.md#m3--transfer-engine) pass
(≥ 90 % of link rate on 100 Mbit/s and 1 Gbit/s; 10 000 × 10 KiB ≤ 2× one file of the same
size), plus: killing the receiver mid-way through a 10 GB file loses at most one segment;
no `.ldpart` remains after any finished or cancelled job.

## M4 — Shares, conflicts, backups (≈ 5–6 weeks)

Core (`localdrop-fs`):

- `FileAccess` trait with the desktop implementation; safe path resolution, symlink rules,
  hidden `.localdrop/`.
- Whitelist entries, audiences, permission inheritance with effective values.
- Remote requests on their own streams: `ListShares`, `ListDir`, `Stat`, `Search`,
  `Download`, `Delete`, `MakeDir`; uploads as jobs with `Share{..}` destination.
- Conflict detection from manifests with the hash cache; modes `KeepExisting`,
  `ReplaceWithBackup`, `ReplaceConflicting`, `Mirror`; `Ask` on both sides, first answer
  wins; journal for the two-step replace.
- Backups and app trash: browse, restore (current versions to Trash), clear, retention.

Screens: `folders` / `d-split`, `files-root`, `files-root-empty`, `files-remote` /
`d-remote`, `d-remote-folder`, `d-remote-empty`, `d-split-side`, `files-local` /
`local-folder`, `empty-folders` / `d-no-shares`, `add-share` / `d-add-share`, `share`,
`delete` / `d-delete`, `search` / `d-search`, `conflict` / `d-conflict`, `mirror` /
`d-mirror`, `backups` / `d-backups`, `restore` / `d-restore`, `clear-backups` /
`d-clear-backups`, `d-defaults`.

Done when: a path-traversal test suite (`..`, absolute, symlinks, junctions, reserved names,
case and NFC collisions) passes on all three desktop OSes; `Mirror` never removes a file
outside the target folder; a crash during `ReplaceWithBackup` never loses both versions.

## M5 — Send ahead (≈ 2–3 weeks)

Mostly wiring, since jobs and `redb` already exist after M3:

- Deferred delivery loop on authentication, `SourceChanged` dialog choices, optional
  expiry, *Change destination* while waiting.
- `OutboxStorage::CopyToCache` for mobile (size prompt before copying).
- Deferred uploads into cached peer shares, permission re-checked on delivery.

Screens: `files-offline` / `d-offline`, `changed` / `d-changed`.

Done when: a job queued for an offline laptop is delivered unattended after it comes back,
and survives a restart of either side in between.

## M6 — Apps and platforms (≈ 6–8 weeks, overlapping M4–M5)

- **Desktop**: drag & drop onto window and device, tray / menu bar (`d-tray`), system
  notifications, login item, firewall rules in the installer, secure key storage (DPAPI,
  Keychain, Secret Service).
- **Android**: share target, SAF `FileAccess` with detached file descriptors, MediaStore
  inbox with `IS_PENDING`, foreground service `dataSync`, multicast/Wi-Fi/wake locks,
  socket binding to the Wi-Fi network, permissions screen.
- **iOS**: Bonjour shim in Swift forwarding records to the node, share extension handing
  files to the app group, security-scoped bookmarks, `Documents/Inbox` in Files, idle-timer
  handling and the "open LocalDrop to receive" hints.
- **Settings**: `settings` / `d-settings`, `d-general`, `notifs`, per-device overrides.

## Release (≈ 2–3 weeks)

- Packaging: MSIX or signed installer (Windows), DMG + notarization (macOS), AppImage and
  Flatpak (Linux), Play Store and TestFlight builds.
- Update prompt on protocol major-version mismatch.
- Password strength rule in the join form (≥ 10 characters or a generated 4-word
  passphrase, [security.md](security.md#threat-model)).
- A short security review of handshake, path resolution and file finalizing before 1.0.

## Platform spikes (run early, in parallel with M1–M2)

These decide whether parts of the design hold; each is a few days and ends with a short
note in `docs/`:

| Spike | Question | Affects |
|---|---|---|
| iOS discovery | Does Bonjour via `NWBrowser` forward announcements reliably without the multicast entitlement? | M1, M6 |
| Android SAF I/O | Throughput of writing through a detached fd into a SAF tree vs. MediaStore. | M3, M6 |
| QUIC on Wi-Fi | BBR vs. Cubic, GSO on Windows, real throughput on 5 GHz Wi-Fi with `quinn`. | M3 acceptance |
| Windows network identity | Network List Manager profile id and SSID on 24H2 without Location. | M1 fingerprints |
| FRB streams | `Stream<NodeEvent>` at ~4 events/s per job across many jobs without UI jank. | M2 FFI |

## Changes to the roadmap

Made while writing this plan and applied to [roadmap.md](roadmap.md):

- **Phase 0** is added before M1 (protocol rework, store rename, node API, CI).
- **`redb` moves from M5 to M3b**, because resume after a crash and the job journal need it.
- **The app starts in phase 0** against a fake node instead of waiting for M6; M6 keeps
  only the platform integrations and live wiring.
- **History** (`history` screen) is part of M3c; it needs persisted finished jobs.

## Rough timeline

With one Rust developer on the core and one Flutter developer on the app, about
**7–9 months** to a 1.0 on all five platforms:

| Months | Core | App / platform |
|---|---|---|
| 1 | P0, M1 | shell, theme, join and devices screens (fake node); iOS and Windows spikes |
| 2 | M2, start M3a | FRB bring-up, device and group screens; SAF and QUIC spikes |
| 3–4 | M3a–c | send, queue, details, incoming dialogs |
| 5 | M4 | share browser, conflicts, backups |
| 6 | M5 | Android and desktop integrations |
| 7 | fixes, performance | iOS integrations, settings |
| 8–9 | release work | packaging, store submissions, beta |

A desktop-only beta (Windows, Linux, macOS) is possible after M3 + desktop parts of M6,
around month 4–5, and is a good point to get real-world feedback before shares.
