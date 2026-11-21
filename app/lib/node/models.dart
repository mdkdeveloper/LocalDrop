// Dart side of the node API (docs/node-api.md). Mirrors the Rust types in
// localdrop-node so the flutter_rust_bridge bindings (M2) can replace the fake
// node without touching the UI. Ids are 32-character hex strings.

enum Presence { online, found, offline }

enum Direction { outgoing, incoming }

enum AcceptPolicy { autoAccept, ask }

/// `JobState` in localdrop-store.
enum JobState {
  waiting,
  awaitingAccept,
  awaitingConflictDecision,
  lowSpace,
  inProgress,
  paused,
  sourceChanged,
  noAnswer,
  completed,
  completedWithErrors,
  rejected,
  failed,
  cancelled,
  expired;

  bool get isActive => switch (this) {
    waiting ||
    awaitingAccept ||
    awaitingConflictDecision ||
    lowSpace ||
    inProgress ||
    paused ||
    sourceChanged => true,
    _ => false,
  };

  bool get needsDecision =>
      this == awaitingAccept ||
      this == awaitingConflictDecision ||
      this == sourceChanged;
}

enum ConflictMode {
  keepExisting,
  keepBoth,
  replaceWithBackup,
  replaceConflicting,
  mirror,
}

enum JobAction {
  pause,
  resume,
  moveToTop,
  cancel,
  retryFailed,
  offerAgain,
  clear,
}

/// `Destination` in localdrop-protocol.
sealed class Destination {
  const Destination();
}

class InboxDestination extends Destination {
  const InboxDestination();
}

class ShareDestination extends Destination {
  const ShareDestination(this.shareId, this.shareName, this.path);
  final String shareId;
  final String shareName;
  final String path;
}

/// `NetworkTrust` in localdrop-node.
enum NetworkTrust { trusted, notTrusted, neverAsk }

/// `NetworkKind` in localdrop-discovery.
enum NetworkKind { wifi, ethernet }

/// A connected or remembered network (`NetworkView` in localdrop-node).
class NetworkView {
  const NetworkView({
    required this.key,
    required this.label,
    required this.kind,
    this.address,
    this.lastSeenUnix = 0,
    required this.connected,
    required this.active,
    required this.trust,
    this.weakIdentity = false,
  });

  /// Fingerprint, hex. Events may carry only its first 8 characters.
  final String key;
  final String label;
  final NetworkKind kind;

  /// This device's address while connected, e.g. `192.168.1.23/24`.
  final String? address;
  final int lastSeenUnix;
  final bool connected;

  /// Discovery runs on it now.
  final bool active;
  final NetworkTrust trust;

  /// Known only by subnet and gateway, so other networks may look the same.
  final bool weakIdentity;

  bool matches(String keyOrPrefix) =>
      keyOrPrefix.length >= 4 && key.startsWith(keyOrPrefix);

  NetworkView copyWith({
    String? label,
    bool? connected,
    bool? active,
    NetworkTrust? trust,
    String? address,
  }) => NetworkView(
    key: key,
    label: label ?? this.label,
    kind: kind,
    address: address ?? this.address,
    lastSeenUnix: lastSeenUnix,
    connected: connected ?? this.connected,
    active: active ?? this.active,
    trust: trust ?? this.trust,
    weakIdentity: weakIdentity,
  );
}

/// *Settings › Networks* (`NetworkSettings` in localdrop-node).
class NetworkSettings {
  const NetworkSettings({
    this.discoverAtLaunch = true,
    this.askOnNewNetwork = true,
    this.trustEveryNetwork = false,
    this.onlyNetwork,
  });

  final bool discoverAtLaunch;

  /// `OnNewNetwork::AskToTrust`; false = stay silent.
  final bool askOnNewNetwork;
  final bool trustEveryNetwork;

  /// Network menu: `null` = every trusted network, else only this key.
  final String? onlyNetwork;

  NetworkSettings copyWith({
    bool? discoverAtLaunch,
    bool? askOnNewNetwork,
    bool? trustEveryNetwork,
    String? Function()? onlyNetwork,
  }) => NetworkSettings(
    discoverAtLaunch: discoverAtLaunch ?? this.discoverAtLaunch,
    askOnNewNetwork: askOnNewNetwork ?? this.askOnNewNetwork,
    trustEveryNetwork: trustEveryNetwork ?? this.trustEveryNetwork,
    onlyNetwork: onlyNetwork != null ? onlyNetwork() : this.onlyNetwork,
  );
}

