import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../theme/theme.dart';
import 'dialogs.dart';
import 'networks.dart';
import 'widgets.dart';

/// Settings tabs that exist so far; the rest comes with M2 (group, devices) and M6.
enum SettingsTab { group, networks }

/// Settings (wireframes `settings`, `d-settings`): a list on phones, tabs on desktop.
class SettingsScreen extends StatelessWidget {
  const SettingsScreen({super.key, this.tab = SettingsTab.group, this.onTab});

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
            SettingsTab.group => const _GroupTab(),
            SettingsTab.networks => const NetworksScreen(),
          },
        ),
      ],
    );
  }
}

IconData _icon(SettingsTab t) => switch (t) {
  SettingsTab.group => Icons.group_outlined,
  SettingsTab.networks => Icons.wifi,
};

String _title(SettingsTab t) => switch (t) {
  SettingsTab.group => 'Group',
  SettingsTab.networks => 'Networks',
};

class _PhoneList extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 24),
      children: [
        Text('Settings', style: t.headlineSmall),
        const SizedBox(height: 8),
        const _GroupCard(),
        const SizedBox(height: 12),
        GroupCard(
          children: [
            ListTile(
              key: const Key('settings-networks'),
              leading: Icon(Icons.wifi, color: c.mute),
              title: const Text('Networks'),
              subtitle: Text(
                s.searchingLabel == null
                    ? 'Not searching'
                    : 'Searching on ${s.searchingLabel}',
                style: t.bodySmall,
              ),
              trailing: Icon(Icons.chevron_right, color: c.faint),
              onTap: () => openNetworksPage(context),
            ),
          ],
        ),
        const SizedBox(height: 12),
        Text(
          'Devices, receive folders and notifications come with later milestones.',
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

class _GroupTab extends StatelessWidget {
  const _GroupTab();

  @override
  Widget build(BuildContext context) => ListView(
    padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
    children: [
      const SectionLabel('Group'),
      const _GroupCard(),
      Padding(
        padding: const EdgeInsets.fromLTRB(4, 10, 4, 0),
        child: Text(
          'Changing the password and leaving the group come with M2.',
          style: Theme.of(context).textTheme.bodySmall,
        ),
      ),
    ],
  );
}

class _GroupCard extends StatelessWidget {
  const _GroupCard();

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final g = s.group;
    return GroupCard(
      children: [
        ListTile(
          title: const Text('Group'),
          trailing: Text(g?.name ?? '—', style: t.bodyMedium),
        ),
        ListTile(
          title: const Text('Fingerprint'),
          subtitle: Text(
            'The same on every device of the group',
            style: t.bodySmall,
          ),
          trailing: Text(
            g?.fingerprint ?? '—',
            style: t.bodyMedium?.copyWith(
              fontFeatures: const [FontFeature.tabularFigures()],
            ),
          ),
        ),
        ListTile(
          title: const Text('This device'),
          trailing: Text(s.deviceName, style: t.bodyMedium),
        ),
      ],
    );
  }
}
