import 'models.dart';

/// What the UI needs from a node: [FakeNode] (scripted demo) or [RustNode]
/// (`localdrop-node` through flutter_rust_bridge), see docs/node-api.md.
abstract class NodeClient {
  /// All node events, in order. Listened to once by the app state.
  Stream<NodeEvent> get events;

  /// If this device already joined a group, opens it and returns the group and this
  /// device's name; otherwise `null` (the app shows the join screen).
  Future<({GroupInfo group, String deviceName})?> restore();

  /// Derives the group key (slow: Argon2id) and joins. Returns name + fingerprint.
  Future<GroupInfo> join({
    required String groupName,
    required String password,
    required String deviceName,
    required bool trustThisNetwork,
  });

  /// Known devices for the first load; later changes arrive as events.
  Future<List<DeviceView>> devices();

  /// Jobs for the first load; later changes arrive as events.
  Future<List<JobView>> jobs();

  /// Starts discovery and queue delivery; events follow on [events].
  Future<void> start();

  Future<void> setDiscovery(bool on);

  Future<void> jobAction(String id, JobAction action);

  Future<void> answerRequest(
    String id, {
    required bool accept,
    required bool always,
  });

  /// `null` cancels the job.
  Future<void> answerConflict(String id, ConflictMode? mode);

  /// Connected networks (trusted or not), then remembered ones.
  Future<List<NetworkView>> networks();

  /// Trusts or distrusts a connected or remembered network (key or 8-char prefix).
  Future<void> setNetworkTrust(String key, NetworkTrust trust);

  Future<void> renameNetwork(String key, String label);

  /// Forgets a remembered network; it is asked about again next time.
  Future<void> forgetNetwork(String key);

  Future<NetworkSettings> networkSettings();

  Future<void> setNetworkSettings(NetworkSettings settings);

  /// *Connect by address*: `host` is an IP or name, `port` null = the default port.
  /// Completes when the device answered; throws with a readable message otherwise.
  Future<void> connectAddress(String host, {int? port, required bool remember});

  Future<void> dispose();
}
