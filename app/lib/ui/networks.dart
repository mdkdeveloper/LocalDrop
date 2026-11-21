import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../node/models.dart';
import '../theme/theme.dart';
import 'dialogs.dart';
import 'format.dart';
import 'widgets.dart';

// Networks and discovery (docs/networks.md): Settings › Networks, the network menu,
// the "Trust this network?" ask, discovery off, looking for devices and
// Connect by address.

IconData networkIcon(NetworkKind k) =>
    k == NetworkKind.wifi ? Icons.wifi : Icons.settings_ethernet;

String _kind(NetworkKind k) => k == NetworkKind.wifi ? 'Wi-Fi' : 'Ethernet';

String trustLabel(NetworkTrust t) => switch (t) {
  NetworkTrust.trusted => 'Trusted',
  NetworkTrust.notTrusted => 'Not trusted',
  NetworkTrust.neverAsk => 'Never ask',
};

/// "Wi-Fi · 192.168.1.23/24 · trusted".
String networkSub(NetworkView n) => [
  _kind(n.kind),
  ?n.address,
  if (n.connected)
    n.trust == NetworkTrust.trusted ? 'trusted' : 'not trusted'
  else
    n.weakIdentity
        ? 'subnet only'
        : 'last seen ${formatAgo(n.lastSeenUnix).toLowerCase()}',
].join(' · ');

class _NetIcon extends StatelessWidget {
  const _NetIcon(this.n);
  final NetworkView n;

  @override
  Widget build(BuildContext context) {
    final c = context.ld;
    return Container(
      width: 32,
      height: 32,
      decoration: BoxDecoration(
        color: c.surface2,
        borderRadius: BorderRadius.circular(9),
      ),
      child: Icon(
        networkIcon(n.kind),
        size: 18,
        color: n.connected ? c.mute : c.faint,
      ),
    );
  }
}

/// What a connected network shows on the right: Discovering, Trust, or its state.
class _ConnectedState extends StatelessWidget {
  const _ConnectedState(this.n);
  final NetworkView n;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    if (n.active) {
      return StateChip(
        s.discoveryOn ? 'Discovering' : 'Trusted',
        tone: s.discoveryOn ? ChipTone.accent : ChipTone.neutral,
      );
    }
    if (n.trust == NetworkTrust.trusted) {
      return const StateChip('Not selected');
    }
    return OutlinedButton(
      key: Key('trust-${n.key}'),
      onPressed: () => s.setNetworkTrust(n.key, NetworkTrust.trusted),
      style: OutlinedButton.styleFrom(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 6),
        visualDensity: VisualDensity.compact,
      ),
      child: const Text('Trust'),
    );
  }
}

// ---------------------------------------------------------------------- settings

/// Settings › Networks (wireframes `networks`, `d-networks`).
class NetworksScreen extends StatelessWidget {
  const NetworksScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    final set = s.networkSettings;
    final connected = s.connectedNetworks;

    final now = [
      const SectionLabel('Connected now'),
      if (connected.isEmpty)
        Card(
          child: ListTile(
            leading: Icon(Icons.wifi_off, color: c.faint),
            title: const Text('No network'),
            subtitle: Text(
              'Connect to Wi-Fi or Ethernet to find your devices.',
              style: t.bodySmall,
            ),
          ),
        )
      else
        GroupCard(
          children: [
            for (final n in connected)
              ListTile(
                leading: _NetIcon(n),
                title: Text(n.label, style: t.titleMedium),
                subtitle: Text(networkSub(n), style: t.bodySmall),
                trailing: _ConnectedState(n),
              ),
          ],
        ),
    ];

