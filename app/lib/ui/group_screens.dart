import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../node/models.dart';
import '../theme/theme.dart';
import 'dialogs.dart';
import 'join_screen.dart'
    show generatePassphrase, minPasswordLength, passwordProblem;
import 'widgets.dart';

// Settings › General / Group (wireframes `d-general`, `group`), Change password
// (`change-pass`, `d-change-pass`) and Leave group (`leave`, `d-leave`).

/// `a91f-07c2-5d1e`: the start of a device id, for people to compare.
String shortId(String id) => id.length < 12
    ? id
    : '${id.substring(0, 4)}-${id.substring(4, 8)}-${id.substring(8, 12)}';

const _changeText =
    'A new password makes a new group key. Devices that don’t get it drop out until '
    'they join again with it.';
const _leaveText =
    'This device forgets the group key and stops finding your devices. Files on disk '
    'aren’t touched.';

/// The group card: name, fingerprint, change password, leave.
class GroupSection extends StatelessWidget {
  const GroupSection({super.key});

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final g = s.group;
    return GroupCard(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 14, 8, 14),
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Group', style: t.bodySmall),
                    Text(g?.name ?? '—', style: t.titleMedium),
                  ],
                ),
              ),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Fingerprint', style: t.bodySmall),
                    Text(
                      g?.fingerprint ?? '—',
                      key: const Key('fingerprint'),
                      style: t.titleMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                        letterSpacing: 1,
                      ),
                    ),
                  ],
                ),
              ),
              TextButton(
                onPressed: g == null
                    ? null
                    : () =>
                          Clipboard.setData(ClipboardData(text: g.fingerprint)),
                child: const Text('Copy'),
              ),
            ],
          ),
        ),
        ListTile(
          title: const Text('Change password'),
          subtitle: Text(
            'Devices without the new one drop out',
            style: t.bodySmall,
          ),
          trailing: OutlinedButton(
            key: const Key('change-password'),
            onPressed: () => showChangePassword(context),
            child: const Text('Change…'),
          ),
        ),
        ListTile(
          title: const Text('Leave group'),
          subtitle: Text(
            'Forget the group key on this device. Queues are kept.',
            style: t.bodySmall,
          ),
          trailing: OutlinedButton(
            key: const Key('leave-group'),
            onPressed: () => showLeaveGroup(context),
            child: const Text('Leave'),
          ),
        ),
      ],
    );
  }
}

/// This device: name, visibility, id and the listening port.
class ThisDeviceSection extends StatelessWidget {
  const ThisDeviceSection({super.key});

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final me = s.me;
    return GroupCard(
      children: [
        ListTile(
          key: const Key('device-name'),
          title: const Text('This device’s name'),
          subtitle: Text('Shown to your other devices', style: t.bodySmall),
          trailing: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(me?.name ?? s.deviceName, style: t.bodyMedium),
              const Icon(Icons.chevron_right),
            ],
          ),
          onTap: () => _rename(context),
        ),
        SwitchListTile(
          title: const Text('Visible to your group'),
          subtitle: Text(
            'Off: you can still send, others can’t find this device',
            style: t.bodySmall,
          ),
          value: me?.visible ?? true,
          onChanged: me == null ? null : s.setVisible,
        ),
        ListTile(
          title: const Text('Device ID'),
          subtitle: Text('Tied to this install’s key', style: t.bodySmall),
          trailing: Text(
            me == null ? '—' : shortId(me.id),
            style: t.bodyMedium?.copyWith(color: context.ld.mute),
          ),
        ),
        ListTile(
          key: const Key('listening-port'),
          title: const Text('Listening port'),
          subtitle: Text(
            me?.fixedPort == 0
                ? 'Auto: a new port at every start${me?.port == null ? '' : ' · now ${me!.port}'}'
                : 'Fixed, so others can connect by address',
            style: t.bodySmall,
          ),
          trailing: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                me == null || me.fixedPort == 0 ? 'Auto' : '${me.fixedPort}',
                style: t.bodyMedium,
              ),
              const Icon(Icons.chevron_right),
            ],
          ),
          onTap: me == null ? null : () => _port(context, me),
        ),
      ],
    );
  }

  Future<void> _rename(BuildContext context) async {
    final s = AppScope.of(context);
    final name = await askText(
      context,
      title: 'This device’s name',
      label: 'Name',
      initial: s.me?.name ?? s.deviceName,
      action: 'Rename',
    );
    if (name != null && name.trim().isNotEmpty) await s.setDeviceName(name);
  }

  Future<void> _port(BuildContext context, ThisDevice me) async {
    final s = AppScope.of(context);
    final v = await askText(
      context,
      title: 'Listening port',
      label: 'Port',
      initial: me.fixedPort == 0 ? '' : '${me.fixedPort}',
      hint:
          'Empty = Auto. 47474 lets others connect by address without a port.',
      action: 'Save',
      number: true,
    );
    if (v == null) return;
    final port = v.trim().isEmpty ? 0 : int.tryParse(v.trim());
    if (port == null || (port != 0 && (port < 1024 || port > 65535))) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Use a port from 1024 to 65535, or Auto'),
          ),
        );
      }
      return;
    }
    await s.setFixedPort(port);
  }
}

