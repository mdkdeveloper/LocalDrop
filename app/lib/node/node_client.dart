import 'models.dart';

/// What the UI needs from a node. Implemented by [FakeNode] now and by the
/// flutter_rust_bridge bindings to `localdrop-node` from M2 (docs/node-api.md).
abstract class NodeClient {
  /// All node events, in order. Listened to once by the app state.
  Stream<NodeEvent> get events;

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

  Future<void> trustNetwork(
    String key, {
    required bool trust,
    bool neverAsk = false,
  });

  Future<void> dispose();
}