    final behaviour = [
      const SectionLabel('Discovery'),
      GroupCard(
        children: [
          SwitchListTile(
            title: const Text('Discovery at launch'),
            subtitle: Text(
              'Turn discovery on when LocalDrop opens',
              style: t.bodySmall,
            ),
            value: set.discoverAtLaunch,
            onChanged: (v) =>
                s.setNetworkSettings(set.copyWith(discoverAtLaunch: v)),
          ),
          ListTile(
            title: const Text('On a new network'),
            subtitle: Text(
              set.askOnNewNetwork
                  ? 'Ask whether to trust it'
                  : 'Stay silent; trust it here or from the network menu',
              style: t.bodySmall,
            ),
            trailing: DropdownButton<bool>(
              value: set.askOnNewNetwork,
              underline: const SizedBox.shrink(),
              items: const [
                DropdownMenuItem(value: true, child: Text('Ask to trust')),
                DropdownMenuItem(value: false, child: Text('Stay silent')),
              ],
              onChanged: (v) => v == null
                  ? null
                  : s.setNetworkSettings(set.copyWith(askOnNewNetwork: v)),
            ),
          ),
          SwitchListTile(
            key: const Key('trust-every'),
            title: const Text('Trust every network'),
            subtitle: Text(
              'Not recommended: announces this device on any Wi-Fi',
              style: t.bodySmall,
            ),
            value: set.trustEveryNetwork,
            onChanged: (v) async {
              if (v && !await _confirmTrustEvery(context)) return;
              await s.setNetworkSettings(set.copyWith(trustEveryNetwork: v));
            },
          ),
        ],
      ),
    ];

    final remembered = s.rememberedNetworks;
    final known = [
      const SectionLabel('Remembered networks'),
      if (remembered.isEmpty)
        Text('No decisions yet.', style: t.bodySmall)
      else
        GroupCard(children: [for (final n in remembered) _RememberedTile(n)]),
      Padding(
        padding: const EdgeInsets.fromLTRB(4, 10, 4, 0),
        child: Text(
          'No network is trusted until you say so. Networks are recognised on this '
          'device only, by the router’s hardware address (or by subnet when that isn’t '
          'available); nothing about them is sent to other devices.',
          style: t.bodySmall,
        ),
      ),
    ];

    return LayoutBuilder(
      builder: (context, box) {
        final two = box.maxWidth >= 860;
        return ListView(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
          children: two
              ? [
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [...now, ...behaviour],
                        ),
                      ),
                      const SizedBox(width: 24),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: known,
                        ),
                      ),
                    ],
                  ),
                ]
              : [...now, ...behaviour, ...known],
        );
      },
    );
  }
}

Future<bool> _confirmTrustEvery(BuildContext context) async =>
    await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Trust every network?'),
        content: const Text(
          'This device will look for your group on any Wi-Fi or Ethernet, including '
          'cafés, hotels and airports. Others there can see that a LocalDrop device is '
          'present, but not who you are or which group.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Trust every network'),
          ),
        ],
      ),
    ) ??
    false;

class _RememberedTile extends StatelessWidget {
  const _RememberedTile(this.n);
  final NetworkView n;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    return ListTile(
      leading: _NetIcon(n),
      title: Text(n.label, style: t.titleMedium),
      subtitle: Text(networkSub(n), style: t.bodySmall),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          DropdownButton<NetworkTrust>(
            key: Key('rule-${n.key}'),
            value: n.trust,
            underline: const SizedBox.shrink(),
            items: [
              for (final v in NetworkTrust.values)
                DropdownMenuItem(value: v, child: Text(trustLabel(v))),
            ],
            onChanged: (v) => v == null ? null : s.setNetworkTrust(n.key, v),
          ),
          PopupMenuButton<String>(
            tooltip: 'More',
            onSelected: (v) =>
                v == 'rename' ? _rename(context, n) : s.forgetNetwork(n.key),
            itemBuilder: (_) => const [
              PopupMenuItem(value: 'rename', child: Text('Rename…')),
              PopupMenuItem(value: 'forget', child: Text('Forget')),
            ],
          ),
        ],
      ),
    );
  }
}

Future<void> _rename(BuildContext context, NetworkView n) async {
  final s = AppScope.of(context);
  final name = await askText(
    context,
    title: 'Rename network',
    label: 'Name',
    initial: n.label,
    action: 'Rename',
  );
  if (name != null && name.trim().isNotEmpty && name.trim() != n.label) {
    await s.renameNetwork(n.key, name.trim());
  }
}