// ---------------------------------------------------------------------- change password

/// Dialog on desktop, a page on phones.
Future<void> showChangePassword(BuildContext context) {
  if (isWide(context)) {
    return showDialog(
      context: context,
      builder: (_) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 560),
          child: const Padding(
            padding: EdgeInsets.all(28),
            child: ChangePasswordForm(),
          ),
        ),
      ),
    );
  }
  return Navigator.of(context).push(
    MaterialPageRoute(
      builder: (context) => Scaffold(
        appBar: AppBar(
          title: Text(
            'Change password · ${AppScope.of(context).group?.name ?? ''}',
          ),
        ),
        body: const SingleChildScrollView(
          padding: EdgeInsets.fromLTRB(20, 8, 20, 24),
          child: ChangePasswordForm(),
        ),
      ),
    ),
  );
}

class ChangePasswordForm extends StatefulWidget {
  const ChangePasswordForm({super.key});

  @override
  State<ChangePasswordForm> createState() => _ChangePasswordFormState();
}

class _ChangePasswordFormState extends State<ChangePasswordForm> {
  final _current = TextEditingController();
  final _new = TextEditingController();
  bool _busy = false;
  String? _currentError;
  String? _newError;
  String? _before;
  String? _after;

  @override
  void dispose() {
    _current.dispose();
    _new.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final current = _current.text;
    final next = _new.text;
    setState(() {
      _currentError = current.isEmpty ? 'Enter the current password.' : null;
      _newError = passwordProblem(next);
    });
    if (_currentError != null || _newError != null) return;
    final s = AppScope.of(context);
    final before = s.group?.fingerprint;
    setState(() => _busy = true);
    try {
      final info = await s.changePassword(current, next);
      if (!mounted) return;
      setState(() {
        _busy = false;
        _before = before;
        _after = info.fingerprint;
      });
    } catch (e) {
      final msg = e is StateError ? e.message : '$e';
      if (!mounted) return;
      setState(() {
        _busy = false;
        if (msg.toLowerCase().contains('current')) {
          _currentError = msg;
        } else {
          _newError = msg;
        }
      });
    }
  }

  Widget _note(String text) {
    final t = Theme.of(context).textTheme;
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 6, right: 10),
            child: Icon(Icons.circle, size: 6, color: context.ld.mute),
          ),
          Expanded(
            child: Text(
              text,
              style: t.bodyMedium?.copyWith(color: context.ld.mute),
            ),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    final wide = isWide(context);
    final done = _after != null;

    final fingerprints = Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: c.bg,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        children: [
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Old fingerprint', style: t.bodySmall),
              Text(
                _before ?? '',
                style: t.titleLarge?.copyWith(color: c.faint, letterSpacing: 1),
              ),
            ],
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Icon(Icons.arrow_forward, color: c.mute),
          ),
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('New fingerprint', style: t.bodySmall),
              Text(
                _after ?? '',
                key: const Key('new-fingerprint'),
                style: t.titleLarge?.copyWith(letterSpacing: 1),
              ),
            ],
          ),
        ],
      ),
    );

    final actions = done
        ? [
            FilledButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('Done'),
            ),
          ]
        : [
            OutlinedButton(
              onPressed: _busy ? null : () => Navigator.of(context).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              key: const Key('submit-password'),
              onPressed: _busy ? null : _submit,
              child: _busy
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Change password'),
            ),
          ];

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (wide) ...[
          Text(
            done ? 'Password changed' : 'Change group password',
            style: t.titleLarge,
          ),
          const SizedBox(height: 8),
        ],
        Text(
          done
              ? 'Enter the new password on each device you keep. Compare the new '
                    'fingerprint there.'
              : _changeText,
          style: t.bodyMedium?.copyWith(color: c.mute),
        ),
        const SizedBox(height: 16),
        if (!done) ...[
          TextField(
            key: const Key('current-password'),
            controller: _current,
            obscureText: true,
            enabled: !_busy,
            decoration: InputDecoration(
              labelText: 'Current password',
              helperText: 'Checked on this device; it isn’t stored anywhere.',
              errorText: _currentError,
            ),
          ),
          const SizedBox(height: 14),
          TextField(
            key: const Key('new-password'),
            controller: _new,
            enabled: !_busy,
            decoration: InputDecoration(
              labelText: 'New password',
              helperText:
                  'At least $minPasswordLength characters, or a generated passphrase.',
              errorText: _newError,
              errorMaxLines: 2,
              suffixIcon: TextButton(
                onPressed: _busy
                    ? null
                    : () => setState(() => _new.text = generatePassphrase()),
                child: const Text('Generate'),
              ),
            ),
            onSubmitted: (_) => _submit(),
          ),
          const SizedBox(height: 16),
        ] else ...[
          fingerprints,
          const SizedBox(height: 16),
        ],
        _note('Lost or unwanted device? Just don’t give it the new password.'),
        _note(
          'Queues and known devices are kept; each device reconnects once it has the new '
          'password.',
        ),
        const SizedBox(height: 16),
        if (wide)
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              for (final (i, a) in actions.indexed) ...[
                if (i > 0) const SizedBox(width: 10),
                a,
              ],
            ],
          )
        else
          for (final a in actions.reversed)
            Padding(padding: const EdgeInsets.only(bottom: 8), child: a),
      ],
    );
  }
}

