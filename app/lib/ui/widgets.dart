import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../node/models.dart';
import '../theme/theme.dart';
import 'dialogs.dart';
import 'format.dart';

/// Small coloured dot: online (success) or offline (faint).
class StatusDot extends StatelessWidget {
  const StatusDot(this.presence, {super.key, this.size = 10});
  final Presence presence;
  final double size;

  @override
  Widget build(BuildContext context) {
    final c = context.ld;
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: switch (presence) {
          Presence.online => c.success,
          Presence.found => c.warning,
          Presence.offline => c.faint,
        },
        border: Border.all(color: c.surface, width: 2),
      ),
    );
  }
}

/// Device icon with its status dot.
class DeviceAvatar extends StatelessWidget {
  const DeviceAvatar(this.device, {super.key, this.size = 32});
  final DeviceView device;
  final double size;

  IconData get _icon {
    final n = device.name.toLowerCase();
    if (n.contains('pixel') || n.contains('phone') || n.contains('iphone')) {
      return Icons.smartphone_outlined;
    }
    if (n.contains('ipad') || n.contains('tablet')) {
      return Icons.tablet_mac_outlined;
    }
    if (n.contains('pc') || n.contains('desktop')) {
      return Icons.desktop_windows_outlined;
    }
    return Icons.laptop_outlined;
  }

  @override
  Widget build(BuildContext context) {
    final c = context.ld;
    return SizedBox(
      width: size + 2,
      height: size + 2,
      child: Stack(
        children: [
          Container(
            width: size,
            height: size,
            decoration: BoxDecoration(
              color: c.surface2,
              borderRadius: BorderRadius.circular(size / 3.5),
            ),
            child: Icon(
              _icon,
              size: size * 0.56,
              color: device.presence == Presence.offline ? c.faint : c.mute,
            ),
          ),
          Positioned(right: 0, bottom: 0, child: StatusDot(device.presence)),
        ],
      ),
    );
  }
}

/// Pill with a short state, coloured by meaning.
class StateChip extends StatelessWidget {
  const StateChip(this.label, {super.key, this.tone = ChipTone.neutral});
  final String label;
  final ChipTone tone;

  @override
  Widget build(BuildContext context) {
    final c = context.ld;
    final (bg, fg) = switch (tone) {
      ChipTone.neutral => (c.surface2, c.mute),
      ChipTone.accent => (c.accentSoft, c.accent),
      ChipTone.success => (c.surface2, c.success),
      ChipTone.warning => (c.surface2, c.warning),
      ChipTone.danger => (c.surface2, c.danger),
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        label,
        style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: fg),
      ),
    );
  }
}

enum ChipTone { neutral, accent, success, warning, danger }

ChipTone toneFor(JobState s) => switch (s) {
  JobState.inProgress => ChipTone.accent,
  JobState.completed => ChipTone.success,
  JobState.awaitingAccept ||
  JobState.awaitingConflictDecision ||
  JobState.lowSpace ||
  JobState.sourceChanged ||
  JobState.noAnswer => ChipTone.warning,
  JobState.failed ||
  JobState.completedWithErrors ||
  JobState.rejected => ChipTone.danger,
  _ => ChipTone.neutral,
};

/// Upper-case section label ("DEVICES", "OUTGOING").
class SectionLabel extends StatelessWidget {
  const SectionLabel(this.text, {super.key, this.trailing});
  final String text;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.fromLTRB(4, 12, 4, 6),
    child: Row(
      children: [
        Expanded(
          child: Text(
            text.toUpperCase(),
            style: Theme.of(context).textTheme.labelSmall,
          ),
        ),
        ?trailing,
      ],
    ),
  );
}

/// The discovery switch with the network it runs on (docs/networks.md).
///
/// The label opens [onOpen] (network menu on desktop, Settings › Networks on phones);
/// the switch turns discovery on and off.
class DiscoveryPill extends StatelessWidget {
  const DiscoveryPill({super.key, this.onOpen});
  final VoidCallback? onOpen;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final c = context.ld;
    final on = s.discoveryOn;
    final where = s.searchingLabel;
    final label = !on
        ? 'Discovery off'
        : where == null
        ? 'No trusted network'
        : 'Discovery · $where';
    return Container(
      decoration: BoxDecoration(
        color: c.surface,
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: c.line),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          InkWell(
            key: const Key('discovery-label'),
            borderRadius: const BorderRadius.horizontal(
              left: Radius.circular(999),
            ),
            onTap: onOpen,
            child: Padding(
              padding: const EdgeInsets.fromLTRB(12, 8, 4, 8),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Container(
                    width: 8,
                    height: 8,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: on && where != null ? c.success : c.faint,
                    ),
                  ),
                  const SizedBox(width: 8),
                  ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 200),
                    child: Text(
                      label,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w600,
                        color: on ? c.ink : c.mute,
                      ),
                    ),
                  ),
                  if (onOpen != null)
                    Icon(Icons.expand_more, size: 16, color: c.mute),
                ],
              ),
            ),
          ),
          Transform.scale(
            scale: 0.7,
            child: Switch(
              key: const Key('discovery-switch'),
              value: on,
              onChanged: s.setDiscovery,
              materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
          ),
        ],
      ),
    );
  }
}

