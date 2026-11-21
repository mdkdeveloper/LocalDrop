import 'dart:async';

import 'package:flutter/foundation.dart';

import '../node/models.dart';
import '../node/node_client.dart';

/// A dialog the node asked for. Shown once; closed by the answer or by [DecisionMade].
sealed class PendingDecision {
  const PendingDecision();
  String get jobId;
}

class RequestDecision extends PendingDecision {
  const RequestDecision(this.request);
  final IncomingRequest request;
  @override
  String get jobId => request.id;
}

class ConflictDecision extends PendingDecision {
  const ConflictDecision(this.conflict);
  final ConflictAsked conflict;
  @override
  String get jobId => conflict.id;
}

/// Everything the screens show, rebuilt from [NodeEvent]s.
class AppState extends ChangeNotifier {
  AppState(this.node);

  final NodeClient node;
  StreamSubscription<NodeEvent>? _sub;

  GroupInfo? group;
  String deviceName = '';
  bool discoveryOn = false;
  int activeNetworks = 0;
  final devices = <String, DeviceView>{};
  final jobs = <String, JobView>{};
  final decisions = <PendingDecision>[];
  final lowSpace = <String, LowSpace>{};
  UntrustedNetwork? untrustedNetwork;
  var networks = <NetworkView>[];
  var networkSettings = const NetworkSettings();
  final keyMismatches = <KeyMismatch>[];
  String? lastError;

  bool get joined => group != null;

  /// Joins the group and starts listening to the node.
  Future<void> join({
    required String groupName,
    required String password,
    required String deviceName,
    required bool trustThisNetwork,
  }) async {
    final info = await node.join(
      groupName: groupName,
      password: password,
      deviceName: deviceName,
      trustThisNetwork: trustThisNetwork,
    );
    this.deviceName = deviceName;
    await _open(info);
  }

  /// Opens the group this device already joined, if any. Returns whether it did.
  Future<bool> restore() async {
    final r = await node.restore();
    if (r == null) return false;
    deviceName = r.deviceName;
    await _open(r.group);
    return true;
  }

  Future<void> _open(GroupInfo info) async {
    for (final d in await node.devices()) {
      devices[d.id] = d;
    }
    for (final j in await node.jobs()) {
      jobs[j.id] = j;
    }
    networks = await node.networks();
    networkSettings = await node.networkSettings();
    _sub = node.events.listen(apply);
    group = info;
    await node.start();
    notifyListeners();
  }

  /// Applies one event. Public so tests can drive the state directly.
  void apply(NodeEvent e) {
    switch (e) {
      case DiscoveryChanged(:final on, :final activeNetworks):
        discoveryOn = on;
        this.activeNetworks = activeNetworks;
        _refreshNetworks();
      case UntrustedNetwork():
        untrustedNetwork = e;
        _refreshNetworks();
      case PeerFound(:final device):
        _device(
          device,
          (d) => d.presence == Presence.online
              ? d
              : d.copyWith(presence: Presence.found),
        );
      case PeerOnline(:final device, :final name):
        devices[device] =
            (devices[device] ?? DeviceView(id: device, name: name)).copyWith(
              name: name,
              presence: Presence.online,
            );
      case PeerOffline(:final device):
        _device(
          device,
          (d) => d.copyWith(
            presence: Presence.offline,
            lastSeenUnix: DateTime.now().millisecondsSinceEpoch ~/ 1000,
          ),
        );
      case KeyMismatch():
        keyMismatches.add(e);
      case JobAdded(:final job):
        jobs[job.id] = job;
      case JobStateChanged(:final id, :final state):
        final j = jobs[id];
        if (j != null) jobs[id] = j.copyWith(state: state);
        if (state != JobState.lowSpace) lowSpace.remove(id);
      case JobProgress(:final id, :final bytesDone, :final speedBps):
        final j = jobs[id];
        if (j != null) {
          jobs[id] = j.copyWith(bytesDone: bytesDone, speedBps: speedBps);
        }
      case JobRemoved(:final id):
        jobs.remove(id);
      case IncomingRequest():
        decisions.add(RequestDecision(e));
      case ConflictAsked():
        decisions.add(ConflictDecision(e));
      case DecisionMade(:final id):
        decisions.removeWhere((d) => d.jobId == id);
      case LowSpace(:final id):
        lowSpace[id] = e;
      case NodeError(:final message):
        lastError = message;
    }
    _recountQueues();
    notifyListeners();
  }

  // -- commands ----------------------------------------------------------------------

  Future<void> setDiscovery(bool on) => _run(() => node.setDiscovery(on));

  Future<void> jobAction(String id, JobAction action) =>
      _run(() => node.jobAction(id, action));

  Future<void> answerRequest(
    String id, {
    required bool accept,
    bool always = false,
  }) {
    decisions.removeWhere((d) => d.jobId == id);
    notifyListeners();
    return _run(() => node.answerRequest(id, accept: accept, always: always));
  }

  Future<void> answerConflict(String id, ConflictMode? mode) {
    decisions.removeWhere((d) => d.jobId == id);
    notifyListeners();
    return _run(() => node.answerConflict(id, mode));
  }