// ---------------------------------------------------------------------- network menu

/// Desktop network menu opened from the discovery pill (`d-net-menu`).
Future<void> showNetworkMenu(
  BuildContext context, {
  required VoidCallback onSettings,
}) => showDialog(
  context: context,
  barrierColor: const Color(0x1418181B),
  builder: (context) => Align(
    alignment: Alignment.topRight,
    child: Padding(
      padding: const EdgeInsets.fromLTRB(16, 56, 16, 16),
      child: Material(
        color: Colors.transparent,
        child: NetworkMenu(
          onSettings: () {
            Navigator.pop(context);
            onSettings();
          },
        ),
      ),
    ),
  ),
);

class NetworkMenu extends StatelessWidget {
  const NetworkMenu({super.key, required this.onSettings});
  final VoidCallback onSettings;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    final set = s.networkSettings;
    final connected = s.connectedNetworks;
    final trusted = connected
        .where((n) => n.trust == NetworkTrust.trusted)
        .toList();
    final kinds = connected.map((n) => _kind(n.kind)).toSet().join(' + ');

    Widget choice(String? only, String title, String sub) =>
        RadioListTile<String?>(
          value: only,
          dense: true,
          title: Text(
            title,
            style: t.bodyMedium?.copyWith(fontWeight: FontWeight.w600),
          ),
          subtitle: Text(sub, style: t.bodySmall),
          contentPadding: const EdgeInsets.symmetric(horizontal: 8),
        );

    return Container(
      width: 380,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(14),
        boxShadow: const [
          BoxShadow(
            color: Color(0x1F000000),
            blurRadius: 28,
            offset: Offset(0, 10),
          ),
        ],
      ),
      // ListTiles paint on the nearest Material, so the surface is a Material.
      child: Material(
        color: c.surface,
        clipBehavior: Clip.antiAlias,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(14),
          side: BorderSide(color: c.line),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 14, 16, 6),
              child: Row(
                children: [
                  Text('Search on', style: t.titleSmall),
                  const Spacer(),
                  Text(
                    connected.isEmpty ? 'Not connected' : '$kinds connected',
                    style: t.bodySmall,
                  ),
                ],
              ),
            ),
            RadioGroup<String?>(
              groupValue: set.onlyNetwork == null
                  ? null
                  : s.networkFor(set.onlyNetwork!)?.key,
              onChanged: (v) =>
                  s.setNetworkSettings(set.copyWith(onlyNetwork: () => v)),
              child: Column(
                children: [
                  choice(
                    null,
                    'Trusted networks',
                    trusted.isEmpty
                        ? 'None connected now'
                        : 'Now: ${trusted.map((n) => n.label).join(' + ')}',
                  ),
                  if (trusted.length > 1 || set.onlyNetwork != null)
                    for (final n in trusted)
                      choice(
                        n.key,
                        'Only ${n.label}',
                        [_kind(n.kind), ?n.address].join(' · '),
                      ),
                ],
              ),
            ),
            const Divider(height: 1),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 10, 16, 2),
              child: Text('CONNECTED', style: t.labelSmall),
            ),
            for (final n in connected)
              ListTile(
                dense: true,
                leading: _NetIcon(n),
                title: Text(
                  n.label,
                  style: t.bodyMedium?.copyWith(fontWeight: FontWeight.w600),
                ),
                subtitle: Text(
                  '${_kind(n.kind)} · ${n.trust == NetworkTrust.trusted ? 'trusted' : 'not trusted'}',
                  style: t.bodySmall,
                ),
                trailing: _ConnectedState(n),
              ),
            const Divider(height: 1),
            ListTile(
              dense: true,
              title: Text(
                'Network settings',
                style: t.bodyMedium?.copyWith(fontWeight: FontWeight.w600),
              ),
              trailing: Icon(Icons.chevron_right, color: c.mute),
              onTap: onSettings,
            ),
          ],
        ),
      ),
    );
  }
}

// ---------------------------------------------------------------------- new network

