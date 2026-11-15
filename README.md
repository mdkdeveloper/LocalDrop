# LocalDrop

Send files between your own devices on the local network — Android, iOS, Windows,
Linux and macOS — with no cloud, no account, and no strangers.

- **Private discovery.** Devices join a *group* with a shared name and password.
  Only group members can see each other on the LAN; to everyone else the announcements
  are random bytes.
- **Authenticated, encrypted transfers** over QUIC/TLS 1.3, with a password-authenticated
  handshake (SPAKE2) on top.
- **Files and whole folders** — drag & drop on desktop, Share on phones. Small files are
  streamed in ~100 MB bundles, large files on their own; built to saturate the link.
- **Send ahead.** Queue files for one of your devices while it is offline; they are
  delivered automatically the moment it shows up on the network.
- **Shared folders.** Whitelist folders other devices may browse, download from, upload
  to or delete in — per device or for the whole group, with per-folder settings
  (delete to trash or permanently, conflict handling, backups of replaced files).
- **Queue with progress** per device; cancel any job.

> **Status:** early foundation. The workspace, core types, key derivation, discovery
> announcement format and wire messages are in place; networking is not implemented yet.
> See [docs/roadmap.md](docs/roadmap.md).

## Documentation

| Document | Contents |
|---|---|
| [Architecture](docs/architecture.md) | Layers, crates, data flow, FFI boundary |
| [Security](docs/security.md) | Threat model, key derivation, handshake, limitations |
| [Discovery](docs/discovery.md) | mDNS and broadcast announcements |
| [Networks](docs/networks.md) | Discovery switch, interfaces, trusted networks |
| [Transfers](docs/transfers.md) | Jobs, bundles, pair queues, performance |
| [Shares](docs/shares.md) | Whitelist, permissions, conflicts, backups |
| [Receiving](docs/receiving.md) | Temp files, verify-then-rename, crash recovery |
| [Protocol](docs/protocol.md) | Control messages, unit streams, remote requests |
| [Deferred send](docs/deferred-send.md) | Outbox model and delivery rules |
| [Platforms](docs/platforms.md) | Per-OS permissions, background limits, Flutter |
| [Roadmap](docs/roadmap.md) | Milestones |
| [Development plan](docs/development-plan.md) | Build order, tracks, screens per milestone, exit criteria |

## Repository layout

```text
crates/
  localdrop-core        device identity, group keys, accept policy, errors
  localdrop-discovery   encrypted announcements, mDNS + UDP broadcast backends
  localdrop-protocol    control messages, framing, file-name sanitizing
  localdrop-transport   QUIC transport + membership handshake
  localdrop-store       known devices (pinned keys) and the outbox
  localdrop-node        high-level API used by all front-ends
  localdrop-cli         `localdrop` command-line tool
  (planned) localdrop-engine, localdrop-fs   transfer engine; shares, permissions, backups
app/                    Flutter app (planned)
docs/                   design documentation
```

## Building

Requires a stable Rust toolchain (see `rust-toolchain.toml`).

```bash
cargo build --workspace
cargo test --workspace
cargo run -p localdrop-cli -- --help
```

## License

MIT — see [LICENSE](LICENSE).