class GroupInfo {
  const GroupInfo(this.name, this.fingerprint);
  final String name;
  final String fingerprint;
}

class DeviceView {
  const DeviceView({
    required this.id,
    required this.name,
    this.presence = Presence.offline,
    this.lastSeenUnix = 0,
    this.acceptOverride,
    this.queuedJobs = 0,
  });

  final String id;
  final String name;
  final Presence presence;
  final int lastSeenUnix;
  final AcceptPolicy? acceptOverride;
  final int queuedJobs;

  DeviceView copyWith({
    String? name,
    Presence? presence,
    int? lastSeenUnix,
    int? queuedJobs,
  }) => DeviceView(
    id: id,
    name: name ?? this.name,
    presence: presence ?? this.presence,
    lastSeenUnix: lastSeenUnix ?? this.lastSeenUnix,
    acceptOverride: acceptOverride,
    queuedJobs: queuedJobs ?? this.queuedJobs,
  );
}

class JobView {
  const JobView({
    required this.id,
    required this.peer,
    required this.direction,
    required this.name,
    this.destination = const InboxDestination(),
    this.deferred = false,
    required this.filesTotal,
    required this.bytesTotal,
    this.bytesDone = 0,
    this.speedBps = 0,
    this.state = JobState.waiting,
    required this.createdUnix,
  });

  final String id;
  final String peer;
  final Direction direction;
  final String name;
  final Destination destination;
  final bool deferred;
  final int filesTotal;
  final int bytesTotal;
  final int bytesDone;
  final int speedBps;
  final JobState state;
  final int createdUnix;

  double get fraction =>
      bytesTotal == 0 ? 0 : (bytesDone / bytesTotal).clamp(0, 1);

  JobView copyWith({int? bytesDone, int? speedBps, JobState? state}) => JobView(
    id: id,
    peer: peer,
    direction: direction,
    name: name,
    destination: destination,
    deferred: deferred,
    filesTotal: filesTotal,
    bytesTotal: bytesTotal,
    bytesDone: bytesDone ?? this.bytesDone,
    speedBps: speedBps ?? this.speedBps,
    state: state ?? this.state,
    createdUnix: createdUnix,
  );
}

/// `NodeEvent` in localdrop-node. Unknown future variants must be ignored.
sealed class NodeEvent {
  const NodeEvent();
}

class DiscoveryChanged extends NodeEvent {
  const DiscoveryChanged(this.on, this.activeNetworks);
  final bool on;
  final int activeNetworks;
}

class UntrustedNetwork extends NodeEvent {
  const UntrustedNetwork(this.key, this.label);
  final String key;
  final String label;
}

class PeerFound extends NodeEvent {
  const PeerFound(this.device);
  final String device;
}

class PeerOnline extends NodeEvent {
  const PeerOnline(this.device, this.name);
  final String device;
  final String name;
}

class PeerOffline extends NodeEvent {
  const PeerOffline(this.device);
  final String device;
}

class KeyMismatch extends NodeEvent {
  const KeyMismatch(this.device, this.name);
  final String device;
  final String name;
}

class JobAdded extends NodeEvent {
  const JobAdded(this.job);
  final JobView job;
}

class JobStateChanged extends NodeEvent {
  const JobStateChanged(this.id, this.state);
  final String id;
  final JobState state;
}

class JobProgress extends NodeEvent {
  const JobProgress(this.id, this.bytesDone, this.bytesTotal, this.speedBps);
  final String id;
  final int bytesDone;
  final int bytesTotal;
  final int speedBps;
}

class JobRemoved extends NodeEvent {
  const JobRemoved(this.id);
  final String id;
}

class IncomingRequest extends NodeEvent {
  const IncomingRequest(
    this.id,
    this.from,
    this.name,
    this.files,
    this.bytes, [
    this.destination = const InboxDestination(),
  ]);
  final String id;
  final String from;
  final String name;
  final int files;
  final int bytes;
  final Destination destination;
}

class ConflictAsked extends NodeEvent {
  const ConflictAsked(this.id, this.conflicts, this.allowedModes);
  final String id;
  final int conflicts;
  final List<ConflictMode> allowedModes;
}

class DecisionMade extends NodeEvent {
  const DecisionMade(this.id);
  final String id;
}

class LowSpace extends NodeEvent {
  const LowSpace(this.id, this.free, this.needed);
  final String id;
  final int free;
  final int needed;
}

class NodeError extends NodeEvent {
  const NodeError(this.message);
  final String message;
}
