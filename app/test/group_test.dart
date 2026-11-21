import 'package:flutter_test/flutter_test.dart';
import 'package:localdrop/node/fake_node.dart';
import 'package:localdrop/node/models.dart';
import 'package:material_ui/material_ui.dart';

import 'network_test.dart' show decline, desktop, phone;
import 'widget_test.dart' show pumpApp;

const password = 'correct-horse-battery';

Future<void> joinWith(WidgetTester tester) async {
  await tester.enterText(find.byKey(const Key('password')), password);
  await tester.tap(find.byKey(const Key('join')));
  await tester.pumpAndSettle();
  await decline(tester);
}

Future<void> openSettings(WidgetTester tester) async {
  await tester.tap(find.text('Settings').last);
  await tester.pumpAndSettle();
}

void main() {
  group('desktop', () {
    testWidgets(
      'change password checks the current one and shows the new fingerprint',
      (tester) async {
        final s = await pumpApp(tester, size: desktop);
        await joinWith(tester);
        await openSettings(tester);
        final before = s.group!.fingerprint;
        expect(find.text(before), findsOneWidget);

        await tester.tap(find.byKey(const Key('change-password')));
        await tester.pumpAndSettle();
        await tester.enterText(
          find.byKey(const Key('current-password')),
          'wrong-one-x',
        );
        await tester.enterText(find.byKey(const Key('new-password')), 'short');
        await tester.tap(find.byKey(const Key('submit-password')));
        await tester.pump();
        expect(find.textContaining('Too short'), findsOneWidget);

        await tester.enterText(
          find.byKey(const Key('new-password')),
          'a-much-longer-one',
        );
        await tester.tap(find.byKey(const Key('submit-password')));
        await tester.pumpAndSettle();
        expect(find.text('The current password is wrong'), findsOneWidget);

        await tester.enterText(
          find.byKey(const Key('current-password')),
          password,
        );
        await tester.tap(find.byKey(const Key('submit-password')));
        await tester.pumpAndSettle();
        expect(find.text('Password changed'), findsOneWidget);
        final after = s.group!.fingerprint;
        expect(after, isNot(before));
        expect(find.byKey(const Key('new-fingerprint')), findsOneWidget);

        await tester.tap(find.text('Done'));
        await tester.pumpAndSettle();
        expect(find.text(after), findsOneWidget);
      },
    );

    testWidgets('leave the group, forget devices, join again', (tester) async {
      final s = await pumpApp(tester, size: desktop);
      await joinWith(tester);
      await openSettings(tester);
      await tester.tap(find.byKey(const Key('leave-group')));
      await tester.pumpAndSettle();
      expect(find.text('Leave group “home”?'), findsOneWidget);
      await tester.tap(find.byKey(const Key('forget-devices')));
      await tester.tap(find.byKey(const Key('confirm-leave')));
      await tester.pumpAndSettle();

      expect(s.joined, isFalse);
      expect(find.text('Welcome to LocalDrop'), findsOneWidget);
      expect(await s.node.devices(), isEmpty);

      await tester.enterText(find.byKey(const Key('password')), password);
      await tester.tap(find.byKey(const Key('join')));
      await tester.pumpAndSettle();
      expect(s.joined, isTrue);
    });

    testWidgets('this device: rename and fixed port', (tester) async {
      final s = await pumpApp(tester, size: desktop);
      await joinWith(tester);
      await openSettings(tester);

      await tester.tap(find.byKey(const Key('device-name')));
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const Key('ask-text')), 'Desk');
      await tester.tap(find.text('Rename'));
      await tester.pumpAndSettle();
      expect(s.me!.name, 'Desk');

      await tester.tap(find.byKey(const Key('listening-port')));
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const Key('ask-text')), '47474');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(s.me!.fixedPort, 47474);
      expect(find.text('47474'), findsOneWidget);
    });

    testWidgets('known devices: accept policy and forget', (tester) async {
      final s = await pumpApp(tester, size: desktop);
      await joinWith(tester);
      await openSettings(tester);
      await tester.tap(find.byKey(const Key('tab-devices')));
      await tester.pumpAndSettle();
      expect(find.textContaining('Asks before receiving'), findsOneWidget);

      await tester.tap(find.byKey(const Key('known-${FakeIds.phone}')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Always automatically'));
      await tester.pumpAndSettle();
      expect(s.devices[FakeIds.phone]!.acceptOverride, AcceptPolicy.autoAccept);

      await tester.tap(find.byKey(const Key('forget-device')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('confirm-forget')));
      await tester.pumpAndSettle();
      expect(s.devices, isNot(contains(FakeIds.phone)));
      expect(find.byKey(const Key('known-${FakeIds.phone}')), findsNothing);
    });

    testWidgets('a key mismatch opens a dialog; keep blocked closes it', (
      tester,
    ) async {
      final s = await pumpApp(tester, size: desktop);
      await joinWith(tester);
      s.apply(const KeyMismatch(FakeIds.laptop, 'Work laptop'));
      await tester.pumpAndSettle();
      expect(find.text('Work laptop’s key doesn’t match'), findsOneWidget);
      expect(find.text('Forget Work laptop'), findsOneWidget);
      await tester.tap(find.byKey(const Key('keep-blocked')));
      await tester.pumpAndSettle();
      expect(find.text('Work laptop’s key doesn’t match'), findsNothing);
      expect(s.keyMismatches, isEmpty);
      expect(s.devices, contains(FakeIds.laptop));
    });
  });

  group('phone', () {
    testWidgets('Settings › Group page and leave sheet', (tester) async {
      final s = await pumpApp(tester, size: phone);
      await joinWith(tester);
      await openSettings(tester);
      await tester.tap(find.byKey(const Key('settings-group')));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('fingerprint')), findsOneWidget);
      expect(
        find.text('Must match on every device in the group'),
        findsOneWidget,
      );

      await tester.scrollUntilVisible(
        find.byKey(const Key('leave-group')),
        200,
      );
      await tester.tap(find.byKey(const Key('leave-group')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('confirm-leave')));
      await tester.pumpAndSettle();
      expect(s.joined, isFalse);
      expect(find.text('Join or create your group'), findsOneWidget);
    });

    testWidgets('known devices page opens device details', (tester) async {
      await pumpApp(tester, size: phone);
      await joinWith(tester);
      await openSettings(tester);
      await tester.tap(find.byKey(const Key('settings-devices')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('known-${FakeIds.desktop}')));
      await tester.pumpAndSettle();
      expect(find.text('ACCEPT FILES FROM IT'), findsOneWidget);
      expect(find.text('Like the app default'), findsOneWidget);
    });
  });
}
