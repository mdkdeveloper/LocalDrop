import 'dart:math';

import 'package:flutter_test/flutter_test.dart';
import 'package:localdrop/node/fake_node.dart';
import 'package:localdrop/node/models.dart';
import 'package:localdrop/state/app_state.dart';
import 'package:localdrop/ui/format.dart';
import 'package:localdrop/ui/join_screen.dart';

Future<AppState> joined() async {
  final s = AppState(FakeNode(speed: 0));
  await s.join(
    groupName: ' Home ',
    password: 'correct-horse',
    deviceName: 'test',
    trustThisNetwork: true,
  );
  await pumpEvents();
  return s;
}

/// Lets the broadcast stream deliver queued events.
Future<void> pumpEvents() => Future<void>.delayed(Duration.zero);

void main() {
  test('join normalizes the group and gives a stable fingerprint', () async {
    final a = await joined();
    final b = await joined();
    expect(a.group!.name, 'home');
    expect(a.group!.fingerprint, matches(RegExp(r'^[0-9a-f]{4}-[0-9a-f]{4}$')));
    expect(a.group!.fingerprint, b.group!.fingerprint);
  });

  test('demo story fills devices, jobs and decisions', () async {
    final s = await joined();
    expect(s.discoveryOn, isTrue);
    expect(s.onlineDevices.map((d) => d.id), [FakeIds.laptop]);
    expect(s.devices[FakeIds.phone]!.presence, Presence.offline);
    expect(s.jobs[FakeIds.photos]!.state, JobState.lowSpace);
    expect(s.lowSpace, contains(FakeIds.photos));
    expect(s.devices[FakeIds.desktop]!.queuedJobs, 1);
    expect(s.decisions.single, isA<RequestDecision>());
    expect(s.untrustedNetwork?.label, 'Office-5G');
  });

  test(
    'accepting a request leads to a conflict, answering it starts the job',
    () async {
      final s = await joined();
      await s.answerRequest(FakeIds.scans, accept: true);
      await pumpEvents();
      expect(s.decisions.single, isA<ConflictDecision>());
      expect(s.jobs[FakeIds.scans]!.state, JobState.awaitingConflictDecision);

      await s.answerConflict(FakeIds.scans, ConflictMode.keepBoth);
      await pumpEvents();
      expect(s.decisions, isEmpty);
      expect(s.jobs[FakeIds.scans]!.state, JobState.inProgress);
    },
  );

  test('job actions follow states, invalid ones become an error', () async {
    final s = await joined();
    await s.jobAction(FakeIds.project, JobAction.pause);
    await pumpEvents();
    expect(s.jobs[FakeIds.project]!.state, JobState.paused);

    await s.jobAction(FakeIds.project, JobAction.clear);
    expect(s.lastError, isNotNull);

    await s.jobAction(FakeIds.project, JobAction.cancel);
    await s.jobAction(FakeIds.project, JobAction.clear);
    await pumpEvents();
    expect(s.jobs, isNot(contains(FakeIds.project)));
    expect(s.devices[FakeIds.desktop]!.queuedJobs, 0);
  });

  test('discovery off pauses running jobs', () async {
    final s = await joined();
    await s.answerRequest(FakeIds.scans, accept: true);
    await s.answerConflict(FakeIds.scans, ConflictMode.keepBoth);
    await s.setDiscovery(false);
    await pumpEvents();
    expect(s.discoveryOn, isFalse);
    expect(s.jobs[FakeIds.scans]!.state, JobState.waiting);
  });

  group('format', () {
    test('bytes, speed and eta', () {
      expect(formatBytes(512), '512 B');
      expect(formatBytes(2400000), '2.4 MB');
      expect(formatBytes(3200000000), '3.2 GB');
      expect(formatBytes(180000000), '180 MB');
      expect(formatSpeed(11400000), '11.4 MB/s');
      expect(formatEta(30, 1), '30 s left');
      expect(formatEta(180000000, 1000000), '3 min left');
      expect(formatEta(1, 0), '');
    });

    test('actions fit the state', () {
      JobView j(JobState s) => JobView(
        id: 'x',
        peer: 'p',
        direction: Direction.outgoing,
        name: 'n',
        filesTotal: 1,
        bytesTotal: 1,
        createdUnix: 0,
        state: s,
      );
      expect(actionsFor(j(JobState.inProgress)), [
        JobAction.pause,
        JobAction.cancel,
      ]);
      expect(actionsFor(j(JobState.paused)).first, JobAction.resume);
      expect(actionsFor(j(JobState.completed)), [JobAction.clear]);
      expect(
        actionLabel(JobAction.cancel, j(JobState.waiting)),
        'Remove from queue',
      );
    });
  });

  group('password', () {
    test('rule and generator', () {
      expect(passwordProblem(''), isNotNull);
      expect(passwordProblem('sunny1'), contains('6 characters'));
      expect(passwordProblem('0123456789'), isNull);
      final p = generatePassphrase(Random(1));
      expect(p.split('-'), hasLength(4));
      expect(passwordProblem(p), isNull);
    });
  });
}