  /// Answers "Trust “Office-5G”?": Trust this network / Not now / Never ask.
  Future<void> answerNetwork({required bool trust, bool neverAsk = false}) {
    final net = untrustedNetwork;
    untrustedNetwork = null;
    notifyListeners();
    if (net == null) return Future.value();
    return setNetworkTrust(
      net.key,
      trust
          ? NetworkTrust.trusted
          : neverAsk
          ? NetworkTrust.neverAsk
          : NetworkTrust.notTrusted,
    );
  }

  Future<void> setNetworkTrust(String key, NetworkTrust trust) {
    if (untrustedNetwork != null &&
        networkFor(untrustedNetwork!.key)?.matches(key) == true) {
      untrustedNetwork = null; // answered from the menu or settings instead
    }
    return _run(() async {
      await node.setNetworkTrust(key, trust);
      await _refreshNetworks();
    });
  }

  Future<void> renameNetwork(String key, String label) => _run(() async {
    await node.renameNetwork(key, label);
    await _refreshNetworks();
  });

  Future<void> forgetNetwork(String key) => _run(() async {
    await node.forgetNetwork(key);
    await _refreshNetworks();
  });

  Future<void> setNetworkSettings(NetworkSettings settings) {
    networkSettings = settings;
    notifyListeners();
    return _run(() async {
      await node.setNetworkSettings(settings);
      networkSettings = await node.networkSettings();
      await _refreshNetworks();
    });
  }

  /// Throws with the node's message, so the form can show it next to the field.
  Future<void> connectAddress(
    String host, {
    int? port,
    required bool remember,
  }) => node.connectAddress(host, port: port, remember: remember);

  void clearError() {
    lastError = null;
    notifyListeners();
  }

  // -- views -------------------------------------------------------------------------

  List<DeviceView> get onlineDevices =>
      devices.values.where((d) => d.presence != Presence.offline).toList()
        ..sort((a, b) => a.name.compareTo(b.name));

  List<DeviceView> get offlineDevices =>
      devices.values.where((d) => d.presence == Presence.offline).toList()
        ..sort((a, b) => b.lastSeenUnix.compareTo(a.lastSeenUnix));

  List<JobView> activeJobs(Direction direction) =>
      jobs.values
          .where((j) => j.direction == direction && j.state.isActive)
          .toList()
        ..sort((a, b) => a.createdUnix.compareTo(b.createdUnix));

  List<JobView> get finishedJobs =>
      jobs.values.where((j) => !j.state.isActive).toList()
        ..sort((a, b) => b.createdUnix.compareTo(a.createdUnix));

  int get runningCount =>
      jobs.values.where((j) => j.state == JobState.inProgress).length;

  int get waitingCount => jobs.values
      .where((j) => j.state.isActive && j.state != JobState.inProgress)
      .length;

  String nameOf(String id) => devices[id]?.name ?? 'Unknown device';

  List<NetworkView> get connectedNetworks =>
      networks.where((n) => n.connected).toList();

  List<NetworkView> get rememberedNetworks => networks
      .where((n) => n.trust != NetworkTrust.notTrusted || !n.connected)
      .toList();

  /// Networks discovery runs on right now (empty while the switch is off).
  List<NetworkView> get searchingOn =>
      discoveryOn ? networks.where((n) => n.active).toList() : const [];

  /// "Home", "Home + Office LAN"; null when discovery runs nowhere.
  String? get searchingLabel {
    final on = searchingOn;
    return on.isEmpty ? null : on.map((n) => n.label).join(' + ');
  }

  /// No trusted network is connected, so the ask takes the main area (or a sheet)
  /// instead of a banner (`new-net`, `d-new-net`).
  bool get askProminently =>
      untrustedNetwork != null && !networks.any((n) => n.active);

  NetworkView? networkFor(String keyOrPrefix) =>
      networks.where((n) => n.matches(keyOrPrefix)).firstOrNull;

  // -- internals ---------------------------------------------------------------------

  void _device(String id, DeviceView Function(DeviceView) f) {
    final d = devices[id];
    if (d != null) devices[id] = f(d);
  }

  void _recountQueues() {
    for (final d in devices.values.toList()) {
      final n = jobs.values
          .where(
            (j) =>
                j.peer == d.id &&
                j.direction == Direction.outgoing &&
                j.state.isActive,
          )
          .length;
      if (n != d.queuedJobs) devices[d.id] = d.copyWith(queuedJobs: n);
    }
  }

  Future<void> _refreshNetworks() async {
    try {
      final list = await node.networks();
      if (_disposed) return;
      networks = list;
      notifyListeners();
    } catch (_) {
      // The next change refreshes again.
    }
  }

  Future<void> _run(Future<void> Function() f) async {
    try {
      await f();
    } catch (e) {
      lastError = e is StateError ? e.message : '$e';
      notifyListeners();
    }
  }

  bool _disposed = false;

  @override
  void dispose() {
    _disposed = true;
    _sub?.cancel();
    node.dispose();
    super.dispose();
  }
}
