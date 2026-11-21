import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../node/models.dart';
import '../state/app_state.dart';
import '../theme/theme.dart';
import 'device_screen.dart';
import 'devices_screen.dart';
import 'dialogs.dart';
import 'known_devices.dart';
import 'format.dart';
import 'networks.dart';
import 'placeholder_screen.dart';
import 'settings_screen.dart';
import 'transfers_screen.dart';
import 'widgets.dart';

/// Where the user is: a top-level section, or one device on desktop.
sealed class Place {
  const Place();
}

class SectionPlace extends Place {
  const SectionPlace(this.section);
  final Section section;
  @override
  bool operator ==(Object other) =>
      other is SectionPlace && other.section == section;
  @override
  int get hashCode => section.hashCode;
}

class DevicePlace extends Place {
  const DevicePlace(this.id);
  final String id;
  @override
  bool operator ==(Object other) => other is DevicePlace && other.id == id;
  @override
  int get hashCode => id.hashCode;
}

/// Desktop: nobody found yet (`d-looking`).
class LookingPlace extends Place {
  const LookingPlace();
  @override
  bool operator ==(Object other) => other is LookingPlace;
  @override
  int get hashCode => 0;
}

/// Phone tabs (wireframe bottom nav); desktop shows devices in the sidebar instead.
enum Section { files, transfers, folders, settings }

/// Main frame after joining: sidebar on desktop, bottom navigation on phones.
class Shell extends StatefulWidget {
  const Shell({super.key});

  @override
  State<Shell> createState() => _ShellState();
}

class _ShellState extends State<Shell> {
  Place _place = const SectionPlace(Section.transfers);
  SettingsTab _settingsTab = SettingsTab.general;
  final _shown = <String>{};
  AppState? _state;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final s = AppScope.of(context);
    if (!identical(s, _state)) {
      if (_state == null && s.devices.isEmpty && s.jobs.isEmpty) {
        // First run: nothing to show yet but the search.
        _place = isWide(context)
            ? const LookingPlace()
            : const SectionPlace(Section.files);
      }
      _state?.removeListener(_onState);
      _state = s..addListener(_onState);
      _onState(); // decisions that arrived before the shell was built
    }
  }

  @override
  void dispose() {
    _state?.removeListener(_onState);
    super.dispose();
  }

  /// Opens each new decision once, and shows errors as a snackbar.
  void _onState() {
    final s = _state!;
    for (final d in s.decisions) {
      final key = '${d.runtimeType}:${d.jobId}';
      if (_shown.add(key)) {
        WidgetsBinding.instance
          ..addPostFrameCallback((_) {
            if (mounted && s.decisions.contains(d)) showDecision(context, d);
          })
          ..ensureVisualUpdate();
      }
    }
    final ask = s.untrustedNetwork;
    if (ask != null &&
        s.askProminently &&
        !isWide(context) &&
        _shown.add('net:${ask.key}')) {
      WidgetsBinding.instance
        ..addPostFrameCallback((_) {
          if (mounted && s.untrustedNetwork == ask) {
            showNewNetworkSheet(context, ask);
          }
        })
        ..ensureVisualUpdate();
    }
    for (final m in s.keyMismatches) {
      if (_shown.add('key:${m.device}')) {
        WidgetsBinding.instance
          ..addPostFrameCallback((_) {
            if (mounted && s.keyMismatches.contains(m)) {
              showKeyMismatch(context, m);
            }
          })
          ..ensureVisualUpdate();
      }
    }
    final err = s.lastError;
    if (err != null) {
      s.clearError();
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          ScaffoldMessenger.of(context)
              .showSnackBar(SnackBar(content: Text(err)));
        }
      });
    }
  }

  void _go(Place p) => setState(() => _place = p);

  void _openNetworkSettings() {
    if (isWide(context)) {
      setState(() {
        _settingsTab = SettingsTab.networks;
        _place = const SectionPlace(Section.settings);
      });
    } else {
      openNetworksPage(context);
    }
  }

  @override
  Widget build(BuildContext context) =>
      isWide(context) ? _desktop(context) : _phone(context);

  Widget _content(Place p) => switch (p) {
    SectionPlace(section: Section.transfers) => const TransfersScreen(),
    SectionPlace(section: Section.files) => DevicesScreen(
      onOpen: (id) => _openDevice(id),
      onTransfers: () => _go(const SectionPlace(Section.transfers)),
    ),
    SectionPlace(section: Section.folders) => const PlaceholderScreen.folders(),
    SectionPlace(section: Section.settings) => SettingsScreen(
      tab: _settingsTab,
      onTab: (t) => setState(() => _settingsTab = t),
    ),
    DevicePlace(:final id) => DeviceScreen(id),
    LookingPlace() => _centered(
      560,
      LookingPanel(onOpen: (id) => _go(DevicePlace(id))),
    ),
  };

  Widget _centered(double width, Widget child) => Center(
    child: SingleChildScrollView(
      padding: const EdgeInsets.all(24),
      child: ConstrainedBox(
        constraints: BoxConstraints(maxWidth: width),
        child: child,
      ),
    ),
  );

  /// Desktop main area: the place, unless discovery state takes over
  /// (`d-new-net`, `d-disc-off`).
  Widget _desktopMain(Place place) {
    final s = AppScope.of(context);
    final settings = place == const SectionPlace(Section.settings);
    final ask = s.untrustedNetwork;
    if (ask != null && s.askProminently && !settings) {
      return _centered(
        540,
        Card(
          child: Padding(
            padding: const EdgeInsets.all(32),
            child: NewNetworkAsk(ask),
          ),
        ),
      );
    }
    if (!s.discoveryOn && (place is DevicePlace || place is LookingPlace)) {
      return Column(
        children: [
          Expanded(child: _centered(460, const DiscoveryOffCard())),
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
            child: PausedBar(
              onView: () => _go(const SectionPlace(Section.transfers)),
            ),
          ),
        ],
      );
    }
    return _content(place);
  }

  void _openDevice(String id) {
    if (isWide(context)) {
      _go(DevicePlace(id));
    } else {
      Navigator.of(context).push(
        MaterialPageRoute(
          builder: (_) => Scaffold(appBar: AppBar(), body: DeviceScreen(id)),
        ),
      );
    }
  }

  Widget _desktop(BuildContext context) {
    // A phone tab that has no desktop place maps to Transfers.
    final place =
        _place is SectionPlace &&
            (_place as SectionPlace).section != Section.settings &&
            (_place as SectionPlace).section != Section.transfers
        ? const SectionPlace(Section.transfers)
        : _place;
    return Scaffold(
      body: Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _Sidebar(place: place, onGo: _go),
          Expanded(
            child: Column(
              children: [
                _Toolbar(
                  title: _title(place),
                  onNetworks: () => showNetworkMenu(
                    context,
                    onSettings: _openNetworkSettings,
                  ),
                ),
                const _NetworkBanner(),
                Expanded(child: _desktopMain(place)),
              ],
            ),
          ),
        ],
      ),
    );
  }

  String _title(Place p) => switch (p) {
    SectionPlace(section: Section.settings) => 'Settings',
    DevicePlace(:final id) => AppScope.of(context).nameOf(id),
    SectionPlace() ||
    LookingPlace() => '', // these screens have their own header
  };

  Widget _phone(BuildContext context) {
    final section = switch (_place) {
      SectionPlace(:final section) => section,
      DevicePlace() || LookingPlace() => Section.files,
    };
    final s = AppScope.of(context);
    return Scaffold(
      body: SafeArea(
        bottom: false,
        child: Column(
          children: [
            const _NetworkBanner(),
            Expanded(child: _content(SectionPlace(section))),
          ],
        ),
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: section.index,
        onDestinationSelected: (i) => _go(SectionPlace(Section.values[i])),
        destinations: [
          const NavigationDestination(
            icon: Icon(Icons.devices_outlined),
            selectedIcon: Icon(Icons.devices),
            label: 'Files',
          ),
          NavigationDestination(
            icon: Badge(
              isLabelVisible: s.runningCount > 0,
              label: Text('${s.runningCount}'),
              child: const Icon(Icons.swap_vert),
            ),
            label: 'Transfers',
          ),
          const NavigationDestination(
            icon: Icon(Icons.folder_shared_outlined),
            selectedIcon: Icon(Icons.folder_shared),
            label: 'Folders',
          ),
          const NavigationDestination(
            icon: Icon(Icons.settings_outlined),
            selectedIcon: Icon(Icons.settings),
            label: 'Settings',
          ),
        ],
      ),
    );
  }
}

