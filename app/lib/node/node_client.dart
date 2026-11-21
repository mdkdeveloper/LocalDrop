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

  // -- this device and the group (Settings › General) --

  Future<ThisDevice> thisDevice();

  Future<void> setDeviceName(String name);

  Future<void> setVisible(bool visible);

  /// 0 = a random port at every start.
  Future<void> setFixedPort(int port);

  /// Checks [current], switches to [newPassword] (10+ characters). Throws with a
  /// readable message (wrong current password, too short). Returns the new group info.
  Future<GroupInfo> changePassword(String current, String newPassword);

  /// Leaves the group; the node can't be used afterwards (join again).
  Future<void> leaveGroup({
    required bool keepQueue,
    required bool forgetDevices,
  });

  // -- known devices --

  /// `null` = like the app default.
  Future<void> setDeviceAccept(String device, AcceptPolicy? policy);

  /// Forgets a device and its pinned key; its queued jobs are cancelled.
  Future<void> forgetDevice(String device);

  Future<void> dispose();
}
