import 'package:material_ui/material_ui.dart';

import '../app.dart';
import 'widgets.dart';

/// Sections whose screens come with later milestones (docs/development-plan.md).
class PlaceholderScreen extends StatelessWidget {
  const PlaceholderScreen.folders({super.key})
    : icon = Icons.folder_shared_outlined,
      title = 'You don’t share any folders',
      body =
          'Devices can’t see your files, but they can still send to your Inbox. '
          'Sharing folders comes with M4.';

  const PlaceholderScreen.settings({super.key})
    : icon = Icons.settings_outlined,
      title = 'Settings',
      body =
          'Group, devices, networks and receive folders. The full settings come with M6.';

  final IconData icon;
  final String title;
  final String body;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final group = s.group;
    return EmptyState(
      icon: icon,
      title: title,
      body: icon == Icons.settings_outlined && group != null
          ? '$body\n\nGroup “${group.name}” · fingerprint ${group.fingerprint} · '
                'this device: ${s.deviceName}'
          : body,
    );
  }
}
