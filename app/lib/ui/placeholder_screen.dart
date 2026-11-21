import 'package:material_ui/material_ui.dart';

import 'widgets.dart';

/// Sections whose screens come with later milestones (docs/development-plan.md).
class PlaceholderScreen extends StatelessWidget {
  const PlaceholderScreen.folders({super.key})
    : icon = Icons.folder_shared_outlined,
      title = 'You don’t share any folders',
      body =
          'Devices can’t see your files, but they can still send to your Inbox. '
          'Sharing folders comes with M4.';

  final IconData icon;
  final String title;
  final String body;

  @override
  Widget build(BuildContext context) =>
      EmptyState(icon: icon, title: title, body: body);
}
