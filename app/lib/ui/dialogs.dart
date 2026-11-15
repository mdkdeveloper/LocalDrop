import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../node/models.dart';
import '../state/app_state.dart';
import '../theme/theme.dart';
import 'format.dart';

bool isWide(BuildContext context) =>
    MediaQuery.sizeOf(context).width >= LdSize.desktopBreakpoint;

/// Opens the dialog (desktop) or sheet (phone) for a pending decision.
Future<void> showDecision(BuildContext context, PendingDecision d) =>
    switch (d) {
      RequestDecision(:final request) => _present(
        context,
        _RequestBody(request),
      ),
      ConflictDecision(:final conflict) => _present(
        context,
        _ConflictBody(conflict),
      ),
    };

Future<void> _present(BuildContext context, Widget body) {
  if (isWide(context)) {
    return showDialog(
      context: context,
      builder: (_) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 460),
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
        padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
        child: body,
      ),
    ),
  );
}

/// Incoming request (accept policy `Ask`): Decline / Accept, "Always accept".
class _RequestBody extends StatefulWidget {
  const _RequestBody(this.request);
  final IncomingRequest request;

  @override
  State<_RequestBody> createState() => _RequestBodyState();
}

class _RequestBodyState extends State<_RequestBody> {
  bool always = false;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final r = widget.request;
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    void answer(bool accept) {
      s.answerRequest(r.id, accept: accept, always: always);
      Navigator.of(context).pop();
    }

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          '${s.nameOf(r.from)} wants to send you files',
          style: t.titleLarge,
        ),
        const SizedBox(height: 16),
        Container(
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
            color: c.bg,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Row(
            children: [
              Icon(Icons.folder_outlined, color: c.accent),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(r.name, style: t.titleMedium),
                    Text(
                      '${r.files} files · ${formatBytes(r.bytes)} → Inbox',
                      style: t.bodySmall,
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        CheckboxListTile(
          value: always,
          onChanged: (v) => setState(() => always = v ?? false),
          contentPadding: EdgeInsets.zero,
          controlAffinity: ListTileControlAffinity.leading,
          title: Text(
            'Always accept from ${s.nameOf(r.from)}',
            style: t.bodyMedium,
          ),
        ),
        Text(
          'Without an answer the request is declined after 10 minutes.',
          style: t.bodySmall,
        ),
        const SizedBox(height: 20),
        Row(
          children: [
            Expanded(
              child: OutlinedButton(
                onPressed: () => answer(false),
                child: const Text('Decline'),
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: FilledButton(
                onPressed: () => answer(true),
                child: const Text('Accept'),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

/// Conflict dialog: the allowed modes; the first answer on either side wins.
class _ConflictBody extends StatefulWidget {
  const _ConflictBody(this.conflict);
  final ConflictAsked conflict;

  @override
  State<_ConflictBody> createState() => _ConflictBodyState();
}

class _ConflictBodyState extends State<_ConflictBody> {
  late ConflictMode mode = widget.conflict.allowedModes.first;

  @override
  Widget build(BuildContext context) {
    final s = AppScope.of(context);
    final k = widget.conflict;
    final job = s.jobs[k.id];
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    void answer(ConflictMode? m) {
      s.answerConflict(k.id, m);
      Navigator.of(context).pop();
    }

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text('${k.conflicts} files already exist', style: t.titleLarge),
        const SizedBox(height: 6),
        Text(
          '${job?.name ?? 'This job'} from ${s.nameOf(job?.peer ?? '')}. '
          'Other files are added either way.',
          style: t.bodyMedium?.copyWith(color: c.mute),
        ),
        const SizedBox(height: 16),
        RadioGroup<ConflictMode>(
          groupValue: mode,
          onChanged: (m) => setState(() => mode = m ?? mode),
          child: Column(
            children: [
              for (final m in k.allowedModes)
                Padding(
                  padding: const EdgeInsets.only(bottom: 8),
                  child: Material(
                    color: m == mode ? c.accentSoft : c.bg,
                    borderRadius: BorderRadius.circular(12),
                    child: RadioListTile<ConflictMode>(
                      value: m,
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(12),
                      ),
                      title: Text(modeLabel(m), style: t.titleMedium),
                      subtitle: Text(modeHint(m), style: t.bodySmall),
                    ),
                  ),
                ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        Row(
          children: [
            Expanded(
              child: OutlinedButton(
                onPressed: () => answer(null),
                child: const Text('Cancel job'),
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: FilledButton(
                onPressed: () => answer(mode),
                child: const Text('Continue'),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

/// The job's ⋮ menu: a bottom sheet with name and progress on phone, a dialog list on
/// desktop. Only actions that fit the state; destructive last, in red.
Future<void> showJobMenu(BuildContext context, JobView job) {
  final s = AppScope.of(context);
  final c = context.ld;
  final actions = actionsFor(job);
  Widget body(BuildContext ctx) {
    final t = Theme.of(ctx).textTheme;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 0, 20, 8),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(job.name, style: t.titleMedium),
              Text(
                '${job.direction == Direction.outgoing ? 'To' : 'From'} ${s.nameOf(job.peer)} · '
                '${stateLabel(job, peerName: s.nameOf(job.peer))}',
                style: t.bodySmall,
              ),
            ],
          ),
        ),
        const Divider(),
        for (final a in actions)
          ListTile(
            title: Text(
              actionLabel(a, job),
              style: TextStyle(
                color: isDestructive(a) ? c.danger : c.ink,
                fontWeight: FontWeight.w500,
              ),
            ),
            onTap: () {
              Navigator.of(ctx).pop();
              s.jobAction(job.id, a);
            },
          ),
        const SizedBox(height: 8),
      ],
    );
  }

  if (isWide(context)) {
    return showDialog(
      context: context,
      builder: (ctx) => Dialog(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 360),
          child: Padding(
            padding: const EdgeInsets.only(top: 20),
            child: body(ctx),
          ),
        ),
      ),
    );
  }
  return showModalBottomSheet(
    context: context,
    builder: (ctx) => SafeArea(child: body(ctx)),
  );
}
