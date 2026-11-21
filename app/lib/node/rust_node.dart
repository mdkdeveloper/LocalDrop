import 'dart:async';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show AnyhowException;

import '../src/rust/api/node.dart' as rs;
import '../src/rust/frb_generated.dart';
import 'models.dart';
import 'node_client.dart';

/// The real node: `localdrop-node` through flutter_rust_bridge (`app/rust`).
///
/// Errors from Rust arrive as [AnyhowException]; they are rethrown as [StateError] with
/// the node's message, like the fake node does, so the UI shows them as they are.
class RustNode implements NodeClient {
  RustNode({String? home}) : home = home ?? rs.defaultHome();

  /// State folder (identity, group key, settings).
  final String home;

  rs.LdNode? _node;
  StreamSubscription<rs.EventDto>? _sub;
  final _events = StreamController<NodeEvent>.broadcast();

  /// Loads the native library. Call once before creating a [RustNode].
  static Future<void> init() => RustLib.init();

  rs.LdNode get _n => _node ?? (throw StateError('Not in a group yet'));

  @override
  Stream<NodeEvent> get events => _events.stream;

  Future<T> _guard<T>(Future<T> Function() f) async {
    try {
      return await f();
    } on AnyhowException catch (e) {
      throw StateError(e.message.split('\n').first);
    }
  }

  T _sync<T>(T Function() f) {
    try {
      return f();
    } on AnyhowException catch (e) {
      throw StateError(e.message.split('\n').first);
    }
  }

  Future<void> _open() async {
    await _sub?.cancel();
    _node?.dispose();
    _node = await _guard(() => rs.LdNode.open(home: home));
    _sub = _n.events().listen((e) {
      final m = _event(e);
      if (m != null && !_events.isClosed) _events.add(m);
    });
  }

  GroupInfo _group() {
    final g = _n.groupInfo();
    return GroupInfo(g.name, g.fingerprint);
  }

  @override
  Future<({GroupInfo group, String deviceName})?> restore() async {
    if (!rs.isJoined(home: home)) return null;
    await _open();
    return (group: _group(), deviceName: _n.deviceName());
  }

  @override
  Future<GroupInfo> join({
    required String groupName,
    required String password,
    required String deviceName,
    required bool trustThisNetwork,
  }) async {
    await _guard(
      () => rs.joinGroup(
        home: home,
        groupName: groupName,
        password: password,
        deviceName: deviceName,
        trustThisNetwork: trustThisNetwork,
      ),
    );
    await _open();
    return _group();
  }

  @override
  Future<List<DeviceView>> devices() async =>
      _sync(() => _n.devices()).map(_device).toList();

  @override
  Future<List<JobView>> jobs() async =>
      _sync(() => _n.jobs()).map(_job).toList();

  @override
  Future<void> start() => _guard(() => _n.start());

  @override
  Future<void> setDiscovery(bool on) => _guard(() => _n.setDiscovery(on_: on));

  @override
  Future<void> jobAction(String id, JobAction action) async => _sync(
    () => _n.jobAction(
      id: id,
      action: rs.JobActionDto.values.byName(action.name),
    ),
  );

  @override
  Future<void> answerRequest(
    String id, {
    required bool accept,
    required bool always,
  }) async =>
      _sync(() => _n.answerRequest(id: id, accept: accept, always: always));

  @override
  Future<void> answerConflict(String id, ConflictMode? mode) async =>
      throw StateError('Conflict decisions come with shared folders (M4)');

  @override
  Future<List<NetworkView>> networks() async =>
      _sync(() => _n.networks()).map(_network).toList();

  @override
  Future<void> setNetworkTrust(String key, NetworkTrust trust) => _guard(
    () => _n.setNetworkTrust(
      key: key,
      trust: rs.NetworkTrustDto.values.byName(trust.name),
    ),
  );

  @override
  Future<void> renameNetwork(String key, String label) async =>
      _sync(() => _n.renameNetwork(key: key, label: label));

  @override
  Future<void> forgetNetwork(String key) async =>
      _sync(() => _n.forgetNetwork(key: key));

  @override
  Future<NetworkSettings> networkSettings() async {
    final s = _n.networkSettings();
    return NetworkSettings(
      discoverAtLaunch: s.discoverAtLaunch,
      askOnNewNetwork: s.askOnNewNetwork,
      trustEveryNetwork: s.trustEveryNetwork,
      onlyNetwork: s.onlyNetwork,
    );
  }

  @override
  Future<void> setNetworkSettings(NetworkSettings settings) => _guard(
    () => _n.setNetworkSettings(
      settings: rs.NetworkSettingsDto(
        discoverAtLaunch: settings.discoverAtLaunch,
        askOnNewNetwork: settings.askOnNewNetwork,
        trustEveryNetwork: settings.trustEveryNetwork,
        onlyNetwork: settings.onlyNetwork,
      ),
    ),
  );

  @override
  Future<void> connectAddress(
    String host, {
    int? port,
    required bool remember,
  }) => _guard(
    () => _n.connectAddress(
      address: port == null ? host.trim() : '${host.trim()}:$port',
      remember: remember,
    ),
  );

  @override
  Future<ThisDevice> thisDevice() async => ThisDevice(
    id: _n.deviceId(),
    name: _n.deviceName(),
    visible: _n.visible(),
    fixedPort: _n.fixedPort(),
    port: _n.listenPort(),
  );

