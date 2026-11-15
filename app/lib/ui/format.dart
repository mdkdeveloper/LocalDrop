import '../node/models.dart';

/// "3.2 GB", "48 MB", "4 KB" (decimal units, like the design).
String formatBytes(int bytes) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  var v = bytes.toDouble();
  var i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  final digits = i == 0 || v >= 100 ? 0 : 1;
  return '${v.toStringAsFixed(digits)} ${units[i]}';
}

String formatSpeed(int bps) => '${formatBytes(bps)}/s';

/// "3 min left", "40 s left".
String formatEta(int remainingBytes, int bps) {
  if (bps <= 0) return '';
  final s = remainingBytes ~/ bps;
  if (s < 60) return '$s s left';
  if (s < 3600) return '${(s / 60).ceil()} min left';
  return '${(s / 3600).toStringAsFixed(1)} h left';
}

/// "2 h ago", "Yesterday", "Just now".
String formatAgo(int unixSeconds, {DateTime? now}) {
  if (unixSeconds == 0) return 'Never';
  final diff = (now ?? DateTime.now()).difference(
    DateTime.fromMillisecondsSinceEpoch(unixSeconds * 1000),
  );
  if (diff.inMinutes < 1) return 'Just now';
  if (diff.inHours < 1) return '${diff.inMinutes} min ago';
  if (diff.inDays < 1) return '${diff.inHours} h ago';
  if (diff.inDays == 1) return 'Yesterday';
  return '${diff.inDays} days ago';
}

/// Queue labels from docs/transfers.md#queue-ui.
String stateLabel(JobView j, {String? peerName}) => switch (j.state) {
  JobState.waiting when j.deferred => 'Waiting for ${peerName ?? 'device'}',
  JobState.waiting => 'Next in queue',
  JobState.awaitingAccept ||
  JobState.awaitingConflictDecision => 'Waiting for decision',
  JobState.lowSpace => 'Paused · receiver low on space',
  JobState.inProgress =>
    j.direction == Direction.outgoing ? 'Sending' : 'Receiving',
  JobState.paused => 'Paused',
  JobState.sourceChanged => 'Files changed',
  JobState.noAnswer => 'No answer',
  JobState.completed => 'Completed',
  JobState.completedWithErrors => 'Completed with errors',
  JobState.rejected => 'Declined',
  JobState.failed => 'Failed',
  JobState.cancelled => 'Cancelled',
  JobState.expired => 'Expired',
};

/// One or two words for the state chip.
String shortState(JobState s) => switch (s) {
  JobState.waiting => 'Waiting',
  JobState.awaitingAccept ||
  JobState.awaitingConflictDecision => 'Needs decision',
  JobState.lowSpace => 'Low space',
  JobState.inProgress => 'Active',
  JobState.paused => 'Paused',
  JobState.sourceChanged => 'Changed',
  JobState.noAnswer => 'No answer',
  JobState.completed => 'Done',
  JobState.completedWithErrors => 'With errors',
  JobState.rejected => 'Declined',
  JobState.failed => 'Failed',
  JobState.cancelled => 'Cancelled',
  JobState.expired => 'Expired',
};

String modeLabel(ConflictMode m) => switch (m) {
  ConflictMode.keepBoth => 'Keep both',
  ConflictMode.keepExisting => 'Upload only new files',
  ConflictMode.replaceWithBackup => 'Replace, keep a backup',
  ConflictMode.replaceConflicting => 'Replace conflicting files',
  ConflictMode.mirror => 'Replace the whole folder',
};

String modeHint(ConflictMode m) => switch (m) {
  ConflictMode.keepBoth => 'New copies are saved as “name (1)”.',
  ConflictMode.keepExisting => 'Existing files stay as they are.',
  ConflictMode.replaceWithBackup => 'Old versions go to Settings › Backups.',
  ConflictMode.replaceConflicting => 'Old versions are moved to the Trash.',
  ConflictMode.mirror => 'Also removes files that aren’t in this upload.',
};

/// Actions of a job's ⋮ menu for its state (docs/transfers.md#job-actions).
List<JobAction> actionsFor(JobView j) => switch (j.state) {
  JobState.inProgress => [JobAction.pause, JobAction.cancel],
  JobState.waiting => [JobAction.pause, JobAction.moveToTop, JobAction.cancel],
  JobState.paused => [JobAction.resume, JobAction.moveToTop, JobAction.cancel],
  JobState.awaitingAccept ||
  JobState.awaitingConflictDecision ||
  JobState.lowSpace ||
  JobState.sourceChanged => [JobAction.cancel],
  JobState.noAnswer => [JobAction.offerAgain, JobAction.clear],
  JobState.completedWithErrors ||
  JobState.failed => [JobAction.retryFailed, JobAction.clear],
  _ => [JobAction.clear],
};

String actionLabel(JobAction a, JobView j) => switch (a) {
  JobAction.pause => 'Pause',
  JobAction.resume => 'Resume',
  JobAction.moveToTop => 'Move to top',
  JobAction.cancel =>
    j.state == JobState.waiting ? 'Remove from queue' : 'Cancel',
  JobAction.retryFailed => 'Retry failed',
  JobAction.offerAgain => 'Offer again now',
  JobAction.clear => 'Clear',
};

bool isDestructive(JobAction a) => a == JobAction.cancel;
