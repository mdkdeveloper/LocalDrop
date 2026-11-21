import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../theme/theme.dart';
import 'dialogs.dart';
import 'group_screens.dart';
import 'known_devices.dart';
import 'networks.dart';
import 'widgets.dart';

/// Settings tabs that exist so far; receive folders, notifications and more come
/// with M3 and M6.
enum SettingsTab { general, devices, networks }

/// Settings (wireframes `settings`, `d-settings`): a list on phones, tabs on desktop.
class SettingsScreen extends StatelessWidget {
  const SettingsScreen({super.key, this.tab = SettingsTab.general, this.onTab});

  final SettingsTab tab;

  /// Desktop: switch tabs in place. Phones push the tab as its own page instead.
  final ValueChanged<SettingsTab>? onTab;

  @override
  Widget build(BuildContext context) {
    final wide = isWide(context);
    final c = context.ld;
    if (!wide) return _PhoneList();
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          width: 220,
          decoration: BoxDecoration(
            border: Border(right: BorderSide(color: c.line)),
          ),
          padding: const EdgeInsets.fromLTRB(12, 16, 12, 16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (final t in SettingsTab.values)
                ListTile(
                  key: Key('tab-${t.name}'),
                  selected: t == tab,
                  selectedTileColor: c.surface2,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(10),
                  ),
                  leading: Icon(_icon(t)),
                  title: Text(_title(t)),
                  onTap: () => onTab?.call(t),
                ),
            ],
          ),
        ),
        Expanded(
          child: switch (tab) {
            SettingsTab.general => const _GeneralTab(),
            SettingsTab.devices => const KnownDevicesList(),
            SettingsTab.networks => const NetworksScreen(),
          },
        ),
      ],
    );
  }
}

IconData _icon(SettingsTab t) => switch (t) {
  SettingsTab.general => Icons.tune,
  SettingsTab.devices => Icons.devices_outlined,
  SettingsTab.networks => Icons.wifi,
};

String _title(SettingsTab t) => switch (t) {
  SettingsTab.general => 'General',
  SettingsTab.devices => 'Devices',
  SettingsTab.networks => 'Networks',
};

class _PhoneList extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    ListTile tile(
      String key,
      IconData icon,
      String title,
      String sub,
      VoidCallback onTap,
    ) => ListTile(
      key: Key(key),
      leading: Icon(icon, color: c.mute),
      title: Text(title),
      subtitle: Text(sub, style: t.bodySmall),
      trailing: Icon(Icons.chevron_right, color: c.faint),
      onTap: onTap,
    );
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 24),
      children: [
        Text('Settings', style: t.headlineSmall),
        const SizedBox(height: 8),
        GroupCard(
          children: [
            tile(
              'settings-group',
              Icons.group_outlined,
              'Group',
              '${s.group?.name} · ${s.group?.fingerprint}',
              () => Navigator.of(context)
                  .push(MaterialPageRoute(builder: (_) => const GroupPage())),
            ),
            tile(
              'settings-devices',
              Icons.devices_outlined,
              'Known devices',
              '${s.devices.length} in group ${s.group?.name}',
              () => openKnownDevicesPage(context),
            ),
            tile(
              'settings-networks',
              Icons.wifi,
              'Networks',
              s.searchingLabel == null
                  ? 'Not searching'
                  : 'Searching on ${s.searchingLabel}',
              () => openNetworksPage(context),
            ),
          ],
        ),
        const SizedBox(height: 12),
        Text(
          'Receive folders and notifications come with later milestones.',
          style: t.bodySmall,
        ),
      ],
    );
  }
}

/// Phones: Settings › Networks as its own page (`networks`).
void openNetworksPage(BuildContext context) => Navigator.of(context).push(
  MaterialPageRoute(
    builder: (context) => Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('Networks'),
            Text(
              'Discovery runs only on trusted networks',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
      ),
      body: const NetworksScreen(),
    ),
  ),
);

class _GeneralTab extends StatelessWidget {
  const _GeneralTab();

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      final group = [const SectionLabel('Group'), const GroupSection()];
      final device = [
        const SectionLabel('This device'),
        const ThisDeviceSection(),
      ];
      return ListView(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
        children: box.maxWidth >= 860
            ? [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: group,
                      ),
                    ),
                    const SizedBox(width: 24),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: device,
                      ),
                    ),
                  ],
                ),
              ]
            : [...group, ...device],
      );
    },
  );
}
