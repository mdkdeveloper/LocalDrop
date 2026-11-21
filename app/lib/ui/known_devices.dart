import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../node/models.dart';
import '../theme/theme.dart';
import 'dialogs.dart';
import 'format.dart';
import 'group_screens.dart' show shortId;
import 'widgets.dart';

// Known devices (wireframes `known`, `d-devices`), device details (`device`,
// `d-device`) and a key that doesn't match (`key`, `d-key`).

String _acceptLabel(AcceptPolicy? p) => switch (p) {
  null => 'Like the app default',
  AcceptPolicy.autoAccept => 'Accepts automatically',
  AcceptPolicy.ask => 'Asks before receiving',
};

String _presence(DeviceView d) => switch (d.presence) {
  Presence.online => 'Online',
  Presence.found => 'Connecting…',
  Presence.offline => formatAgo(d.lastSeenUnix),
};

/// The list of known devices with this device on top.
class KnownDevicesList extends StatelessWidget {
  const KnownDevicesList({super.key});

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    final all = [...s.onlineDevices, ...s.offlineDevices];
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(4, 8, 4, 8),
          child: Text(
            'Devices that joined your group and were seen by this device · '
            '${all.length} known',
            style: t.bodySmall,
          ),
        ),
        GroupCard(
          children: [
            ListTile(
              leading: Icon(Icons.computer, color: c.mute),
              title: Text('${s.deviceName} (this device)'),
              subtitle: Text(
                s.me == null ? '' : shortId(s.me!.id),
                style: t.bodySmall,
              ),
            ),
            for (final d in all)
              ListTile(
                key: Key('known-${d.id}'),
                leading: DeviceAvatar(d),
                title: Text(d.name),
                subtitle: Text(
                  [
                    _presence(d),
                    _acceptLabel(d.acceptOverride),
                    if (d.queuedJobs > 0) '${d.queuedJobs} queued',
                  ].join(' · '),
                  style: t.bodySmall,
                ),
                trailing: d.queuedJobs > 0
                    ? StateChip('${d.queuedJobs}')
                    : Icon(Icons.chevron_right, color: c.faint),
                onTap: () => showDeviceDetails(context, d.id),
              ),
          ],
        ),
        const SizedBox(height: 12),
        Container(
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(12),
            border: Border.all(color: c.line),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(Icons.info_outline, color: c.mute, size: 20),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Forgetting a device doesn’t lock it out',
                      style: t.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      'Anyone with the group password can rejoin. To remove a device for '
                      'good, change the group password on your other devices.',
                      style: t.bodySmall,
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

/// Phones: Settings › Known devices as its own page (`known`).
void openKnownDevicesPage(BuildContext context) => Navigator.of(context).push(
  MaterialPageRoute(
    builder: (context) => Scaffold(
      appBar: AppBar(title: const Text('Known devices')),
      body: const KnownDevicesList(),
    ),
  ),
);

/// Dialog on desktop, a page on phones.
Future<void> showDeviceDetails(BuildContext context, String id) {
  if (isWide(context)) {
    return showDialog(
      context: context,
      builder: (_) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 520),
          child: Padding(
            padding: const EdgeInsets.all(28),
            child: DeviceDetails(id),
          ),
        ),
      ),
    );
  }
  return Navigator.of(context).push(
    MaterialPageRoute(
      builder: (context) => Scaffold(
        appBar: AppBar(title: Text(AppScope.of(context).nameOf(id))),
        body: SingleChildScrollView(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
          child: DeviceDetails(id),
        ),
      ),
    ),
  );
}