// ---------------------------------------------------------------------- leave

Future<void> showLeaveGroup(BuildContext context) => isWide(context)
    ? showDialog(
        context: context,
        builder: (_) => Dialog(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 480),
            child: const Padding(
              padding: EdgeInsets.all(28),
              child: LeaveForm(),
            ),
          ),
        ),
      )
    : showModalBottomSheet(
        context: context,
        isScrollControlled: true,
        builder: (_) => const SafeArea(
          child: Padding(
            padding: EdgeInsets.fromLTRB(20, 4, 20, 16),
            child: LeaveForm(),
          ),
        ),
      );

class LeaveForm extends StatefulWidget {
  const LeaveForm({super.key});

  @override
  State<LeaveForm> createState() => _LeaveFormState();
}

class _LeaveFormState extends State<LeaveForm> {
  bool _keepQueue = true;
  bool _forget = false;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    final queued = s.activeJobs(Direction.outgoing).length;
    final known = s.devices.length;
    final wide = isWide(context);
    final actions = [
      OutlinedButton(
        onPressed: () => Navigator.of(context).pop(),
        child: const Text('Cancel'),
      ),
      FilledButton(
        key: const Key('confirm-leave'),
        style: FilledButton.styleFrom(backgroundColor: c.danger),
        onPressed: () async {
          // Close every route first: after leaving, the app shows the join screen.
          Navigator.of(context).popUntil((r) => r.isFirst);
          await s.leaveGroup(keepQueue: _keepQueue, forgetDevices: _forget);
        },
        child: const Text('Leave group'),
      ),
    ];
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Leave group “${s.group?.name}”?', style: t.titleLarge),
        const SizedBox(height: 8),
        Text(_leaveText, style: t.bodyMedium?.copyWith(color: c.mute)),
        const SizedBox(height: 12),
        CheckboxListTile(
          value: _keepQueue,
          onChanged: (v) => setState(() => _keepQueue = v ?? true),
          contentPadding: EdgeInsets.zero,
          controlAffinity: ListTileControlAffinity.leading,
          title: const Text('Keep the send queue'),
          subtitle: Text(
            queued == 0
                ? 'Nothing queued now'
                : '$queued ${queued == 1 ? 'job' : 'jobs'} · sent if you join this group again',
            style: t.bodySmall,
          ),
        ),
        CheckboxListTile(
          key: const Key('forget-devices'),
          value: _forget,
          onChanged: (v) => setState(() => _forget = v ?? false),
          contentPadding: EdgeInsets.zero,
          controlAffinity: ListTileControlAffinity.leading,
          title: const Text('Forget known devices'),
          subtitle: Text(
            'Otherwise the $known ${known == 1 ? 'device is' : 'devices are'} recognised '
            'again if you rejoin',
            style: t.bodySmall,
          ),
        ),
        const SizedBox(height: 4),
        Text(
          'Shared folders, backups and trusted networks stay as they are.',
          style: t.bodySmall,
        ),
        const SizedBox(height: 20),
        if (wide)
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [actions[0], const SizedBox(width: 10), actions[1]],
          )
        else ...[
          actions[1],
          const SizedBox(height: 8),
          actions[0],
        ],
      ],
    );
  }
}

/// Phones: Settings › Group as its own page (`group`).
class GroupPage extends StatelessWidget {
  const GroupPage({super.key});

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    final g = s.group;
    return Scaffold(
      appBar: AppBar(title: const Text('Group')),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
        children: [
          Card(
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Column(
                children: [
                  Text(
                    g?.name ?? '',
                    style: t.titleMedium?.copyWith(color: c.mute),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    g?.fingerprint ?? '',
                    key: const Key('fingerprint'),
                    style: t.headlineMedium?.copyWith(
                      fontWeight: FontWeight.w700,
                      letterSpacing: 2,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    'Must match on every device in the group',
                    style: t.bodySmall,
                  ),
                ],
              ),
            ),
          ),
          const SectionLabel('Password'),
          GroupCard(
            children: [
              ListTile(
                key: const Key('change-password'),
                title: const Text('Change password'),
                subtitle: Text(
                  'Devices that don’t get the new one drop out',
                  style: t.bodySmall,
                ),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => showChangePassword(context),
              ),
            ],
          ),
          const SectionLabel('This device'),
          const ThisDeviceSection(),
          const SizedBox(height: 20),
          OutlinedButton(
            key: const Key('leave-group'),
            onPressed: () => showLeaveGroup(context),
            child: const Text('Leave group'),
          ),
          const SizedBox(height: 8),
          Text(
            'Leaving forgets the group key on this device. You can join again with the '
            'same name and password.',
            style: t.bodySmall,
          ),
        ],
      ),
    );
  }
}