/// "Trust “Office-5G”?" when no trusted network is connected
/// (`d-new-net` in the main area, `new-net` as a sheet on phones).
class NewNetworkAsk extends StatelessWidget {
  const NewNetworkAsk(this.ask, {super.key, this.inSheet = false});
  final UntrustedNetwork ask;
  final bool inSheet;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    final n = s.networkFor(ask.key);
    final where = n == null ? null : [_kind(n.kind), ?n.address].join(' · ');
    void answer({required bool trust, bool neverAsk = false}) {
      s.answerNetwork(trust: trust, neverAsk: neverAsk);
      if (inSheet) Navigator.of(context).pop();
    }

    final buttons = [
      FilledButton(
        onPressed: () => answer(trust: true),
        child: const Text('Trust this network'),
      ),
      OutlinedButton(
        onPressed: () => answer(trust: false),
        child: const Text('Not now'),
      ),
      OutlinedButton(
        onPressed: () => answer(trust: false, neverAsk: true),
        child: const Text('Never ask'),
      ),
    ];

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 48,
          height: 48,
          decoration: BoxDecoration(
            color: c.surface2,
            borderRadius: BorderRadius.circular(14),
          ),
          child: Icon(
            n == null ? Icons.wifi : networkIcon(n.kind),
            color: c.mute,
          ),
        ),
        const SizedBox(height: 12),
        Text(
          'Network not trusted',
          style: t.labelLarge?.copyWith(color: c.mute),
        ),
        const SizedBox(height: 6),
        Text(
          'Trust “${ask.label}”?',
          style: t.headlineSmall,
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 8),
        Text(
          'LocalDrop only searches on networks you trust. Here it stays silent: your '
          'devices can’t see this one and you can’t see them.',
          style: t.bodyMedium?.copyWith(color: c.mute),
          textAlign: TextAlign.center,
        ),
        if (where != null) ...[
          const SizedBox(height: 6),
          Text(where, style: t.bodySmall),
        ],
        const SizedBox(height: 20),
        if (inSheet)
          Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (final b in buttons)
                Padding(padding: const EdgeInsets.only(bottom: 8), child: b),
            ],
          )
        else
          Wrap(
            spacing: 10,
            runSpacing: 8,
            alignment: WrapAlignment.center,
            children: buttons.reversed.toList(),
          ),
        const SizedBox(height: 8),
        Text('Change it later in Settings › Networks.', style: t.bodySmall),
      ],
    );
  }
}

Future<void> showNewNetworkSheet(BuildContext context, UntrustedNetwork ask) =>
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      builder: (_) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 4, 20, 12),
          child: NewNetworkAsk(ask, inSheet: true),
        ),
      ),
    );

// ---------------------------------------------------------------------- discovery off

const _offDesktop =
    'This device isn’t looking for your devices and they can’t find it. Queued files '
    'wait until you turn it back on. Nothing is sent or received meanwhile.';
const _offPhone =
    'Not looking for your devices, and they can’t find this phone. Queued files wait.';

/// `disc-off` / `d-disc-off`: what discovery off means, and the way back.
class DiscoveryOffCard extends StatelessWidget {
  const DiscoveryOffCard({super.key, this.compact = false});
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    return Card(
      child: Padding(
        padding: EdgeInsets.all(compact ? 20 : 28),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: compact ? 40 : 48,
              height: compact ? 40 : 48,
              decoration: BoxDecoration(
                color: c.surface2,
                borderRadius: BorderRadius.circular(14),
              ),
              child: Icon(Icons.portable_wifi_off, color: c.mute),
            ),
            const SizedBox(height: 12),
            Text('Discovery is off', style: t.titleLarge),
            const SizedBox(height: 8),
            Text(
              compact ? _offPhone : _offDesktop,
              style: t.bodyMedium?.copyWith(color: c.mute),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 16),
            SizedBox(
              width: compact ? double.infinity : null,
              child: FilledButton(
                onPressed: () => s.setDiscovery(true),
                child: const Text('Turn on discovery'),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// "Transfers paused · 2 queued · View" while discovery is off.
class PausedBar extends StatelessWidget {
  const PausedBar({super.key, required this.onView});
  final VoidCallback onView;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    final queued = s.waitingCount;
    return Container(
      padding: const EdgeInsets.fromLTRB(16, 6, 8, 6),
      decoration: BoxDecoration(
        color: c.surface,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: c.line),
      ),
      child: Row(
        children: [
          Icon(Icons.pause_circle_outline, color: c.mute, size: 20),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              queued == 0
                  ? 'Transfers paused'
                  : 'Transfers paused · $queued queued',
              style: t.bodyMedium?.copyWith(fontWeight: FontWeight.w500),
            ),
          ),
          TextButton(onPressed: onView, child: const Text('View')),
        ],
      ),
    );
  }
}

