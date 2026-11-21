# LocalDrop app

Cross-platform UI in **Flutter** (Windows, Linux, macOS, Android, iOS). It calls the
Rust `localdrop-node` crate through
[`flutter_rust_bridge`](https://github.com/fzyzcjy/flutter_rust_bridge) 2.13 (crate
`rust/`, built by cargokit in `rust_builder/`). A **fake node** that plays a scripted
story is still there, so every screen can be tried without a network
([../docs/node-api.md](../docs/node-api.md#fake-node)); the widget tests use it.

## Running

```bash
flutter run -d windows                                   # real node (needs Rust)
flutter run -d windows --dart-define=LOCALDROP_HOME=C:/tmp/ld-app   # separate state folder
flutter run -d chrome --dart-define=NODE=fake            # scripted demo, also on web
flutter run -d windows --dart-define=NODE=fake --dart-define=FAKE_SPEED=4
flutter analyze
flutter test
```

On Windows, Flutter plugins need symlinks: turn on **Developer Mode**
(`start ms-settings:developers`) once, or `flutter build windows` stops with
"Building with plugins requires symlink support".

After changing `rust/src/api/`, regenerate the bindings and format them:

```bash
flutter_rust_bridge_codegen generate   # in app/; writes lib/src/rust and rust/src/frb_generated.rs
cargo fmt                              # the generated Rust must pass `cargo fmt --check` in CI
```

Requires Flutter 3.50+ (Material comes from the `material_ui` package).

## Layout

```text
lib/
  main.dart            entry point; picks the node (fake until M2)
  app.dart             MaterialApp, light/dark theme, AppScope (state for widgets)
  node/
    models.dart        Dart mirror of the node API types and NodeEvent
    node_client.dart   what the UI needs from a node
    rust_node.dart     the real node through flutter_rust_bridge
    fake_node.dart     scripted demo node (same story and ids as localdrop_node::fake)
  src/rust/            GENERATED bindings (flutter_rust_bridge_codegen)
  state/app_state.dart AppState: builds devices, jobs and pending dialogs from events
  theme/
    tokens.g.dart      colour tokens, GENERATED from ../design/tokens.json
    theme.dart         ThemeData from the tokens; `context.ld.accent`
  ui/                  screens and widgets
tool/gen_tokens.dart   regenerates tokens.g.dart
```

- **Design is the source of truth.** Colours come from `design/tokens.json` (exported by
  `design/tools/styled.py`); after changing them run `dart run tool/gen_tokens.dart`.
  Screens follow the wireframes; each screen's doc comment names its wireframe keys
  (`d-transfers`, `transfers`, …).
- **One layout per width.** At 840 px and wider the desktop layout is used (sidebar with
  devices, Transfers, Settings); below it the phone layout (bottom navigation: Files,
  Transfers, Folders, Settings).
- **Events in, commands out.** Widgets read `AppScope.of(context)` and call its commands;
  `AppState` only changes in response to node events, so the real node can replace the
  fake one without touching the screens.

## Screens so far

| Screen | Wireframes | State |
|---|---|---|
| Join group | `join`, `d-join`, `join-error` | form, password rule, passphrase generator; fingerprint is fake until M2 |
| Shell | sidebar / bottom nav | devices online and offline, discovery switch |
| Transfers | `transfers`, `d-transfers`, `d-low-space`, `empty-transfers` | queues per device and direction, progress, low space, finished |
| Job menu | `job-menu`, `d-job-menu` | actions that fit the state, destructive last |
| Incoming request | `incoming`, `d-incoming` | accept / decline, always accept |
| Conflict | `conflict`, `d-conflict` | allowed modes; first answer wins |
| New network | `new-net`, `d-new-net` | banner: Trust / Not now / Never ask |
| Device | `d-remote`, `d-offline`, `files-offline`, `files-root` | queued jobs, send-ahead hint; shares from M4 |
| Folders, Settings | `folders`, `settings` | placeholders until M4 / M6 |

Not yet: the Inter font (system font for now), the design's own icon set (Material
icons for now), file pickers and drag & drop (need the bridge).
