import 'dart:async';

import 'models.dart';
import 'node_client.dart';

/// Ids shared with the Rust fake node (`localdrop_node::fake::ids`).
abstract final class FakeIds {
  static const laptop = '11111111111111111111111111111111';
  static const phone = '22222222222222222222222222222222';
  static const desktop = '33333333333333333333333333333333';
  static const photos = 'a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1';
  static const scans = 'a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2';
  static const project = 'a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3';

  /// Appears after a successful *Connect by address*.
  static const deskPc = '44444444444444444444444444444444';

  // Network fingerprints (64 hex); events use the first 8 characters.
  static const home =
      'c41e7b2093d4c0aa51f2e8b6a7d90c3e1b5f4a2d6e8c0b9a7f3d2e1c4b5a6978';
  static const office =
      '5f0c9a1e7d3b2c4a6e8f0d1b3a5c7e9f2b4d6a8c0e1f3a5b7d9c2e4f6a8b0c1d';
  static const parents =
      '9b2d4f6a8c0e1a3b5d7f9e2c4a6b8d0f1e3a5c7b9d2f4e6a8c0b1d3f5e7a9c2b';
  static const cafe =
      'e7a9c2b4d6f8a0c1e3b5d7f9a2c4e6b8d0f1a3c5e7b9d2f4a6c8e0b1d3f5a7c9';
}

/// A node without a network: plays the same story as `localdrop_node::fake::demo`
/// and reacts to the UI's commands, so every screen can be built and tried out.
///
/// Running jobs advance on a timer at a fixed speed. [speed] > 1 plays faster;
/// 0 plays the script without delays and stops the timer (tests).
class FakeNode implements NodeClient {
  FakeNode({this.speed = 1});

  final int speed;
  final _events = StreamController<NodeEvent>.broadcast();
  final _jobs = <String, JobView>{};
  final _timers = <Timer>[];
  Timer? _ticker;
  bool _discovery = true;
  bool _started = false;
  var _settings = const NetworkSettings();
  final _networks = <NetworkView>[
    const NetworkView(
      key: FakeIds.home,
      label: 'Home',
      kind: NetworkKind.wifi,
      address: '192.168.1.23/24',
      lastSeenUnix: _created,
      connected: true,
      active: true,
      trust: NetworkTrust.trusted,
    ),
    const NetworkView(
      key: FakeIds.parents,
      label: 'Parents’ Wi-Fi',
      kind: NetworkKind.wifi,
      lastSeenUnix: _created - 21 * 86400,
      connected: false,
      active: false,
      trust: NetworkTrust.trusted,
      weakIdentity: true,
    ),
    const NetworkView(
      key: FakeIds.cafe,
      label: 'Café Wi-Fi',
      kind: NetworkKind.wifi,
      lastSeenUnix: _created - 86400,
      connected: false,
      active: false,
      trust: NetworkTrust.neverAsk,
    ),
  ];

  static const _bytesPerSecond = 11400000;
  static const _created = 1763200000;

  @override
  Stream<NodeEvent> get events => _events.stream;

  /// The demo always starts at the join screen.
  @override
  Future<({GroupInfo group, String deviceName})?> restore() async => null;

  @override
  Future<GroupInfo> join({
    required String groupName,
    required String password,
    required String deviceName,
    required bool trustThisNetwork,
  }) async {
    await _wait(600); // Argon2id takes ~0.3–1 s on the real node.
    final name = groupName.trim().toLowerCase();
    return GroupInfo(name, _fakeFingerprint('$name\u0000$password'));
  }

  @override
  Future<List<DeviceView>> devices() async => const [
    DeviceView(id: FakeIds.laptop, name: 'Work laptop', lastSeenUnix: _created),
    DeviceView(id: FakeIds.phone, name: 'Pixel 8', lastSeenUnix: _created),
    DeviceView(
      id: FakeIds.desktop,
      name: 'Living room PC',
      lastSeenUnix: _created - 7200,
    ),
  ];

  @override
  Future<List<JobView>> jobs() async => _jobs.values.toList();