// ---------------------------------------------------------------------- looking

/// `looking` / `d-looking`: joined, nobody found yet.
class LookingPanel extends StatelessWidget {
  const LookingPanel({super.key, required this.onOpen, this.compact = false});
  final ValueChanged<String> onOpen;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    final on = s.searchingOn;
    final where = on.isEmpty
        ? 'Not searching: no trusted network is connected.'
        : 'Searching on ${on.map((n) => '${n.label} (${_kind(n.kind)}, trusted)').join(' and ')}.';
    final found = s.onlineDevices.firstOrNull;
    final steps = [
      (
        'Open LocalDrop on another device',
        'Phone, tablet or computer on the same network.',
      ),
      (
        'Join with the same name and password',
        'Group “${s.group?.name}”. The fingerprint must show ${s.group?.fingerprint}.',
      ),
      (
        'Trust this network there too',
        'Both devices search only on networks they trust.',
      ),
    ];

    return Card(
      child: Padding(
        padding: EdgeInsets.all(compact ? 16 : 32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Center(
              child: Container(
                width: compact ? 72 : 104,
                height: compact ? 72 : 104,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  border: Border.all(color: c.line),
                ),
                alignment: Alignment.center,
                child: Container(
                  width: compact ? 48 : 64,
                  height: compact ? 48 : 64,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: c.accentSoft,
                  ),
                  child: Icon(Icons.radar, color: c.accent, size: 28),
                ),
              ),
            ),
            const SizedBox(height: 16),
            Text(
              'Looking for your devices…',
              style: compact ? t.titleLarge : t.headlineSmall,
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 4),
            Text(
              where,
              style: t.bodyMedium?.copyWith(color: c.mute),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 20),
            for (final (i, (title, sub)) in steps.indexed)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    CircleAvatar(
                      radius: 12,
                      backgroundColor: c.surface2,
                      child: Text(
                        '${i + 1}',
                        style: t.labelMedium?.copyWith(color: c.ink),
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            title,
                            style: t.bodyMedium?.copyWith(
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                          Text(sub, style: t.bodySmall),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            if (found != null)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: OutlinedButton(
                  onPressed: () => onOpen(found.id),
                  child: Text('${found.name} found  ›'),
                ),
              ),
            Text(
              'Not showing up? Check that both devices are on the same network and the '
              'firewall allows LocalDrop.',
              style: t.bodySmall,
            ),
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton(
                key: const Key('connect-by-address'),
                onPressed: () => showConnectByAddress(context),
                style: TextButton.styleFrom(padding: EdgeInsets.zero),
                child: const Text('Connect by address…'),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ---------------------------------------------------------------------- connect by address

/// `connect` (sheet) / `d-connect` (dialog).
Future<void> showConnectByAddress(BuildContext context) {
  if (isWide(context)) {
    return showDialog(
      context: context,
      builder: (_) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 480),
          child: const Padding(
            padding: EdgeInsets.all(28),
            child: ConnectForm(),
          ),
        ),
      ),
    );
  }
  return showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    builder: (context) => Padding(
      padding: EdgeInsets.only(bottom: MediaQuery.viewInsetsOf(context).bottom),
      child: const SafeArea(
        child: Padding(
          padding: EdgeInsets.fromLTRB(20, 4, 20, 16),
          child: ConnectForm(),
        ),
      ),
    ),
  );
}

/// Checks `host` and `port` before asking the node; returns the problem or null.
String? addressProblem(String host, String port) {
  final h = host.trim();
  if (h.isEmpty) return 'Enter an IP address or a name.';
  if (h.contains(' ') || h.contains('/')) {
    return 'That doesn’t look like an address.';
  }
  if (port.trim().isNotEmpty) {
    final p = int.tryParse(port.trim());
    if (p == null || p < 1 || p > 65535) {
      return 'The port is a number from 1 to 65535.';
    }
  }
  return null;
}

class ConnectForm extends StatefulWidget {
  const ConnectForm({super.key});

