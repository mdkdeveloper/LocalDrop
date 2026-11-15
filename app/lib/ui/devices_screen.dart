import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../node/models.dart';
import '../theme/theme.dart';
import 'format.dart';
import 'widgets.dart';

/// Phone "Files" tab: the group's devices (wireframes `files-root`, `looking`).
class DevicesScreen extends StatelessWidget {
  const DevicesScreen({super.key, required this.onOpen});
  final ValueChanged<String> onOpen;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    final online = s.onlineDevices;
    final offline = s.offlineDevices;

    ListTile tile(DeviceView d) => ListTile(
      leading: DeviceAvatar(d),
      title: Text(d.name, style: t.titleMedium),
      subtitle: Text(
        d.presence == Presence.offline
            ? [
                formatAgo(d.lastSeenUnix),
                if (d.queuedJobs > 0) '${d.queuedJobs} queued',
              ].join(' · ')
            : d.presence == Presence.found
            ? 'Connecting…'
            : 'Online',
        style: t.bodySmall,
      ),
      trailing: Icon(Icons.chevron_right, color: c.faint),
      onTap: () => onOpen(d.id),
    );

    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 24),
      children: [
        Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('Devices', style: t.headlineSmall),
                  Text(
                    '${s.group?.name} · ${s.group?.fingerprint}',
                    style: t.bodySmall,
                  ),
                ],
              ),
            ),
            const DiscoveryPill(),
          ],
        ),
        const SizedBox(height: 8),
        const SectionLabel('Online'),
        if (online.isEmpty)
          Card(
            child: ListTile(
              leading: SizedBox(
                width: 24,
                height: 24,
                child: CircularProgressIndicator(strokeWidth: 2, color: c.mute),
              ),
              title: Text(
                s.discoveryOn
                    ? 'Looking for your devices…'
                    : 'Discovery is off',
              ),
              subtitle: Text(
                'Open LocalDrop on your other devices with the same group name and password.',
                style: t.bodySmall,
              ),
            ),
          )
        else
          GroupCard(children: online.map(tile).toList()),
        if (offline.isNotEmpty) ...[
          const SectionLabel('Known · offline'),
          GroupCard(children: offline.map(tile).toList()),
        ],
      ],
    );
  }
}