  /// Starts the story.
  @override
  Future<void> start() async {
    if (_started) return;
    _started = true;
    if (speed > 0) {
      _ticker = Timer.periodic(
        const Duration(milliseconds: 250),
        (_) => _tick(0.25 * speed),
      );
    }
    _script([
      (0, const DiscoveryChanged(true, 1)),
      (800, const PeerFound(FakeIds.laptop)),
      (300, const PeerOnline(FakeIds.laptop, 'Work laptop')),
      (600, const PeerOnline(FakeIds.phone, 'Pixel 8')),
      (
        1000,
        JobAdded(
          _job(
            FakeIds.photos,
            FakeIds.laptop,
            Direction.outgoing,
            'Holiday photos',
            1820,
            3200000000,
            JobState.waiting,
          ),
        ),
      ),
      (200, const JobStateChanged(FakeIds.photos, JobState.inProgress)),
      (
        2500,
        JobAdded(
          _job(
            FakeIds.scans,
            FakeIds.phone,
            Direction.incoming,
            'Scans',
            12,
            48000000,
            JobState.awaitingAccept,
          ),
        ),
      ),
      (
        0,
        const IncomingRequest(
          FakeIds.scans,
          FakeIds.phone,
          'Scans',
          12,
          48000000,
        ),
      ),
      (6000, const LowSpace(FakeIds.photos, 1200000000, 3400000000)),
      (0, const JobStateChanged(FakeIds.photos, JobState.lowSpace)),
      (4000, const PeerOffline(FakeIds.phone)),
      (
        500,
        JobAdded(
          _job(
            FakeIds.project,
            FakeIds.desktop,
            Direction.outgoing,
            'Project files',
            340,
            2000000000,
            JobState.waiting,
            deferred: true,
          ),
        ),
      ),
      (1500, const UntrustedNetwork('5f0c9a1e', 'Office-5G')),
    ]);
  }

  @override
  Future<void> setDiscovery(bool on) async {
    _discovery = on;
    _emit(DiscoveryChanged(on, _activeCount));
    for (final j in _jobs.values.where(
      (j) => j.state == JobState.inProgress && !on,
    )) {
      _setState(j.id, JobState.waiting);
    }
  }

  @override
  Future<void> jobAction(String id, JobAction action) async {
    final job = _jobs[id];
    if (job == null) throw StateError('no such job');
    switch (action) {
      case JobAction.pause
          when job.state.isActive && job.state != JobState.paused:
        _setState(id, JobState.paused);
      case JobAction.resume when job.state == JobState.paused:
        _setState(id, _canRun(job) ? JobState.inProgress : JobState.waiting);
      case JobAction.cancel
          when job.state.isActive || job.state == JobState.noAnswer:
        _setState(id, JobState.cancelled);
      case JobAction.clear when !job.state.isActive:
        _jobs.remove(id);
        _emit(JobRemoved(id));
      case JobAction.moveToTop || JobAction.retryFailed || JobAction.offerAgain:
        break; // order and retries only matter with a real engine
      default:
        throw StateError(
          '${action.name} is not possible while ${job.state.name}',
        );
    }
  }

  @override
  Future<void> answerRequest(
    String id, {
    required bool accept,
    required bool always,
  }) async {
    _emit(DecisionMade(id));
    if (!accept) {
      _setState(id, JobState.rejected);
      return;
    }
    _setState(id, JobState.awaitingConflictDecision);
    _script([
      (
        400,
        ConflictAsked(id, 3, const [
          ConflictMode.keepBoth,
          ConflictMode.keepExisting,
          ConflictMode.replaceWithBackup,
        ]),
      ),
    ]);
  }

  @override
  Future<void> answerConflict(String id, ConflictMode? mode) async {
    _emit(DecisionMade(id));
    _setState(id, mode == null ? JobState.cancelled : JobState.inProgress);
  }

  @override
  Future<List<NetworkView>> networks() async => [
    ..._networks.where((n) => n.connected),
    ..._networks.where((n) => !n.connected),
  ];

  @override
  Future<void> setNetworkTrust(String key, NetworkTrust trust) async {
    final i = _network(key);
    _networks[i] = _networks[i].copyWith(trust: trust);
    _replan();
  }

  @override
  Future<void> renameNetwork(String key, String label) async {
    final name = label.trim();
    if (name.isEmpty) throw StateError('A network needs a name');
    final i = _network(key);
    _networks[i] = _networks[i].copyWith(label: name);
  }

  @override
  Future<void> forgetNetwork(String key) async {
    final i = _network(key);
    if (_networks[i].connected) {
      // Connected networks stay listed, but the decision is gone.
      _networks[i] = _networks[i].copyWith(trust: NetworkTrust.notTrusted);
    } else {
      _networks.removeAt(i);
    }
    _replan();
  }

  @override
  Future<NetworkSettings> networkSettings() async => _settings;

  @override
  Future<void> setNetworkSettings(NetworkSettings settings) async {
    _settings = settings;
    _replan();
  }