  @override
  State<ConnectForm> createState() => _ConnectFormState();
}

class _ConnectFormState extends State<ConnectForm> {
  final _host = TextEditingController();
  final _port = TextEditingController();
  bool _remember = true;
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _host.dispose();
    _port.dispose();
    super.dispose();
  }

  Future<void> _connect() async {
    final problem = addressProblem(_host.text, _port.text);
    if (problem != null) {
      setState(() => _error = problem);
      return;
    }
    setState(() {
      _busy = true;
      _error = null;
    });
    final s = AppScope.of(context);
    try {
      await s.connectAddress(
        _host.text.trim(),
        port: _port.text.trim().isEmpty ? null : int.parse(_port.text.trim()),
        remember: _remember,
      );
      if (mounted) Navigator.of(context).pop();
    } catch (e) {
      final msg = e is StateError ? e.message : '$e';
      if (mounted) {
        setState(() {
          _busy = false;
          _error = msg;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    final wide = isWide(context);
    final actions = [
      FilledButton(
        key: const Key('connect'),
        onPressed: _busy ? null : _connect,
        child: _busy
            ? const SizedBox(
                width: 18,
                height: 18,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            : const Text('Connect'),
      ),
      OutlinedButton(
        onPressed: _busy ? null : () => Navigator.of(context).pop(),
        child: const Text('Cancel'),
      ),
    ];

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Connect by address', style: t.titleLarge),
        const SizedBox(height: 6),
        Text(
          'For networks where devices can’t find each other (guest Wi-Fi, VPN, mDNS '
          'blocked).',
          style: t.bodyMedium?.copyWith(color: c.mute),
        ),
        const SizedBox(height: 16),
        TextField(
          key: const Key('address'),
          controller: _host,
          autofocus: true,
          enabled: !_busy,
          keyboardType: TextInputType.url,
          decoration: InputDecoration(
            labelText: 'Address',
            hintText: '192.168.1.42',
            helperText: 'IP address or name, e.g. desk-pc.local',
            errorText: _error,
            errorMaxLines: 3,
          ),
          onSubmitted: (_) => _connect(),
        ),
        const SizedBox(height: 12),
        TextField(
          key: const Key('port'),
          controller: _port,
          enabled: !_busy,
          keyboardType: TextInputType.number,
          decoration: const InputDecoration(
            labelText: 'Port',
            hintText: 'Auto',
            helperText:
                'Leave empty unless the other device uses a fixed port.',
          ),
        ),
        CheckboxListTile(
          value: _remember,
          onChanged: _busy
              ? null
              : (v) => setState(() => _remember = v ?? true),
          contentPadding: EdgeInsets.zero,
          controlAffinity: ListTileControlAffinity.leading,
          title: const Text('Remember this address'),
          subtitle: Text(
            'Try it again whenever LocalDrop starts',
            style: t.bodySmall,
          ),
        ),
        Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: c.bg,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(Icons.shield_outlined, size: 18, color: c.mute),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  'The device still has to prove it knows the group password. The '
                  'address must be on a trusted network or an adapter you added (e.g. a '
                  'VPN).',
                  style: t.bodySmall,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 20),
        if (wide)
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [actions[1], const SizedBox(width: 10), actions[0]],
          )
        else ...[
          actions[0],
          const SizedBox(height: 8),
          actions[1],
        ],
      ],
    );
  }
}