class _Sidebar extends StatelessWidget {
  const _Sidebar({required this.place, required this.onGo});
  final Place place;
  final ValueChanged<Place> onGo;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    final online = s.onlineDevices;
    final offline = s.offlineDevices;

    Widget device(DeviceView d) => _SideItem(
      selected: place == DevicePlace(d.id),
      lead: DeviceAvatar(d, size: 28),
      title: d.name,
      sub: d.presence == Presence.offline
          ? [
              if (d.lastSeenUnix > 0)
                'Last seen ${formatAgo(d.lastSeenUnix).toLowerCase()}',
              if (d.queuedJobs > 0) '${d.queuedJobs} queued',
            ].join(' · ')
          : d.presence == Presence.found
          ? 'Connecting…'
          : 'Online',
      trailing: d.presence == Presence.offline && d.queuedJobs > 0
          ? StateChip('${d.queuedJobs}')
          : null,
      onTap: () => onGo(DevicePlace(d.id)),
    );

    return Container(
      width: LdSize.sidebarWidth,
      decoration: BoxDecoration(
        color: c.surface,
        border: Border(right: BorderSide(color: c.line)),
      ),
      padding: const EdgeInsets.fromLTRB(12, 16, 12, 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(6, 4, 6, 12),
            child: Row(
              children: [
                Container(
                  width: 28,
                  height: 28,
                  decoration: BoxDecoration(
                    color: c.accent,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Icon(
                    Icons.swap_vert_rounded,
                    size: 18,
                    color: c.onAccent,
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'LocalDrop',
                        style: t.titleMedium?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      Text(
                        '${s.group?.name} · ${s.group?.fingerprint}',
                        style: t.bodySmall,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          Expanded(
            child: ListView(
              children: [
                const SectionLabel('Devices'),
                if (online.isEmpty)
                  _SideItem(
                    selected: place == const LookingPlace(),
                    onTap: () => onGo(const LookingPlace()),
                    lead: SizedBox(
                      width: 28,
                      height: 28,
                      child: s.searchingLabel != null
                          ? Padding(
                              padding: const EdgeInsets.all(6),
                              child: CircularProgressIndicator(
                                strokeWidth: 2,
                                color: c.mute,
                              ),
                            )
                          : Icon(
                              Icons.portable_wifi_off,
                              size: 20,
                              color: c.faint,
                            ),
                    ),
                    title: s.searchingLabel != null
                        ? 'Searching on ${s.searchingLabel}…'
                        : 'Not searching',
                    sub: 'No devices yet',
                  ),
                ...online.map(device),
                if (offline.isNotEmpty) ...[
                  const SectionLabel('Known · offline'),
                  ...offline.map(device),
                ],
              ],
            ),
          ),
          _SideItem(
            selected: place == const SectionPlace(Section.transfers),
            lead: Icon(Icons.swap_vert, color: c.mute),
            title: 'Transfers',
            trailing: s.runningCount > 0
                ? StateChip('${s.runningCount}', tone: ChipTone.accent)
                : null,
            onTap: () => onGo(const SectionPlace(Section.transfers)),
          ),
          _SideItem(
            selected: place == const SectionPlace(Section.settings),
            lead: Icon(Icons.settings_outlined, color: c.mute),
            title: 'Settings',
            onTap: () => onGo(const SectionPlace(Section.settings)),
          ),
        ],
      ),
    );
  }
}

class _SideItem extends StatelessWidget {
  const _SideItem({
    required this.selected,
    required this.lead,
    required this.title,
    this.sub,
    this.trailing,
    this.onTap,
  });

  final bool selected;
  final Widget lead;
  final String title;
  final String? sub;
  final Widget? trailing;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    return Padding(
      padding: const EdgeInsets.only(bottom: 2),
      child: Material(
        color: selected ? c.surface2 : Colors.transparent,
        borderRadius: BorderRadius.circular(10),
        child: InkWell(
          borderRadius: BorderRadius.circular(10),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
            child: Row(
              children: [
                lead,
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        title,
                        style: t.bodyMedium?.copyWith(
                          fontWeight: FontWeight.w600,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                      if (sub != null && sub!.isNotEmpty)
                        Text(
                          sub!,
                          style: t.bodySmall,
                          overflow: TextOverflow.ellipsis,
                        ),
                    ],
                  ),
                ),
                ?trailing,
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Desktop top bar: where you are, and the discovery pill with the network menu.
class _Toolbar extends StatelessWidget {
  const _Toolbar({required this.title, required this.onNetworks});
  final String title;
  final VoidCallback onNetworks;

  @override
  Widget build(BuildContext context) {
    final c = context.ld;
    return Container(
      height: 56,
      padding: const EdgeInsets.symmetric(horizontal: 16),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: c.line)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(
              title,
              style: Theme.of(context).textTheme.titleMedium,
              overflow: TextOverflow.ellipsis,
            ),
          ),
          DiscoveryPill(onOpen: onNetworks),
        ],
      ),
    );
  }
}

/// "LocalDrop only searches on networks you trust. Trust “Office-5G”?" while another
/// trusted network keeps discovery running; otherwise the ask takes the main area.
class _NetworkBanner extends StatelessWidget {
  const _NetworkBanner();

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final net = s.untrustedNetwork;
    if (net == null || s.askProminently) return const SizedBox.shrink();
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    return Container(
      margin: const EdgeInsets.fromLTRB(16, 12, 16, 0),
      padding: const EdgeInsets.fromLTRB(16, 12, 8, 12),
      decoration: BoxDecoration(
        color: c.accentSoft,
        borderRadius: BorderRadius.circular(LdSize.cardRadius),
      ),
      child: Wrap(
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: 8,
        runSpacing: 4,
        children: [
          Icon(Icons.wifi, color: c.accent, size: 20),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 420),
            child: Text.rich(
              TextSpan(
                children: [
                  TextSpan(
                    text: 'Trust “${net.label}”? ',
                    style: t.bodyMedium?.copyWith(fontWeight: FontWeight.w600),
                  ),
                  TextSpan(
                    text: 'LocalDrop only searches on networks you trust.',
                    style: t.bodyMedium,
                  ),
                ],
              ),
            ),
          ),
          TextButton(
            onPressed: () => s.answerNetwork(trust: false, neverAsk: true),
            child: const Text('Never ask'),
          ),
          TextButton(
            onPressed: () => s.answerNetwork(trust: false),
            child: const Text('Not now'),
          ),
          FilledButton(
            onPressed: () => s.answerNetwork(trust: true),
            style: FilledButton.styleFrom(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
            ),
            child: const Text('Trust this network'),
          ),
        ],
      ),
    );
  }
}
