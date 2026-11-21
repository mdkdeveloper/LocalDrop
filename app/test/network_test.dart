import 'package:flutter_test/flutter_test.dart';
import 'package:localdrop/node/fake_node.dart';
import 'package:localdrop/node/models.dart';
import 'package:localdrop/ui/networks.dart';
import 'package:material_ui/material_ui.dart';

import 'state_test.dart' show joined, pumpEvents;
import 'widget_test.dart' show join, pumpApp;

const desktop = Size(1280, 800);
const phone = Size(390, 844);

/// Like pumpAndSettle, for screens with a spinner that never stops ("Searching…").
Future<void> settle(WidgetTester tester) async {
  for (var i = 0; i < 10; i++) {
    await tester.pump(const Duration(milliseconds: 100));
  }
}

/// Closes the incoming request that the demo story opens on its own.
Future<void> decline(WidgetTester tester) async {
  await tester.tap(find.text('Decline'));
  await tester.pumpAndSettle();
}

void main() {
  group('state', () {
    test('networks load; trusting one adds it to the search', () async {
      final s = await joined();
      expect(s.connectedNetworks.map((n) => n.label), ['Home', 'Office-5G']);
      expect(s.searchingLabel, 'Home');
      expect(s.askProminently, isFalse); // Home keeps discovery running

      await s.answerNetwork(trust: true);
      await pumpEvents();
      expect(s.untrustedNetwork, isNull);
      expect(s.searchingLabel, 'Home + Office-5G');
      expect(s.activeNetworks, 2);
    });

    test('never ask, rename and forget a remembered network', () async {
      final s = await joined();
      await s.answerNetwork(trust: false, neverAsk: true);
      expect(s.networkFor('5f0c9a1e')!.trust, NetworkTrust.neverAsk);

      await s.renameNetwork(FakeIds.parents, 'Mum and Dad');
      expect(s.networkFor(FakeIds.parents)!.label, 'Mum and Dad');

      await s.forgetNetwork(FakeIds.cafe);
      expect(s.networkFor(FakeIds.cafe), isNull);
      expect(s.lastError, isNull);
    });

    test('network menu selection and trust every network', () async {
      final s = await joined();
      await s.setNetworkTrust(FakeIds.office, NetworkTrust.trusted);
      await s.setNetworkSettings(
        s.networkSettings.copyWith(onlyNetwork: () => FakeIds.office),
      );
      await pumpEvents();
      expect(s.searchingLabel, 'Office-5G');

      await s.setNetworkSettings(
        s.networkSettings.copyWith(
          onlyNetwork: () => null,
          trustEveryNetwork: true,
        ),
      );
      await s.setNetworkTrust(FakeIds.home, NetworkTrust.notTrusted);
      await pumpEvents();
      expect(s.searchingLabel, 'Home + Office-5G'); // trusted anyway
    });

    test(
      'no trusted network makes the ask prominent; off searches nowhere',
      () async {
        final s = await joined();
        await s.setNetworkTrust(FakeIds.home, NetworkTrust.notTrusted);
        await pumpEvents();
        expect(s.askProminently, isTrue);
        expect(s.searchingLabel, isNull);

        await s.setNetworkTrust(FakeIds.home, NetworkTrust.trusted);
        await s.setDiscovery(false);
        await pumpEvents();
        expect(s.searchingOn, isEmpty);
        expect(s.activeNetworks, 0);
      },
    );

    test('address check', () {
      expect(addressProblem('', ''), isNotNull);
      expect(addressProblem('desk pc', ''), isNotNull);
      expect(addressProblem('192.168.1.42', '0'), isNotNull);
      expect(addressProblem('192.168.1.42', 'x'), isNotNull);
      expect(addressProblem('desk-pc.local', ''), isNull);
      expect(addressProblem('10.0.0.2', '47474'), isNull);
    });
  });

  group('desktop', () {
    testWidgets('network menu, then Settings › Networks', (tester) async {
      await pumpApp(tester, size: desktop);
      await join(tester);
      await decline(tester);

      expect(find.text('Discovery · Home'), findsOneWidget);
      await tester.tap(find.byKey(const Key('discovery-label')));
      await tester.pumpAndSettle();
      expect(find.text('Search on'), findsOneWidget);
      expect(find.text('Office-5G'), findsOneWidget);

      await tester.tap(find.byKey(const Key('trust-${FakeIds.office}')));
      await tester.pumpAndSettle();
      expect(find.text('Only Home'), findsOneWidget); // two trusted now

      await tester.tap(find.text('Network settings'));
      await tester.pumpAndSettle();
      expect(find.text('Search on'), findsNothing);
      expect(find.text('REMEMBERED NETWORKS'), findsOneWidget);
      expect(find.text('Parents’ Wi-Fi'), findsOneWidget);
      expect(find.text('Discovery · Home + Office-5G'), findsOneWidget);
    });

    testWidgets('untrusted network without a trusted one takes the main area', (
      tester,
    ) async {
      final s = await pumpApp(tester, size: desktop);
      await join(tester);
      await decline(tester);

      await s.setNetworkTrust(FakeIds.home, NetworkTrust.notTrusted);
      await tester.pumpAndSettle();
      expect(find.text('Network not trusted'), findsOneWidget);
      expect(find.text('No trusted network'), findsOneWidget);
      expect(find.textContaining('10.20.1.57/22'), findsOneWidget);

      await tester.tap(find.text('Trust this network'));
      await tester.pumpAndSettle();
      expect(find.text('Network not trusted'), findsNothing);
      expect(find.text('Discovery · Office-5G'), findsOneWidget);
    });

    testWidgets('looking for devices, connect by address', (tester) async {
      final s = await pumpApp(tester, size: desktop);
      await join(tester);
      await decline(tester);
      s.apply(const PeerOffline(FakeIds.laptop));
      await settle(tester);

      await tester.tap(find.text('Searching on Home…'));
      await settle(tester);
      expect(find.text('Looking for your devices…'), findsOneWidget);
      expect(
        find.textContaining('must show ${s.group!.fingerprint}'),
        findsOneWidget,
      );

      await tester.tap(find.byKey(const Key('connect-by-address')));
      await settle(tester);
      await tester.tap(find.byKey(const Key('connect')));
      await tester.pump();
      expect(find.text('Enter an IP address or a name.'), findsOneWidget);

      await tester.enterText(find.byKey(const Key('address')), 'example.org');
      await tester.tap(find.byKey(const Key('connect')));
      await settle(tester);
      expect(
        find.textContaining('No LocalDrop device answered'),
        findsOneWidget,
      );

      await tester.enterText(find.byKey(const Key('address')), '192.168.1.42');
      await tester.tap(find.byKey(const Key('connect')));
      await settle(tester);
      expect(find.text('Connect by address'), findsNothing);
      expect(find.text('Desk PC found  ›'), findsOneWidget);

      await tester.tap(find.text('Desk PC found  ›'));
      await settle(tester);
      expect(find.text('Desk PC'), findsWidgets);
      expect(find.text('Queued for this device'.toUpperCase()), findsOneWidget);
    });

    testWidgets('discovery off on a device page', (tester) async {
      await pumpApp(tester, size: desktop);
      await join(tester);
      await decline(tester);
      await tester.tap(find.text('Work laptop').first);
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('discovery-switch')));
      await tester.pumpAndSettle();
      expect(find.text('Discovery is off'), findsOneWidget);
      expect(find.textContaining('Transfers paused'), findsOneWidget);

      await tester.tap(find.text('Turn on discovery'));
      await tester.pumpAndSettle();
      expect(find.text('Discovery is off'), findsNothing);
    });
  });

  group('phone', () {
    testWidgets('discovery off, paused bar leads to transfers', (tester) async {
      await pumpApp(tester, size: phone);
      await join(tester);
      await decline(tester);
      await tester.tap(find.text('Files'));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('discovery-switch')));
      await tester.pumpAndSettle();
      expect(find.text('Discovery is off'), findsOneWidget);
      await tester.tap(find.text('View'));
      await tester.pumpAndSettle();
      expect(find.text('Holiday photos'), findsOneWidget);
    });

    testWidgets('new network sheet when nothing trusted is connected', (
      tester,
    ) async {
      final s = await pumpApp(tester, size: phone);
      await join(tester);
      await decline(tester);

      await s.setNetworkTrust(FakeIds.home, NetworkTrust.notTrusted);
      await tester.pumpAndSettle();
      expect(find.text('Trust “Office-5G”?'), findsOneWidget);
      await tester.tap(find.text('Never ask'));
      await tester.pumpAndSettle();
      expect(find.text('Trust “Office-5G”?'), findsNothing);
      expect(s.networkFor(FakeIds.office)!.trust, NetworkTrust.neverAsk);
    });

    testWidgets('Settings › Networks page', (tester) async {
      await pumpApp(tester, size: phone);
      await join(tester);
      await decline(tester);
      await tester.tap(find.text('Settings'));
      await tester.pumpAndSettle();
      expect(find.text('Searching on Home'), findsOneWidget);

      await tester.tap(find.byKey(const Key('settings-networks')));
      await tester.pumpAndSettle();
      expect(
        find.text('Discovery runs only on trusted networks'),
        findsOneWidget,
      );
      expect(find.text('CONNECTED NOW'), findsOneWidget);
      await tester.scrollUntilVisible(find.text('Café Wi-Fi'), 200);
      expect(find.text('Never ask'), findsWidgets);
    });
  });
}
