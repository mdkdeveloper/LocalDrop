import 'package:flutter_test/flutter_test.dart';
import 'package:localdrop/app.dart';
import 'package:localdrop/node/fake_node.dart';
import 'package:localdrop/node/models.dart';
import 'package:localdrop/state/app_state.dart';
import 'package:material_ui/material_ui.dart';

Future<AppState> pumpApp(
  WidgetTester tester, {
  required Size size,
  ThemeMode mode = ThemeMode.light,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = AppState(FakeNode(speed: 0));
  await tester.pumpWidget(LocalDropApp(state: state, themeMode: mode));
  return state;
}

Future<void> join(WidgetTester tester) async {
  await tester.tap(find.text('Generate'));
  await tester.pump();
  await tester.tap(find.byKey(const Key('join')));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('short password is refused with the rule', (tester) async {
    await pumpApp(tester, size: const Size(1280, 800));
    await tester.enterText(find.byKey(const Key('password')), 'sunny1');
    await tester.tap(find.byKey(const Key('join')));
    await tester.pump();
    expect(find.textContaining('Too short: 6 characters'), findsOneWidget);
    expect(find.text('Welcome to LocalDrop'), findsOneWidget);
  });

  testWidgets('desktop: join, answer request and conflict, see transfers', (
    tester,
  ) async {
    final state = await pumpApp(tester, size: const Size(1280, 800));
    await join(tester);

    // Sidebar with the group and devices; the request dialog opened on its own.
    expect(find.text('LocalDrop'), findsOneWidget);
    expect(find.text('Work laptop'), findsWidgets);
    expect(find.text('Pixel 8 wants to send you files'), findsOneWidget);

    await tester.tap(find.text('Accept'));
    await tester.pumpAndSettle();
    expect(find.text('3 files already exist'), findsOneWidget);
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();
    expect(state.jobs[FakeIds.scans]!.state, JobState.inProgress);

    expect(find.text('Transfers'), findsWidgets);
    expect(find.text('Holiday photos'), findsOneWidget);
    expect(find.textContaining('running out of space'), findsOneWidget);
    expect(find.textContaining('Trust “Office-5G”?'), findsOneWidget);

    await tester.tap(find.text('Trust this network'));
    await tester.pumpAndSettle();
    expect(find.textContaining('Trust “Office-5G”?'), findsNothing);

    // Offline device page explains send-ahead.
    await tester.tap(find.text('Living room PC'));
    await tester.pumpAndSettle();
    expect(find.textContaining('go out automatically'), findsOneWidget);
    expect(find.text('Project files'), findsOneWidget);
  });

  testWidgets('phone, dark: bottom navigation and job menu sheet', (
    tester,
  ) async {
    await pumpApp(tester, size: const Size(390, 844), mode: ThemeMode.dark);
    await join(tester);
    await tester.tap(find.text('Decline'));
    await tester.pumpAndSettle();

    expect(find.byType(NavigationBar), findsOneWidget);
    await tester.ensureVisible(find.text('Project files'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Project files'));
    await tester.pumpAndSettle();
    expect(find.text('Pause'), findsOneWidget);
    await tester.tap(find.text('Pause'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Files'));
    await tester.pumpAndSettle();
    expect(find.text('Devices'), findsOneWidget);
    expect(find.text('Known · offline'.toUpperCase()), findsOneWidget);
  });
}
