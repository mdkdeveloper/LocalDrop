import 'dart:io' show Platform;
import 'dart:math';

import 'package:material_ui/material_ui.dart';

import '../app.dart';
import '../theme/theme.dart';
import 'dialogs.dart';

/// First run: join or create the group (wireframes `join` / `d-join`).
class JoinScreen extends StatefulWidget {
  const JoinScreen({super.key});

  @override
  State<JoinScreen> createState() => _JoinScreenState();
}

/// Minimum password length (docs/security.md#threat-model).
const minPasswordLength = 10;

String? passwordProblem(String pw) {
  if (pw.isEmpty) return 'Enter the group password.';
  if (pw.length < minPasswordLength) {
    return 'Too short: ${pw.length} characters. Use at least $minPasswordLength, '
        'or generate a passphrase.';
  }
  return null;
}

/// Four random words, e.g. "maple-river-quiet-lantern".
String generatePassphrase([Random? rng]) {
  const words = [
    'amber',
    'anchor',
    'apple',
    'arrow',
    'autumn',
    'basil',
    'beacon',
    'birch',
    'bridge',
    'canyon',
    'cedar',
    'cloud',
    'comet',
    'copper',
    'coral',
    'delta',
    'ember',
    'falcon',
    'fern',
    'forest',
    'garden',
    'glacier',
    'harbor',
    'hazel',
    'island',
    'jasper',
    'juniper',
    'lantern',
    'lemon',
    'maple',
    'meadow',
    'meteor',
    'mint',
    'nectar',
    'oasis',
    'olive',
    'orbit',
    'pebble',
    'pepper',
    'pine',
    'planet',
    'quiet',
    'raven',
    'river',
    'saffron',
    'shadow',
    'silver',
    'spruce',
    'stone',
    'summit',
    'thistle',
    'thunder',
    'tulip',
    'velvet',
    'willow',
    'winter',
    'yarrow',
    'zephyr',
  ];
  final r = rng ?? Random.secure();
  return List.generate(4, (_) => words[r.nextInt(words.length)]).join('-');
}

class _JoinScreenState extends State<JoinScreen> {
  final _group = TextEditingController(text: 'home');
  final _password = TextEditingController();
  final _name = TextEditingController(text: _defaultDeviceName());
  bool _trust = true;
  bool _busy = false;
  bool _showPassword = false;
  String? _passwordError;

  static String _defaultDeviceName() {
    try {
      return Platform.localHostname;
    } catch (_) {
      return 'My device';
    }
  }

  Future<void> _join() async {
    final problem = passwordProblem(_password.text);
    setState(() => _passwordError = problem);
    if (problem != null || _group.text.trim().isEmpty) return;
    setState(() => _busy = true);
    await AppScope.of(context).join(
      groupName: _group.text,
      password: _password.text,
      deviceName: _name.text.trim().isEmpty
          ? _defaultDeviceName()
          : _name.text.trim(),
      trustThisNetwork: _trust,
    );
  }

