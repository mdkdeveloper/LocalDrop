# Node API

The API that front-ends (the CLI and the Flutter app) use. It lives in `localdrop-node`;
the Flutter app reaches it through a thin `flutter_rust_bridge` crate
([architecture.md](architecture.md#ffi-boundary)). This document is the contract between
the core and the app tracks of the [development plan](development-plan.md): the app is
built against it from phase 0, first with the [fake node](#fake-node).

Status markers: **✔** works now, **M1…M5** returns `Error::NotImplemented` until that
milestone.

## Rules

- Owned types only (`String`, `Vec`, plain structs and enums). Ids are typed in Rust
  (`DeviceId`, `JobId`, `ShareId`) and become 32-character hex strings in the bridge.
- One event channel per node (`mpsc::UnboundedReceiver<NodeEvent>`, a `Stream` in Dart).
  No callbacks into Dart.
- Commands return `Result<_, Error>`; the bridge maps `Error` to a Dart exception with
  its message. `InvalidState` means the action doesn't fit the item's current state (the
  UI should not have offered it).
- Commands change state and also emit the matching event, so the UI can update from
  events alone and use the getters only for the first load.
- No platform code in Rust for what the OS owns: pickers, notifications, foreground
  services, secure storage. The app does those and passes results in.

## Creating a node

```rust
let (node, events) = Node::new(config, identity, group, group_key, store);
```

| Argument | What |
|---|---|
| `config: NodeConfig` | device name, `inbox_dir`, `deferred_inbox_dir`, `job_subfolder`, listen port, accept settings, outbox storage |
| `identity: DeviceIdentity` | the device key from secure storage (generated on first launch) |
| `group: GroupConfig`, `group_key: GroupKey` | from the join form; the password itself is not kept |
| `store: Arc<dyn Store>` | `MemoryStore` now, `redb` from M3 |

Joining is building the key (`GroupKey::derive`, 0.3–1 s, run off the UI thread) and
creating the node; screens `join` / `d-join`.

## Commands

### Lifecycle and group

| Command | Status | Screens |
|---|---|---|
| `start()` / `stop()` | M1 / ✔ | — |
| `set_discovery(on)` | M1 | `disc-off`, `d-disc-off`, discovery switch |
| `group_info() -> GroupInfo{name, fingerprint}` | ✔ | `group`, settings |
| `set_device_name(name)`, `set_visible(on)` | ✔ | `group`, `d-general` |
| `fixed_port()` / `set_fixed_port(port)` — 0 = random; a running node restarts its endpoint and discovery | ✔ | `d-general` (*Listening port*) |
| `change_password(current, new) -> GroupInfo` | ✔ | `change-pass`, `d-change-pass` |
| `leave_group(keep_queue, forget_devices)` | ✔ | `leave`, `d-leave` |

### Devices and networks

| Command | Status | Screens |
|---|---|---|
| `devices() -> Vec<DeviceView>` | ✔ (presence always `Offline` until M1) | `devices`, `d-devices`, `known` |
| `set_device_accept(device, Option<AcceptPolicy>)` | ✔ | `device`, `d-device` |
| `forget_device(device)` — cancels its queued jobs | ✔ | `device`, `d-device` |
| `connect_address(address, remember) -> DeviceId` | ✔ | `connect`, `d-connect` |
| `ping(device) -> Duration` | ✔ | (CLI) |
| `listen_port() -> Option<u16>` | ✔ | `d-general` |
| `networks() -> Vec<NetworkView>` | ✔ | `networks`, `d-networks`, `d-net-menu` |
| `set_network_trust(key, NetworkTrust)` | ✔ | `new-net`, `d-new-net`, `networks` |
| `rename_network(key, label)` / `forget_network(key)` | ✔ | `networks`, `d-networks` |
| `network_settings() -> NetworkSettings` / `set_network_settings(..)` | ✔ | `networks`, `d-networks`, `d-net-menu` |

`DeviceView{id, name, presence: Online|Found|Offline, last_seen_unix, accept_override,
queued_jobs}`.

`NetworkView{key, label, kind: WiFi|Ethernet, address, last_seen_unix, connected, active,
trust: Trusted|NotTrusted|NeverAsk, weak_identity}`: connected networks first, then
remembered ones. `key` is the full hex fingerprint; events and the CLI may use its first
8 characters, which every command accepts. `address` is this device's address while
connected (`192.168.1.23/24`). `active` means trusted and selected in the network menu;
whether discovery actually runs also depends on the switch (`DiscoveryChanged`).

`NetworkSettings{trust_every_network, selection: TrustedNetworks|Only(key),
discover_at_launch, on_new_network: AskToTrust|StaySilent, include_interfaces}`. The bridge
exposes `set_network_settings` with a whole `NetworkSettings` value instead of the Rust
closure.

`connect_address` takes `ip[:port]` or a host name; without a port it tries 47474, the
port to set as fixed on devices that should be reachable that way (discovery uses a random
port by default). The address must be on a trusted network of the other device. With
`remember` it is saved and tried again at every start.

Presence: `Found` = announcement seen, `Online` = authenticated connection. A lost
announcement doesn't take an online device offline (QUIC keep-alives decide); a closed
connection goes back to `Found` while it is still announced, otherwise `Offline`.

The app shows the *Trust this network?* ask prominently (sheet on phones, main area on
desktop) when no trusted network is connected, and as a banner when another trusted network
keeps discovery running.

### Jobs

| Command | Status | Screens |
|---|---|---|
| `send(SendRequest{target, sources, destination}) -> JobId` | ✔ queues; live sending M3 | `send`, `d-send`, `d-drop` |
| `queue_send(SendRequest) -> JobId` | ✔ | `files-offline`, `d-offline` |
| `jobs() -> Vec<JobView>` — active in queue order, then history | ✔ | `transfers`, `d-transfers`, `history` |
| `job_action(id, JobAction)` | ✔ Pause, Resume, MoveToTop, Cancel, Clear; M3 RetryFailed, OfferAgain | `job-menu`, `d-job-menu`, `details` |
| `answer_request(id, AcceptAnswer{accept, always})` | M3 | `incoming`, `d-incoming` |
| `answer_conflict(id, Option<ConflictMode>)` — `None` cancels | M4 | `conflict`, `d-conflict`, `mirror`, `d-mirror` |
| `answer_source_changed(id, SourceChangedChoice)` | M5 | `changed`, `d-changed` |

`send` scans files and folders for metadata only (no reading, no hashing); a folder keeps
its structure, symbolic links are skipped. With `OutboxStorage::CopyToCache` the files
are copied into `<cache>/<job id>/` and the copies are deleted when the job is cancelled.
The job name is the item's name, or *"first and N more"* for several items.

`JobView{id, peer, direction: Outgoing|Incoming, name, destination, deferred, files_total,
bytes_total, bytes_done, state, created_unix}`. `JobState` values and which actions fit
each state: [transfers.md](transfers.md#job-actions).

### Remote shares (M4)

| Command | Screens |
|---|---|
| `remote_shares(peer) -> Vec<ShareInfo>` | `files-root`, `d-remote` |
| `list_dir(peer, share, path, cursor) -> (Vec<DirEntry>, Option<Cursor>)` | `files-remote`, `d-remote-folder` |
| `search(peer, shares, query, cursor) -> (Vec<SearchHit>, Option<Cursor>)` | `search`, `d-search` |
| `download(peer, share, paths, to) -> JobId` | `files-remote`, `d-split` |
| `delete_remote(peer, share, paths) -> Vec<(path, Option<error>)>` | `delete`, `d-delete` |
| `make_dir(peer, share, path)` | `files-remote` |

Still to be added in M4 (shape decided then): local share settings (`add-share`,
`d-add-share`, `share`, `d-defaults`), backups and app trash (`backups`, `restore`,
`clear-backups` and their desktop versions). App settings beyond `NodeConfig` (`settings`,
`d-general`, `notifs`) follow in M6.

## Events

All variants of `NodeEvent` (`#[non_exhaustive]`: the app must handle unknown ones):

| Event | Meaning | UI |
|---|---|---|
| `DiscoveryChanged{on, active_networks}` | switch or active networks changed | header, `disc-off` |
| `UntrustedNetwork{key, label}` | connected to a network that isn't trusted | `new-net` sheet, `d-new-net` banner |
| `PeerFound{device}` | announcement seen | device list |
| `PeerOnline{device, name}` / `PeerOffline{device}` | authenticated / gone | device list |
| `KeyMismatch{device, name}` | known device with a different key, refused | `key`, `d-key` |
| `JobAdded{job}` | new row in a queue | transfers |
| `JobStateChanged{id, state}` | state change | row, details |
| `JobProgress{id, bytes_done, bytes_total, speed_bps}` | ~4/s for the job in flight | row, details, tray |
| `JobRemoved{id}` | row cleared | transfers |
| `IncomingRequest{id, from, name, files, bytes, destination}` | accept policy `Ask` | `incoming`, `d-incoming`, notification |
| `ConflictAsked{id, conflicts, allowed_modes}` | dialog on both sides | `conflict`, `d-conflict` |
| `DecisionMade{id}` | answered on the other side; close the dialog | — |
| `SourceChanged{id, changed, missing}` | queued files changed | `changed`, `d-changed` |
| `LowSpace{id, free, needed}` | receiver almost full | `low-space`, `d-low-space` |
| `Error{message}` | failure outside a job | banner / toast |

## Fake node

Feature `fake` of `localdrop-node` (module `localdrop_node::fake`):

- `Script` — a list of `Step{after, event}`; build with `Script::default().then(ms, event)`.
- `script.play(speed)` spawns a task and returns an event receiver of the same type as
  the real node's. `speed` > 1 plays faster, `0` without delays (tests).
- `fake::demo()` — a story that touches most screens: discovery on, two devices online, an
  outgoing job with progress, an incoming request followed by a conflict, the receiver
  running low on space, a device going offline, a job queued for an offline device, and an
  untrusted network.
- `fake::ids` — fixed device and job ids used by the demo, for UI tests.

Until the bridge exists (M2), the app has a Dart port of the same story
(`app/lib/node/fake_node.dart`, same ids) behind the `NodeClient` interface it will
share with the bridge. It also reacts to commands (accept, conflict answers, job
actions, the discovery switch) so the screens can be clicked through. From M2 the bridge
also exposes a `fake_events(speed)` stream from the Rust fake, so the app can switch
between fake and real nodes in a debug menu.
