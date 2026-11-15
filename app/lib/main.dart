import 'package:material_ui/material_ui.dart';

import 'app.dart';
import 'node/fake_node.dart';
import 'state/app_state.dart';

/// Until the Rust bridge lands (M2) the app runs on the fake node.
/// `--dart-define=FAKE_SPEED=4` plays the demo story faster.
void main() {
  const speed = int.fromEnvironment('FAKE_SPEED', defaultValue: 1);
  runApp(LocalDropApp(state: AppState(FakeNode(speed: speed))));
}
