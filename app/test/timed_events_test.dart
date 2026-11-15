import 'package:flutter_test/flutter_test.dart';
import 'package:localdrop/app.dart';
import 'package:localdrop/node/fake_node.dart';
import 'package:localdrop/state/app_state.dart';
import 'package:material_ui/material_ui.dart';

void main() {
  testWidgets(
    'request dialog opens by itself when the event arrives later (timed fake node)',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final node = FakeNode(speed: 1);
      final state = AppState(node);
      await tester.pumpWidget(
        LocalDropApp(state: state, themeMode: ThemeMode.light),
      );
      await tester.tap(find.text('Generate'));
      await tester.pump();
      await tester.tap(find.byKey(const Key('join')));
      for (var i = 0; i < 40; i++) {
        await tester.pump(const Duration(milliseconds: 200));
      }
      expect(state.decisions, isNotEmpty);
      expect(find.text('Pixel 8 wants to send you files'), findsOneWidget);
      await node.dispose();
    },
  );
}