  @override
  void dispose() {
    _group.dispose();
    _password.dispose();
    _name.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final wide = isWide(context);
    final form = _form(context);
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: EdgeInsets.all(wide ? 40 : 20),
            child: wide
                ? Card(
                    child: IntrinsicHeight(
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          SizedBox(width: 400, child: _info(context)),
                          SizedBox(
                            width: 440,
                            child: Padding(
                              padding: const EdgeInsets.all(32),
                              child: form,
                            ),
                          ),
                        ],
                      ),
                    ),
                  )
                : ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 480),
                    child: form,
                  ),
          ),
        ),
      ),
    );
  }

  Widget _logo(BuildContext context, double size) {
    final c = context.ld;
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        color: c.accent,
        borderRadius: BorderRadius.circular(size / 3.5),
      ),
      child: Icon(Icons.swap_vert_rounded, color: c.onAccent, size: size * 0.6),
    );
  }

  Widget _info(BuildContext context) {
    final c = context.ld;
    final t = Theme.of(context).textTheme;
    Widget step(int n, String title, String sub) => Padding(
      padding: const EdgeInsets.only(top: 16),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          CircleAvatar(
            radius: 14,
            backgroundColor: c.accent,
            child: Text(
              '$n',
              style: TextStyle(
                color: c.onAccent,
                fontSize: 13,
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title, style: t.titleMedium?.copyWith(fontSize: 14)),
                const SizedBox(height: 2),
                Text(sub, style: t.bodySmall?.copyWith(fontSize: 13)),
              ],
            ),
          ),
        ],
      ),
    );
    return Container(
      color: c.bg,
      padding: const EdgeInsets.all(32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _logo(context, 48),
          const SizedBox(height: 16),
          Text(
            'Welcome to LocalDrop',
            style: t.headlineSmall?.copyWith(fontSize: 24),
          ),
          const SizedBox(height: 8),
          Text(
            'Send files between your own devices on this network. No account, no cloud.',
            style: t.bodyMedium?.copyWith(color: c.mute),
          ),
          step(
            1,
            'Same name and password everywhere',
            'The first device creates the group, the others join it the same way.',
          ),
          step(
            2,
            'Compare the fingerprint',
            'The 4+4 code must match on every device.',
          ),
          step(
            3,
            'Trust your home network',
            'Your devices find each other only on networks you trust. Other devices on the '
                'network can’t see them.',
          ),
        ],
      ),
    );
  }

  Widget _form(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final c = context.ld;
    final wide = isWide(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (!wide) ...[
          Align(alignment: Alignment.centerLeft, child: _logo(context, 56)),
          const SizedBox(height: 16),
        ],
        Text(
          'Join or create your group',
          style: wide ? t.titleLarge : t.headlineSmall?.copyWith(fontSize: 26),
        ),
        if (!wide) ...[
          const SizedBox(height: 8),
          Text(
            'Use the same group name and password on all your devices; the first one creates '
            'the group. Nobody else on the network can see them.',
            style: t.bodyMedium?.copyWith(color: c.mute, fontSize: 15),
          ),
        ],
        const SizedBox(height: 20),
        TextField(
          controller: _group,
          decoration: const InputDecoration(labelText: 'Group name'),
          textInputAction: TextInputAction.next,
        ),
        const SizedBox(height: 16),
        TextField(
          key: const Key('password'),
          controller: _password,
          obscureText: !_showPassword,
          onChanged: (_) {
            if (_passwordError != null) {
              setState(() => _passwordError = passwordProblem(_password.text));
            }
          },
          decoration: InputDecoration(
            labelText: 'Password',
            helperText:
                'At least $minPasswordLength characters, or use a generated passphrase.',
            errorText: _passwordError,
            suffixIcon: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                IconButton(
                  tooltip: _showPassword ? 'Hide' : 'Show',
                  icon: Icon(
                    _showPassword
                        ? Icons.visibility_off_outlined
                        : Icons.visibility_outlined,
                  ),
                  onPressed: () =>
                      setState(() => _showPassword = !_showPassword),
                ),
                TextButton(
                  onPressed: () => setState(() {
                    _password.text = generatePassphrase();
                    _showPassword = true;
                    _passwordError = null;
                  }),
                  child: const Text('Generate'),
                ),
                const SizedBox(width: 4),
              ],
            ),
          ),
        ),
        const SizedBox(height: 16),
        TextField(
          controller: _name,
          decoration: const InputDecoration(labelText: 'This device’s name'),
        ),
        const SizedBox(height: 12),
        CheckboxListTile(
          value: _trust,
          onChanged: (v) => setState(() => _trust = v ?? true),
          contentPadding: EdgeInsets.zero,
          controlAffinity: ListTileControlAffinity.leading,
          title: Text(
            'Trust this network',
            style: t.bodyMedium?.copyWith(fontWeight: FontWeight.w500),
          ),
          subtitle: Text(
            'LocalDrop only searches on networks you trust.',
            style: t.bodySmall,
          ),
        ),
        const SizedBox(height: 16),
        FilledButton(
          key: const Key('join'),
          onPressed: _busy ? null : _join,
          child: _busy
              ? SizedBox(
                  height: 18,
                  width: 18,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    color: c.onAccent,
                  ),
                )
              : const Text('Join group'),
        ),
      ],
    );
  }
}