  @override
  Future<void> setDeviceName(String name) async =>
      _sync(() => _n.setDeviceName(name: name));

  @override
  Future<void> setVisible(bool visible) =>
      _guard(() => _n.setVisible(visible: visible));

  @override
  Future<void> setFixedPort(int port) =>
      _guard(() => _n.setFixedPort(port: port));

  @override
  Future<GroupInfo> changePassword(String current, String newPassword) async {
    final g = await _guard(
      () => _n.changePassword(current: current, new_: newPassword),
    );
    return GroupInfo(g.name, g.fingerprint);
  }

  @override
  Future<void> leaveGroup({
    required bool keepQueue,
    required bool forgetDevices,
  }) async {
    await _guard(
      () => _n.leaveGroup(keepQueue: keepQueue, forgetDevices: forgetDevices),
    );
    await _sub?.cancel();
    _sub = null;
    _node?.dispose();
    _node = null;
  }

  @override
  Future<void> setDeviceAccept(String device, AcceptPolicy? policy) async =>
      _sync(
        () => _n.setDeviceAccept(
          device: device,
          policy: switch (policy) {
            AcceptPolicy.autoAccept => rs.AcceptPolicyDto.autoAccept,
            AcceptPolicy.ask => rs.AcceptPolicyDto.ask,
            null => null,
          },
        ),
      );

  @override
  Future<void> forgetDevice(String device) async =>
      _sync(() => _n.forgetDevice(device: device));

  @override
  Future<void> dispose() async {
    await _sub?.cancel();
    final n = _node;
    if (n != null) {
      try {
        await n.stop();
      } catch (_) {}
      n.dispose();
    }
    await _events.close();
  }

  // -- mapping ---------------------------------------------------------------------

  static DeviceView _device(rs.DeviceDto d) => DeviceView(
    id: d.id,
    name: d.name,
    presence: Presence.values.byName(d.presence.name),
    lastSeenUnix: d.lastSeenUnix.toInt(),
    acceptOverride: switch (d.acceptOverride) {
      rs.AcceptPolicyDto.autoAccept => AcceptPolicy.autoAccept,
      rs.AcceptPolicyDto.ask => AcceptPolicy.ask,
      null => null,
    },
    queuedJobs: d.queuedJobs,
  );

  static Destination _destination(rs.DestinationDto d) => d.shareId.isEmpty
      ? const InboxDestination()
      : ShareDestination(d.shareId, '', d.path);

  static JobState _state(rs.JobStateDto s) => JobState.values.byName(s.name);

  static JobView _job(rs.JobDto j) => JobView(
    id: j.id,
    peer: j.peer,
    direction: Direction.values.byName(j.direction.name),
    name: j.name,
    destination: _destination(j.destination),
    deferred: j.deferred_,
    filesTotal: j.filesTotal,
    bytesTotal: j.bytesTotal.toInt(),
    bytesDone: j.bytesDone.toInt(),
    state: _state(j.state),
    createdUnix: j.createdUnix.toInt(),
  );

  static NetworkView _network(rs.NetworkDto n) => NetworkView(
    key: n.key,
    label: n.label,
    kind: n.kind == rs.NetworkKindDto.wiFi
        ? NetworkKind.wifi
        : NetworkKind.ethernet,
    address: n.address,
    lastSeenUnix: n.lastSeenUnix.toInt(),
    connected: n.connected,
    active: n.active,
    trust: NetworkTrust.values.byName(n.trust.name),
    weakIdentity: n.weakIdentity,
  );

  static NodeEvent? _event(rs.EventDto e) => switch (e.kind) {
    rs.EventKind.discoveryChanged => DiscoveryChanged(e.on_, e.count),
    rs.EventKind.untrustedNetwork => UntrustedNetwork(e.id, e.text),
    rs.EventKind.peerFound => PeerFound(e.id),
    rs.EventKind.peerOnline => PeerOnline(e.id, e.text),
    rs.EventKind.peerOffline => PeerOffline(e.id),
    rs.EventKind.keyMismatch => KeyMismatch(e.id, e.text),
    rs.EventKind.jobAdded => e.job == null ? null : JobAdded(_job(e.job!)),
    rs.EventKind.jobStateChanged =>
      e.state == null ? null : JobStateChanged(e.id, _state(e.state!)),
    rs.EventKind.jobProgress => JobProgress(
      e.id,
      e.a.toInt(),
      e.b.toInt(),
      e.c.toInt(),
    ),
    rs.EventKind.jobRemoved => JobRemoved(e.id),
    rs.EventKind.incomingRequest => IncomingRequest(
      e.id,
      e.from,
      e.text,
      e.count,
      e.a.toInt(),
      e.destination == null
          ? const InboxDestination()
          : _destination(e.destination!),
    ),
    rs.EventKind.conflictAsked => ConflictAsked(e.id, e.count, const [
      ConflictMode.keepBoth,
      ConflictMode.keepExisting,
      ConflictMode.replaceWithBackup,
    ]),
    rs.EventKind.decisionMade => DecisionMade(e.id),
    rs.EventKind.lowSpace => LowSpace(e.id, e.a.toInt(), e.b.toInt()),
    rs.EventKind.error => NodeError(e.text),
    rs.EventKind.other => null,
  };
}
