import 'package:material_ui/material_ui.dart';

import 'tokens.g.dart';

export 'tokens.g.dart';

/// Gives widgets the design tokens: `context.ld.accent`.
class LdTheme extends ThemeExtension<LdTheme> {
  const LdTheme(this.colors);

  final LdColors colors;

  @override
  LdTheme copyWith({LdColors? colors}) => LdTheme(colors ?? this.colors);

  @override
  LdTheme lerp(LdTheme? other, double t) => t < 0.5 ? this : (other ?? this);
}

extension LdContext on BuildContext {
  LdColors get ld => Theme.of(this).extension<LdTheme>()!.colors;
}

/// Radii and spacing used across screens (from the wireframes).
abstract final class LdSize {
  static const double cardRadius = 14;
  static const double fieldRadius = 10;
  static const double gutter = 16;
  static const double sidebarWidth = 264;

  /// Below this width the phone layout (bottom navigation) is used.
  static const double desktopBreakpoint = 840;
}

ThemeData buildTheme(Brightness brightness) {
  final c = brightness == Brightness.light ? LdColors.light : LdColors.dark;
  final scheme = ColorScheme(
    brightness: brightness,
    primary: c.accent,
    onPrimary: c.onAccent,
    primaryContainer: c.accentSoft,
    onPrimaryContainer: c.ink,
    secondary: c.accent,
    onSecondary: c.onAccent,
    surface: c.surface,
    onSurface: c.ink,
    onSurfaceVariant: c.mute,
    surfaceContainerHighest: c.surface2,
    outline: c.line,
    outlineVariant: c.line,
    error: c.danger,
    onError: c.onAccent,
  );
  final base = ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    scaffoldBackgroundColor: c.bg,
    dividerColor: c.line,
    extensions: [LdTheme(c)],
  );
  final shape = RoundedRectangleBorder(
    borderRadius: BorderRadius.circular(LdSize.fieldRadius),
  );
  final text = base.textTheme.apply(bodyColor: c.ink, displayColor: c.ink);
  return base.copyWith(
    textTheme: text.copyWith(
      headlineSmall: text.headlineSmall?.copyWith(
        fontSize: 22,
        fontWeight: FontWeight.w700,
      ),
      titleLarge: text.titleLarge?.copyWith(
        fontSize: 20,
        fontWeight: FontWeight.w700,
      ),
      titleMedium: text.titleMedium?.copyWith(
        fontSize: 15,
        fontWeight: FontWeight.w600,
      ),
      bodyMedium: text.bodyMedium?.copyWith(fontSize: 14),
      bodySmall: text.bodySmall?.copyWith(fontSize: 12, color: c.mute),
      labelSmall: text.labelSmall?.copyWith(
        fontSize: 11,
        fontWeight: FontWeight.w600,
        letterSpacing: 0.6,
        color: c.mute,
      ),
    ),
    cardTheme: CardThemeData(
      color: c.surface,
      elevation: 0,
      margin: EdgeInsets.zero,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(LdSize.cardRadius),
      ),
      clipBehavior: Clip.antiAlias,
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        backgroundColor: c.accent,
        foregroundColor: c.onAccent,
        shape: shape,
        padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 14),
        textStyle: const TextStyle(fontWeight: FontWeight.w600),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: c.ink,
        side: BorderSide(color: c.line),
        shape: shape,
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      ),
    ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: c.surface,
      hintStyle: TextStyle(color: c.faint),
      helperStyle: TextStyle(color: c.mute, fontSize: 12),
      helperMaxLines: 3,
      errorMaxLines: 3,
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(LdSize.fieldRadius),
        borderSide: BorderSide(color: c.line),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(LdSize.fieldRadius),
        borderSide: BorderSide(color: c.line),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(LdSize.fieldRadius),
        borderSide: BorderSide(color: c.accent, width: 1.5),
      ),
    ),
    navigationBarTheme: NavigationBarThemeData(
      backgroundColor: c.surface,
      indicatorColor: c.accentSoft,
      surfaceTintColor: Colors.transparent,
      labelTextStyle: WidgetStatePropertyAll(
        TextStyle(fontSize: 11, fontWeight: FontWeight.w600, color: c.ink),
      ),
    ),
    dividerTheme: DividerThemeData(color: c.line, thickness: 1, space: 1),
    progressIndicatorTheme: ProgressIndicatorThemeData(
      color: c.accent,
      linearTrackColor: c.surface2,
    ),
    dialogTheme: DialogThemeData(
      backgroundColor: c.surface,
      surfaceTintColor: Colors.transparent,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),
    ),
    bottomSheetTheme: BottomSheetThemeData(
      backgroundColor: c.surface,
      surfaceTintColor: Colors.transparent,
      showDragHandle: true,
    ),
    checkboxTheme: CheckboxThemeData(
      fillColor: WidgetStateProperty.resolveWith(
        (s) => s.contains(WidgetState.selected) ? c.accent : Colors.transparent,
      ),
      side: BorderSide(color: c.faint, width: 1.5),
    ),
  );
}