  /// Succeeds for `192.168.1.*` and `*.local`, as if "Desk PC" answered there.
  @override
  Future<void> connectAddress(
    String host, {
    int? port,
    required bool remember,
  }) async {
    final h = host.trim();
    if (h.isEmpty) throw StateError('Enter an address');
    await _wait(800);
    if (!h.startsWith('192.168.1.') && !h.endsWith('.local')) {
      throw StateError(
        'No LocalDrop device answered at $h:${port ?? defaultPort}. '
        'Check the address and that LocalDrop is open there.',
      );
    }
    _emit(const PeerFound(FakeIds.deskPc));
    _emit(const PeerOnline(FakeIds.deskPc, 'Desk PC'));
  }

  /// Port shown as "Auto" in *Connect by address*.
  static const defaultPort = 47474;

  @override
  Future<void> dispose() async {
    _ticker?.cancel();
    for (final t in _timers) {
      t.cancel();
    }
    await _events.close();
  }

  // -- internals -------------------------------------------------------------------

  bool _canRun(JobView j) => _discovery && j.peer != FakeIds.desktop;

  int _network(String key) {
    final i = _networks.indexWhere((n) => n.matches(key));
    if (i < 0) throw StateError('No such network');
    return i;
  }

  bool _isActive(NetworkView n) =>
      n.connected &&
      (_settings.trustEveryNetwork || n.trust == NetworkTrust.trusted) &&
      (_settings.onlyNetwork == null || n.matches(_settings.onlyNetwork!));

  int get _activeCount =>
      _discovery ? _networks.where((n) => n.active).length : 0;

  /// Applies trust rules and the network menu, like `network::plan` in Rust.
  void _replan() {
    for (var i = 0; i < _networks.length; i++) {
      _networks[i] = _networks[i].copyWith(active: _isActive(_networks[i]));
    }
    _emit(DiscoveryChanged(_discovery, _activeCount));
  }

  void _tick(double seconds) {
    for (final j in _jobs.values.toList()) {
      if (j.state != JobState.inProgress) continue;
      final done = (j.bytesDone + _bytesPerSecond * seconds).round().clamp(
        0,
        j.bytesTotal,
      );
      _jobs[j.id] = j.copyWith(bytesDone: done, speedBps: _bytesPerSecond);
      _emit(JobProgress(j.id, done, j.bytesTotal, _bytesPerSecond));
      if (done >= j.bytesTotal) _setState(j.id, JobState.completed);
    }
  }

  void _setState(String id, JobState state) {
    final j = _jobs[id];
    if (j == null || j.state == state) return;
    _jobs[id] = j.copyWith(state: state);
    _emit(JobStateChanged(id, state));
  }

  void _emit(NodeEvent e) {
    if (e is JobAdded) _jobs[e.job.id] = e.job;
    if (e is UntrustedNetwork && !_networks.any((n) => n.matches(e.key))) {
      // A second network came up next to Home; it isn't trusted yet.
      _networks.insert(
        1,
        NetworkView(
          key: FakeIds.office,
          label: e.label,
          kind: NetworkKind.wifi,
          address: '10.20.1.57/22',
          lastSeenUnix: _created,
          connected: true,
          active: false,
          trust: NetworkTrust.notTrusted,
        ),
      );
    }
    if (e is JobStateChanged) {
      final j = _jobs[e.id];
      if (j == null) return;
      _jobs[e.id] = j.copyWith(state: e.state);
    }
    if (!_events.isClosed) _events.add(e);
  }

  void _script(List<(int, NodeEvent)> steps) {
    if (speed == 0) {
      for (final (_, e) in steps) {
        _emit(e);
      }
      return;
    }
    var at = 0;
    for (final (ms, e) in steps) {
      at += ms;
      _timers.add(Timer(Duration(milliseconds: at ~/ speed), () => _emit(e)));
    }
  }

  Future<void> _wait(int ms) => speed == 0
      ? Future.value()
      : Future.delayed(Duration(milliseconds: ms ~/ speed));

  static JobView _job(
    String id,
    String peer,
    Direction dir,
    String name,
    int files,
    int bytes,
    JobState state, {
    bool deferred = false,
  }) => JobView(
    id: id,
    peer: peer,
    direction: dir,
    name: name,
    deferred: deferred,
    filesTotal: files,
    bytesTotal: bytes,
    state: state,
    createdUnix: _created,
  );

  /// Looks like a real fingerprint (`3fa2-91c0`) and is stable for the same input,
  /// but is NOT the real derivation; that comes from the Rust core.
  static String _fakeFingerprint(String input) {
    var h = 0x811c9dc5;
    for (final c in input.codeUnits) {
      h = ((h ^ c) * 0x01000193) & 0xffffffff;
    }
    final hex = h.toRadixString(16).padLeft(8, '0');
    return '${hex.substring(0, 4)}-${hex.substring(4)}';
  }
}
