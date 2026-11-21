import 'package:material_ui/material_ui.dart';

import 'app.dart';
import 'node/fake_node.dart';
import 'node/node_client.dart';
import 'node/rust_node.dart';
import 'state/app_state.dart';

/// The app runs on the real node (`localdrop-node` through flutter_rust_bridge).
///
/// - `--dart-define=NODE=fake` plays the scripted demo instead (no network, also on web);
///   `--dart-define=FAKE_SPEED=4` plays it faster.
/// - `--dart-define=LOCALDROP_HOME=<folder>` uses another state folder, e.g. to run a
///   second instance next to the CLI on one computer.
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  const kind = String.fromEnvironment('NODE', defaultValue: 'real');
  const home = String.fromEnvironment('LOCALDROP_HOME');
  final NodeClient node;
  if (kind == 'fake') {
    const speed = int.fromEnvironment('FAKE_SPEED', defaultValue: 1);
    node = FakeNode(speed: speed);
  } else {
    await RustNode.init();
    node = RustNode(home: home.isEmpty ? null : home);
  }
  final state = AppState(node);
  await state.restore();
  runApp(LocalDropApp(state: state));
}
