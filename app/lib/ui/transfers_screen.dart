import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../node/models.dart';
import '../theme/theme.dart';
import 'dialogs.dart';
import 'format.dart';
import 'widgets.dart';

enum _Filter { all, outgoing, incoming }

/// Transfers: one queue per device and direction (wireframes `transfers` / `d-transfers`).
class TransfersScreen extends StatefulWidget {
  const TransfersScreen({super.key});

  @override
  State<TransfersScreen> createState() => _TransfersScreenState();
}

class _TransfersScreenState extends State<TransfersScreen> {
  _Filter _filter = _Filter.all;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    final wide = isWide(context);

    final out = s.activeJobs(Direction.outgoing);
    final inc = s.activeJobs(Direction.incoming);
    final finished = s.finishedJobs;
    final empty = out.isEmpty && inc.isEmpty && finished.isEmpty;

    final header = Padding(
      padding: EdgeInsets.fromLTRB(
        wide ? 28 : 20,
        wide ? 24 : 12,
        wide ? 28 : 20,
        8,
      ),
      child: Wrap(
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: 12,
        runSpacing: 12,
        children: [
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text('Transfers', style: t.headlineSmall),
              Text(
                '${s.runningCount} active · ${s.waitingCount} waiting',
                style: t.bodySmall?.copyWith(fontSize: 13),
              ),
            ],
          ),
          const DiscoveryPill(),
          SegmentedButton<_Filter>(
            segments: const [
              ButtonSegment(value: _Filter.all, label: Text('All')),
              ButtonSegment(value: _Filter.outgoing, label: Text('Outgoing')),
              ButtonSegment(value: _Filter.incoming, label: Text('Incoming')),
            ],
            selected: {_filter},
            showSelectedIcon: false,
            onSelectionChanged: (v) => setState(() => _filter = v.first),
          ),
        ],
      ),
    );

    if (empty) {
      return Column(
        children: [
          header,
          const Expanded(
            child: EmptyState(
              icon: Icons.swap_vert,
              title: 'Nothing is being sent',
              body:
                  'Drag files onto a device, or use Share on your phone. Files for devices '
                  'that are offline wait here and go out when the device is back.',
            ),
          ),
        ],
      );
    }

    List<Widget> queues(Direction dir, List<JobView> jobs) {
      final byPeer = <String, List<JobView>>{};
      for (final j in jobs) {
        byPeer.putIfAbsent(j.peer, () => []).add(j);
      }
      return [
        for (final MapEntry(key: peer, value: list) in byPeer.entries) ...[
          _LowSpaceBanners(list),
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: GroupCard(
              header: Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        '${dir == Direction.outgoing ? 'To' : 'From'} ${s.nameOf(peer)}',
                        style: t.titleMedium?.copyWith(fontSize: 14),
                      ),
                    ),
                    _PeerChip(peer),
                  ],
                ),
              ),
              children: [for (final j in list) JobTile(j)],
            ),
          ),
        ],
      ];
    }

    final showOut = _filter != _Filter.incoming;
    final showIn = _filter != _Filter.outgoing;
    final outCol = [
      const SectionLabel('Outgoing'),
      if (out.isEmpty) _none('Nothing to send'),
      ...queues(Direction.outgoing, out),
    ];
    final inCol = [
      const SectionLabel('Incoming'),
      if (inc.isEmpty) _none('Nothing coming in'),
      ...queues(Direction.incoming, inc),
    ];
    final done = finished
        .where(
          (j) =>
              _filter == _Filter.all ||
              (j.direction == Direction.outgoing) ==
                  (_filter == _Filter.outgoing),
        )
        .toList();

    return ListView(
      padding: EdgeInsets.only(bottom: 24),
      children: [
        header,
        Padding(
          padding: EdgeInsets.symmetric(horizontal: wide ? 28 : 16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (wide && showOut && showIn)
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: outCol,
                      ),
                    ),
                    const SizedBox(width: 16),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: inCol,
                      ),
                    ),
                  ],
                )
              else ...[
                if (showOut) ...outCol,
                if (showIn) ...inCol,
              ],
              if (done.isNotEmpty) ...[
                SectionLabel(
                  'Finished',
                  trailing: TextButton(
                    onPressed: () {
                      for (final j in done) {
                        s.jobAction(j.id, JobAction.clear);
                      }
                    },
                    child: Text(
                      'Clear finished',
                      style: TextStyle(color: c.mute, fontSize: 12),
                    ),
                  ),
                ),
                GroupCard(
                  children: [
                    for (final j in done)
                      ListTile(
                        leading: Icon(
                          Icons.check_circle_outline,
                          color: j.state == JobState.completed
                              ? c.success
                              : c.faint,
                        ),
                        title: Text(
                          j.name,
                          style: t.bodyMedium?.copyWith(
                            fontWeight: FontWeight.w500,
                          ),
                        ),
                        subtitle: Text(
                          '${j.direction == Direction.outgoing ? 'To' : 'From'} ${s.nameOf(j.peer)} · '
                          '${formatBytes(j.bytesTotal)}',
                          style: t.bodySmall,
                        ),
                        trailing: StateChip(
                          shortState(j.state),
                          tone: toneFor(j.state),
                        ),
                        onTap: () => showJobMenu(context, j),
                      ),
                  ],
                ),
              ],
            ],
          ),
        ),
      ],
    );
  }

  Widget _none(String text) => Padding(
    padding: const EdgeInsets.fromLTRB(4, 0, 4, 12),
    child: Text(text, style: Theme.of(context).textTheme.bodySmall),
  );
}

class _PeerChip extends StatelessWidget {
  const _PeerChip(this.peer);
  final String peer;

  @override
  Widget build(BuildContext context) {
    final d = AppScope.of(context).devices[peer];
    final online = d != null && d.presence == Presence.online;
    return StateChip(
      online ? 'Online' : 'Offline · waiting',
      tone: online ? ChipTone.success : ChipTone.neutral,
    );
  }
}

/// "Work laptop is running out of space: 1.2 GB free, 3.4 GB needed".
class _LowSpaceBanners extends StatelessWidget {
  const _LowSpaceBanners(this.jobs);
  final List<JobView> jobs;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final c = context.ld;
    final banners = [
      for (final j in jobs)
        if (s.lowSpace[j.id] case final ls?)
          Container(
            margin: const EdgeInsets.only(bottom: 8),
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: c.surface,
              borderRadius: BorderRadius.circular(LdSize.cardRadius),
              border: Border.all(color: c.warning),
            ),
            child: Row(
              children: [
                Icon(Icons.storage_outlined, color: c.warning),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    '${s.nameOf(j.peer)} is running out of space: ${formatBytes(ls.free)} free, '
                    '${formatBytes(ls.needed)} needed. The job continues on its own when space is free.',
                    style: Theme.of(context).textTheme.bodySmall
                        ?.copyWith(color: c.ink, fontSize: 13),
                  ),
                ),
              ],
            ),
          ),
    ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: banners,
    );
  }
}