/// One job row: icon, name, subtitle, progress and actions.
class JobTile extends StatelessWidget {
  const JobTile(this.job, {super.key});
  final JobView job;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    final peer = s.nameOf(job.peer);
    final running = job.state == JobState.inProgress;
    final showBar =
        running ||
        job.state == JobState.paused ||
        job.state == JobState.lowSpace;
    final sub = [
      if (job.filesTotal > 1) '${job.filesTotal} files',
      formatBytes(job.bytesTotal),
      job.destination is InboxDestination
          ? '→ Inbox'
          : '→ ${(job.destination as ShareDestination).shareName}',
    ].join(' · ');
    final meta = running
        ? '${formatBytes(job.bytesDone)} of ${formatBytes(job.bytesTotal)} · '
              '${formatSpeed(job.speedBps)} · ${formatEta(job.bytesTotal - job.bytesDone, job.speedBps)}'
        : stateLabel(job, peerName: peer);
    final decision = s.decisions.where((d) => d.jobId == job.id).firstOrNull;

    return InkWell(
      onTap: () => showJobMenu(context, job),
      child: Padding(
        padding: const EdgeInsets.all(14),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Container(
                  width: 36,
                  height: 36,
                  decoration: BoxDecoration(
                    color: c.accentSoft,
                    borderRadius: BorderRadius.circular(9),
                  ),
                  child: Icon(
                    job.filesTotal > 1
                        ? Icons.folder_outlined
                        : Icons.description_outlined,
                    size: 20,
                    color: c.accent,
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        job.name,
                        style: t.titleMedium,
                        overflow: TextOverflow.ellipsis,
                      ),
                      const SizedBox(height: 2),
                      Text(
                        sub,
                        style: t.bodySmall,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
                if (decision != null)
                  FilledButton(
                    onPressed: () => showDecision(context, decision),
                    style: FilledButton.styleFrom(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 14,
                        vertical: 8,
                      ),
                    ),
                    child: const Text('Resolve'),
                  )
                else
                  IconButton(
                    tooltip: 'More',
                    icon: Icon(Icons.more_vert, color: c.mute),
                    onPressed: () => showJobMenu(context, job),
                  ),
              ],
            ),
            if (showBar) ...[
              const SizedBox(height: 10),
              ClipRRect(
                borderRadius: BorderRadius.circular(3),
                child: LinearProgressIndicator(
                  value: job.fraction,
                  minHeight: 6,
                  color: job.state == JobState.lowSpace ? c.warning : null,
                ),
              ),
            ],
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(
                  child: Text(
                    meta,
                    style: t.bodySmall,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                if (!running) ...[
                  const SizedBox(width: 8),
                  StateChip(shortState(job.state), tone: toneFor(job.state)),
                ],
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// A rounded card holding rows separated by dividers.
class GroupCard extends StatelessWidget {
  const GroupCard({super.key, required this.children, this.header});
  final List<Widget> children;
  final Widget? header;

  @override
  Widget build(BuildContext context) {
    final rows = <Widget>[?header];
    for (var i = 0; i < children.length; i++) {
      if (i > 0 || header != null) {
        rows.add(const Divider(indent: 14, endIndent: 14));
      }
      rows.add(children[i]);
    }
    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: rows,
      ),
    );
  }
}

/// Explains what an empty list is for and offers the next step (docs/platforms.md).
class EmptyState extends StatelessWidget {
  const EmptyState({
    super.key,
    required this.icon,
    required this.title,
    required this.body,
    this.action,
  });
  final IconData icon;
  final String title;
  final String body;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 420),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 56,
                height: 56,
                decoration: BoxDecoration(
                  color: c.accentSoft,
                  borderRadius: BorderRadius.circular(16),
                ),
                child: Icon(icon, color: c.accent, size: 28),
              ),
              const SizedBox(height: 16),
              Text(title, style: t.titleLarge, textAlign: TextAlign.center),
              const SizedBox(height: 8),
              Text(
                body,
                style: t.bodyMedium?.copyWith(color: c.mute),
                textAlign: TextAlign.center,
              ),
              if (action != null) ...[const SizedBox(height: 20), action!],
            ],
          ),
        ),
      ),
    );
  }
}

/// Asks for one line of text; `null` if cancelled. The dialog owns its controller, so
/// nothing is disposed while the closing animation still shows the field.
Future<String?> askText(
  BuildContext context, {
  required String title,
  required String label,
  required String initial,
  required String action,
  String? hint,
  bool number = false,
}) => showDialog<String>(
  context: context,
  builder: (_) => _AskTextDialog(
    title: title,
    label: label,
    initial: initial,
    action: action,
    hint: hint,
    number: number,
  ),
);

class _AskTextDialog extends StatefulWidget {
  const _AskTextDialog({
    required this.title,
    required this.label,
    required this.initial,
    required this.action,
    this.hint,
    this.number = false,
  });

  final String title;
  final String label;
  final String initial;
  final String action;
  final String? hint;
  final bool number;

  @override
  State<_AskTextDialog> createState() => _AskTextDialogState();
}

class _AskTextDialogState extends State<_AskTextDialog> {
  late final _ctl = TextEditingController(text: widget.initial);

  @override
  void dispose() {
    _ctl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
    title: Text(widget.title),
    content: TextField(
      key: const Key('ask-text'),
      controller: _ctl,
      autofocus: true,
      keyboardType: widget.number ? TextInputType.number : null,
      decoration: InputDecoration(
        labelText: widget.label,
        helperText: widget.hint,
        helperMaxLines: 3,
      ),
      onSubmitted: (v) => Navigator.pop(context, v),
    ),
    actions: [
      TextButton(
        onPressed: () => Navigator.pop(context),
        child: const Text('Cancel'),
      ),
      FilledButton(
        onPressed: () => Navigator.pop(context, _ctl.text),
        child: Text(widget.action),
      ),
    ],
  );
}
