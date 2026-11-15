import 'package:material_ui/material_ui.dart';

import 'state/app_state.dart';
import 'theme/theme.dart';
import 'ui/join_screen.dart';
import 'ui/shell.dart';

/// Makes [AppState] available to every widget and rebuilds dependents on change.
class AppScope extends InheritedNotifier<AppState> {
  const AppScope({super.key, required AppState state, required super.child})
    : super(notifier: state);

  static AppState of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<AppScope>()!.notifier!;
}

class LocalDropApp extends StatelessWidget {
  const LocalDropApp({
    super.key,
    required this.state,
    this.themeMode = ThemeMode.system,
  });

  final AppState state;
  final ThemeMode themeMode;

  @override
  Widget build(BuildContext context) {
    return AppScope(
      state: state,
      child: MaterialApp(
        title: 'LocalDrop',
        debugShowCheckedModeBanner: false,
        theme: buildTheme(Brightness.light),
        darkTheme: buildTheme(Brightness.dark),
        themeMode: themeMode,
        home: Builder(
          builder: (context) =>
              AppScope.of(context).joined ? const Shell() : const JoinScreen(),
        ),
      ),
    );
  }
}
