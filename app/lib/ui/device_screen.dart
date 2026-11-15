import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../node/models.dart';
import '../theme/theme.dart';
import 'format.dart';
import 'widgets.dart';

/// One device: what's queued for it and (from M4) its shared folders
/// (wireframes `d-remote`, `d-offline`, `files-offline`).
class DeviceScreen extends StatelessWidget {
  const DeviceScreen(this.id, {super.key});
  final String id;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final d = s.devices[id];
    if (d == null) return const SizedBox.shrink();
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    final queued = s
        .activeJobs(Direction.outgoing)
        .where((j) => j.peer == id)
        .toList();
    final online = d.presence == Presence.online;

    return ListView(
      padding: const EdgeInsets.fromLTRB(24, 24, 24, 24),
      children: [
        Row(
          children: [
            DeviceAvatar(d, size: 48),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(d.name, style: t.headlineSmall),
                  Text(
                    online
                        ? 'Online'
                        : 'Offline · last seen ${formatAgo(d.lastSeenUnix).toLowerCase()}',
                    style: t.bodySmall?.copyWith(fontSize: 13),
                  ),
                ],
              ),
            ),
            Tooltip(
              message: 'Picking files needs the Rust bridge (M2)',
              child: FilledButton.icon(
                onPressed: null,
                icon: const Icon(Icons.upload_file),
                label: Text(online ? 'Send files…' : 'Queue files…'),
              ),
            ),
          ],
        ),
        if (!online) ...[
          const SizedBox(height: 16),
          Container(
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              color: c.accentSoft,
              borderRadius: BorderRadius.circular(LdSize.cardRadius),
            ),
            child: Row(
              children: [
                Icon(Icons.schedule, color: c.accent),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    'Files you send now wait here and go out automatically when ${d.name} is back '
                    'on a trusted network with LocalDrop running.',
                    style: t.bodyMedium,
                  ),
                ),
              ],
            ),
          ),
        ],
        const SectionLabel('Queued for this device'),
        if (queued.isEmpty)
          Text('Nothing queued.', style: t.bodySmall)
        else
          GroupCard(children: [for (final j in queued) JobTile(j)]),
        const SectionLabel('Shared folders'),
        Card(
          child: ListTile(
            leading: Icon(Icons.inbox_outlined, color: c.accent),
            title: const Text('Inbox'),
            subtitle: Text(
              'Browsing shared folders comes with M4.',
              style: t.bodySmall,
            ),
          ),
        ),
      ],
    );
  }
}