class DeviceDetails extends StatelessWidget {
  const DeviceDetails(this.id, {super.key});
  final String id;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final d = s.devices[id];
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    if (d == null) {
      return Text('This device was forgotten.', style: t.bodyMedium);
    }
    final wide = isWide(context);
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (wide)
          Row(
            children: [
              DeviceAvatar(d, size: 44),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(d.name, style: t.titleLarge),
                    Text(_presence(d), style: t.bodySmall),
                  ],
                ),
              ),
            ],
          )
        else
          Text(_presence(d), style: t.bodySmall),
        const SectionLabel('Accept files from it'),
        Card(
          child: RadioGroup<AcceptPolicy?>(
            groupValue: d.acceptOverride,
            onChanged: (v) => s.setDeviceAccept(id, v),
            child: const Column(
              children: [
                RadioListTile<AcceptPolicy?>(
                  value: null,
                  title: Text('Like the app default'),
                ),
                RadioListTile<AcceptPolicy?>(
                  value: AcceptPolicy.autoAccept,
                  title: Text('Always automatically'),
                ),
                RadioListTile<AcceptPolicy?>(
                  value: AcceptPolicy.ask,
                  title: Text('Ask me every time'),
                  subtitle: Text('Requests wait up to 10 min'),
                ),
              ],
            ),
          ),
        ),
        const SectionLabel('About'),
        GroupCard(
          children: [
            ListTile(
              dense: true,
              title: const Text('Device ID'),
              trailing: Text(shortId(d.id), style: t.bodyMedium),
            ),
            ListTile(
              dense: true,
              title: const Text('Last seen'),
              trailing: Text(
                d.presence == Presence.offline
                    ? formatAgo(d.lastSeenUnix)
                    : 'Now',
                style: t.bodyMedium,
              ),
            ),
            ListTile(
              dense: true,
              title: const Text('Send queue'),
              trailing: Text(
                d.queuedJobs == 0 ? 'Empty' : '${d.queuedJobs} queued',
                style: t.bodyMedium,
              ),
            ),
          ],
        ),
        const SizedBox(height: 16),
        Row(
          children: [
            TextButton(
              key: const Key('forget-device'),
              style: TextButton.styleFrom(foregroundColor: c.danger),
              onPressed: () => _forget(context, d),
              child: const Text('Forget device'),
            ),
            const Spacer(),
            if (wide)
              FilledButton(
                onPressed: () => Navigator.of(context).pop(),
                child: const Text('Close'),
              ),
          ],
        ),
      ],
    );
  }

  Future<void> _forget(BuildContext context, DeviceView d) async {
    final s = AppScope.of(context);
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Forget ${d.name}?'),
        content: Text(
          '${d.queuedJobs > 0 ? 'Its ${d.queuedJobs} queued jobs are cancelled. ' : ''}'
          'It shows up again as a new device if it still knows the group password; '
          'change the password to keep it out.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            key: const Key('confirm-forget'),
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Forget'),
          ),
        ],
      ),
    );
    if (ok != true) return;
    await s.forgetDevice(d.id);
    if (context.mounted) Navigator.of(context).pop();
  }
}

// ---------------------------------------------------------------------- key mismatch

/// A device presented a key that doesn't belong to it and was refused (`key`, `d-key`).
///
/// The device id is derived from the key, so a reinstalled device can't keep its id:
/// it shows up as a new device. A mismatch therefore means someone in the group claimed
/// to be this device, or a stale address now answers as another one. There is no
/// "trust the new key"; the choices are keep it blocked, or forget the old device.
Future<void> showKeyMismatch(BuildContext context, KeyMismatch m) {
  final body = KeyMismatchBody(m);
  if (isWide(context)) {
    return showDialog(
      context: context,
      builder: (_) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 540),
          child: Padding(padding: const EdgeInsets.all(28), child: body),
        ),
      ),
    );
  }
  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    builder: (_) => SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(20, 4, 20, 16),
        child: body,
      ),
    ),
  );
}

class KeyMismatchBody extends StatelessWidget {
  const KeyMismatchBody(this.m, {super.key});
  final KeyMismatch m;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    final known = s.devices.containsKey(m.device);
    void close() {
      s.dismissKeyMismatch(m.device);
      Navigator.of(context).pop();
    }

    final keep = OutlinedButton(
      key: const Key('keep-blocked'),
      onPressed: close,
      child: const Text('Keep blocked'),
    );
    final forget = TextButton(
      key: const Key('forget-mismatch'),
      style: TextButton.styleFrom(foregroundColor: c.mute),
      onPressed: () {
        s.forgetDevice(m.device);
        Navigator.of(context).pop();
      },
      child: Text('Forget ${m.name}'),
    );
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Container(
              width: 40,
              height: 40,
              decoration: BoxDecoration(
                color: c.surface2,
                borderRadius: BorderRadius.circular(12),
              ),
              child: Icon(Icons.key_off_outlined, color: c.warning),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('${m.name}’s key doesn’t match', style: t.titleLarge),
                  Text('Refused', style: t.bodySmall),
                ],
              ),
            ),
          ],
        ),
        const SizedBox(height: 14),
        Text(
          'A device in your group answered as ${m.name} (${shortId(m.device)}) but '
          'couldn’t prove it has that device’s key, so LocalDrop didn’t connect. A '
          'reinstalled device doesn’t cause this: it shows up as a new device. Someone '
          'may be pretending to be ${m.name}.',
          style: t.bodyMedium?.copyWith(color: c.mute),
        ),
        const SizedBox(height: 8),
        Text(
          'If you don’t trust every device that knows the password, change the group '
          'password.',
          style: t.bodySmall,
        ),
        const SizedBox(height: 20),
        if (isWide(context))
          Row(children: [if (known) forget, const Spacer(), keep])
        else ...[
          keep,
          if (known) ...[const SizedBox(height: 8), forget],
        ],
      ],
    );
  }
}
