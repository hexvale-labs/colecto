import 'package:cupertino_ui/cupertino_ui.dart';
import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/preferences_models.dart';

/// The design system a screen is drawn with. Each platform gets its native
/// one (`AGENTS.md` rule 4), sharing Colecto's palette and layouts:
/// - [desktop]: Colecto's shadcn-style system, from Colecto (Linux and Web).
/// - [fluent]: Fluent UI (Windows).
/// - [macos]: `macos_ui` (macOS).
/// - [android]: Material 3.
/// - [ios]: Cupertino.
enum ColectoPlatformStyle {
  desktop('colecto', 'shadcn (Linux & Web)'),
  fluent('fluent', 'Fluent (Windows)'),
  macos('macos', 'macOS'),
  android('material', 'Material 3 (Android)'),
  ios('cupertino', 'Cupertino (iOS)');

  const ColectoPlatformStyle(this.id, this.label);

  /// The value accepted by `--dart-define=COLECTO_DESIGN=<id>`.
  final String id;
  final String label;

  /// Phones and tablets: 44 px targets, swipe actions, pushed pages.
  bool get touch => this == android || this == ios;

  /// Pointer-first desktop designs.
  bool get pointer => !touch;

  /// The platform Flutter should emulate (scroll physics, text editing
  /// shortcuts, page transitions), or null to keep the real one.
  TargetPlatform? get platform => switch (this) {
    desktop => null,
    fluent => TargetPlatform.windows,
    macos => TargetPlatform.macOS,
    android => TargetPlatform.android,
    ios => TargetPlatform.iOS,
  };

  static ColectoPlatformStyle? parse(String? value) =>
      switch (value?.trim().toLowerCase()) {
        'colecto' || 'shadcn' || 'linux' || 'web' || 'desktop' => desktop,
        'fluent' || 'windows' => fluent,
        'macos' || 'mac' || 'appkit' => macos,
        'material' || 'android' || 'm3' => android,
        'cupertino' || 'ios' => ios,
        _ => null,
      };

  /// The native design for where the app is running.
  static ColectoPlatformStyle native() {
    if (kIsWeb) return desktop;
    return switch (defaultTargetPlatform) {
      TargetPlatform.android => android,
      TargetPlatform.iOS => ios,
      TargetPlatform.windows => fluent,
      TargetPlatform.macOS => macos,
      _ => desktop,
    };
  }
}

/// `--dart-define=COLECTO_DESIGN=colecto|fluent|macos|material|cupertino`
/// previews another platform's design on any machine.
const String colectoDesignDefine = String.fromEnvironment('COLECTO_DESIGN');

/// Carries the active design through the theme.
@immutable
class ColectoDesign extends ThemeExtension<ColectoDesign> {
  const ColectoDesign(this.style);

  final ColectoPlatformStyle style;

  @override
  ColectoDesign copyWith({ColectoPlatformStyle? style}) =>
      ColectoDesign(style ?? this.style);

  @override
  ColectoDesign lerp(ThemeExtension<ColectoDesign>? other, double t) =>
      t < 0.5 || other is! ColectoDesign ? this : other;
}

ColectoPlatformStyle colectoPlatformStyle(BuildContext context) =>
    Theme.of(context).extension<ColectoDesign>()?.style ??
    ColectoPlatformStyle.native();

/// Control corner radius for the active design.
double colectoRadius(BuildContext context) =>
    switch (colectoPlatformStyle(context)) {
      ColectoPlatformStyle.fluent => 4,
      ColectoPlatformStyle.macos => 6,
      _ => 8,
    };

/// Motion tokens from Colecto's `DESIGN.md` ("Motion"): presses scale to
/// 0.97, popovers grow from their trigger in ~200 ms with a strong ease-out
/// and leave in 120 ms, cards rise 6 px in 220 ms.
abstract final class ColectoMotion {
  /// `cubic-bezier(0.23, 1, 0.32, 1)`.
  static const Curve easeOut = Cubic(0.23, 1, 0.32, 1);
  static const Duration hover = Duration(milliseconds: 140);
  static const Duration press = Duration(milliseconds: 120);
  static const Duration popIn = Duration(milliseconds: 200);
  static const Duration popOut = Duration(milliseconds: 120);
  static const Duration card = Duration(milliseconds: 220);
  static const double pressScale = 0.97;

  /// `prefers-reduced-motion` removes every transition.
  static Duration of(BuildContext context, Duration d) =>
      MediaQuery.maybeDisableAnimationsOf(context) ?? false ? Duration.zero : d;
}

/// Colecto's surface and accent tokens (`docs/ui-design.md`): near-black and
/// neutral with a breath of green; the accent carries the colour, and each
/// colour has one job.
@immutable
class ColectoPalette extends ThemeExtension<ColectoPalette> {
  const ColectoPalette({
    required this.brightness,
    required this.ground,
    required this.sunk,
    required this.bg,
    required this.float,
    required this.raise,
    required this.control,
    required this.controlHover,
    required this.line,
    required this.lineSoft,
    required this.ink,
    required this.ink2,
    required this.ink3,
    required this.bodyInk,
    required this.accent,
    required this.accentHi,
    required this.accentInk,
    required this.select,
    required this.statusOnline,
    required this.danger,
    required this.dangerBg,
    required this.dangerInk,
    required this.star,
    required this.scrim,
    required this.shadow,
  });

  final Brightness brightness;

  /// Top bar and window ground.
  final Color ground;

  /// Inputs and inset notes.
  final Color sunk;

  /// Panes and Settings.
  final Color bg;

  /// Menus, the palette, dialogs and toasts.
  final Color float;

  /// Hover and active nav items.
  final Color raise;

  /// Secondary buttons and segmented selections.
  final Color control;
  final Color controlHover;
  final Color line;
  final Color lineSoft;
  final Color ink;
  final Color ink2;
  final Color ink3;

  /// Message body text: a touch softer than [ink] for long reading.
  final Color bodyInk;

  /// The owner-chosen accent (brand mint by default).
  final Color accent;
  final Color accentHi;
  final Color accentInk;

  /// Selected rows, active chips and notices.
  final Color select;
  final Color statusOnline;
  final Color danger;
  final Color dangerBg;
  final Color dangerInk;
  final Color star;
  final Color scrim;
  final Color shadow;

  bool get isDark => brightness == Brightness.dark;

  static const Color defaultDarkAccent = Color(0xFFA2D6B1);
  static const Color defaultLightAccent = Color(0xFF1F6E43);
  static const String defaultAccentHex = '#a2d6b1';

  static Color? parseHexColor(String? hex) {
    if (hex == null) return null;
    final clean = hex.trim();
    if (!RegExp(r'^#[0-9a-fA-F]{6}$').hasMatch(clean)) return null;
    return Color(int.parse('FF${clean.substring(1)}', radix: 16));
  }

  static String toHex(Color c) =>
      '#${(c.toARGB32() & 0xFFFFFF).toRadixString(16).padLeft(6, '0')}';

  static ColectoPalette dark({String? accentHex}) {
    final accent = parseHexColor(accentHex) ?? defaultDarkAccent;
    const bg = Color(0xFF121514); // oklch(0.148 0.003 165)
    return ColectoPalette(
      brightness: Brightness.dark,
      ground: const Color(0xFF0A0C0B), // oklch(0.112 0.002 165)
      sunk: const Color(0xFF0D100F), // oklch(0.125 0.003 165)
      bg: bg,
      float: const Color(0xFF181C1A), // oklch(0.185 0.004 165)
      raise: const Color(0xFF1E2220), // oklch(0.207 0.004 165)
      control: const Color(0xFF242927), // oklch(0.235 0.005 165)
      controlHover: const Color(0xFF2E3431), // oklch(0.29 0.011 165)
      line: const Color(0xFF2C322F), // oklch(0.265 0.005 165)
      lineSoft: const Color(0xFF1F2422), // oklch(0.212 0.004 165)
      ink: const Color(0xFFF1F4F2), // oklch(0.95 0.003 155)
      ink2: const Color(0xFFC1C7C3), // oklch(0.79 0.006 155)
      ink3: const Color(0xFF8C948F), // oklch(0.63 0.008 158)
      bodyInk: const Color(0xFFDCE3DE), // oklch(0.9 0.01 155)
      accent: accent,
      // `color-mix(in oklch, var(--mint) 55%, white)`
      accentHi: Color.lerp(accent, const Color(0xFFFFFFFF), 0.45)!,
      // `color-mix(in oklch, var(--mint) 14%, black)`
      accentInk: Color.lerp(const Color(0xFF000000), accent, 0.14)!,
      // `color-mix(in oklch, var(--mint) 13%, var(--bg))`
      select: Color.lerp(bg, accent, 0.13)!,
      statusOnline: accent,
      danger: const Color(0xFFE58A7E), // oklch(0.8 0.1 30)
      dangerBg: const Color(0xFF331B19), // oklch(0.245 0.045 28)
      dangerInk: const Color(0xFFF2D3CD), // oklch(0.9 0.04 30)
      star: const Color(0xFFE6C665), // oklch(0.85 0.12 85)
      scrim: const Color(0x9E070908), // oklch(0.08 0.004 165 / 0.62)
      shadow: const Color(0xCC000000),
    );
  }

  static ColectoPalette light({String? accentHex}) {
    final picked = parseHexColor(accentHex);
    // Light, low-chroma presets read on the dark ground; on white they need
    // depth for WCAG AA, so darken a light accent rather than use it raw.
    final accent = picked == null
        ? defaultLightAccent
        : (picked.computeLuminance() > 0.3
              ? Color.lerp(picked, const Color(0xFF0B2414), 0.62)!
              : picked);
    const bg = Color(0xFFFFFFFF);
    return ColectoPalette(
      brightness: Brightness.light,
      ground: const Color(0xFFF2F5F3),
      sunk: const Color(0xFFF5F8F6),
      bg: bg,
      float: const Color(0xFFFFFFFF),
      raise: const Color(0xFFEFF3F1),
      control: const Color(0xFFE6ECE9),
      controlHover: const Color(0xFFDCE4E0),
      line: const Color(0xFFD5DDD8),
      lineSoft: const Color(0xFFE6ECE8),
      ink: const Color(0xFF121815),
      ink2: const Color(0xFF3D4842),
      ink3: const Color(0xFF65716A),
      bodyInk: const Color(0xFF1C2420),
      accent: accent,
      accentHi: Color.lerp(accent, const Color(0xFF000000), 0.25)!,
      accentInk: const Color(0xFFFFFFFF),
      select: Color.lerp(bg, accent, 0.1)!,
      statusOnline: const Color(0xFF23864D),
      danger: const Color(0xFFB9382C),
      dangerBg: const Color(0xFFFCECEB),
      dangerInk: const Color(0xFF8A2318),
      star: const Color(0xFFB8860B),
      scrim: const Color(0x52101814),
      shadow: const Color(0x33101814),
    );
  }

  /// Material You: every surface, line and ink taken from the system's
  /// dynamic scheme (Android 12+), so Colecto matches the wallpaper.
  static ColectoPalette fromScheme(ColorScheme s) {
    final dark = s.brightness == Brightness.dark;
    return ColectoPalette(
      brightness: s.brightness,
      ground: s.surfaceContainerLowest,
      sunk: s.surfaceContainerLow,
      bg: s.surface,
      float: s.surfaceContainer,
      raise: s.surfaceContainerHigh,
      control: s.surfaceContainerHighest,
      controlHover: Color.lerp(s.surfaceContainerHighest, s.onSurface, 0.08)!,
      line: s.outlineVariant,
      lineSoft: Color.lerp(s.outlineVariant, s.surface, 0.45)!,
      ink: s.onSurface,
      ink2: s.onSurfaceVariant,
      ink3: Color.lerp(s.onSurfaceVariant, s.surface, 0.28)!,
      bodyInk: s.onSurface,
      accent: s.primary,
      accentHi: dark ? s.primaryFixedDim : s.primary,
      accentInk: s.onPrimary,
      select: s.secondaryContainer,
      statusOnline: s.primary,
      danger: s.error,
      dangerBg: s.errorContainer,
      dangerInk: s.onErrorContainer,
      star: dark ? const Color(0xFFE6C665) : const Color(0xFFB8860B),
      scrim: s.scrim.withValues(alpha: 0.4),
      shadow: s.shadow,
    );
  }

  static ColectoPalette of(BuildContext context) {
    return Theme.of(context).extension<ColectoPalette>() ??
        ColectoPalette.dark();
  }

  /// Floating surfaces (menus, the palette, dialogs) carry one elevation:
  /// a soft long shadow plus a short contact shadow.
  List<BoxShadow> get floatShadow => [
    BoxShadow(
      color: shadow.withValues(alpha: isDark ? 0.6 : 0.16),
      blurRadius: 40,
      spreadRadius: -10,
      offset: const Offset(0, 18),
    ),
    BoxShadow(
      color: shadow.withValues(alpha: isDark ? 0.25 : 0.08),
      blurRadius: 6,
      offset: const Offset(0, 2),
    ),
  ];

  /// A stable hue per correspondent (`hue()` in Colecto's `lib/initial.ts`).
  static int hueFor(String name) {
    var hash = 0;
    for (final rune in name.trim().toLowerCase().runes) {
      hash = (hash * 31 + rune) & 0xFFFFFFFF;
    }
    return hash % 360;
  }

  /// Avatar fill and ink for a correspondent hue.
  (Color, Color) avatarColors(int hue) {
    final h = hue.toDouble();
    return isDark
        ? (
            HSLColor.fromAHSL(1, h, 0.26, 0.24).toColor(),
            HSLColor.fromAHSL(1, h, 0.45, 0.86).toColor(),
          )
        : (
            HSLColor.fromAHSL(1, h, 0.45, 0.9).toColor(),
            HSLColor.fromAHSL(1, h, 0.4, 0.3).toColor(),
          );
  }

  @override
  ColectoPalette copyWith({Color? accent}) => this;

  @override
  ColectoPalette lerp(ThemeExtension<ColectoPalette>? other, double t) {
    if (other is! ColectoPalette) return this;
    Color l(Color a, Color b) => Color.lerp(a, b, t)!;
    return ColectoPalette(
      brightness: t < 0.5 ? brightness : other.brightness,
      ground: l(ground, other.ground),
      sunk: l(sunk, other.sunk),
      bg: l(bg, other.bg),
      float: l(float, other.float),
      raise: l(raise, other.raise),
      control: l(control, other.control),
      controlHover: l(controlHover, other.controlHover),
      line: l(line, other.line),
      lineSoft: l(lineSoft, other.lineSoft),
      ink: l(ink, other.ink),
      ink2: l(ink2, other.ink2),
      ink3: l(ink3, other.ink3),
      bodyInk: l(bodyInk, other.bodyInk),
      accent: l(accent, other.accent),
      accentHi: l(accentHi, other.accentHi),
      accentInk: l(accentInk, other.accentInk),
      select: l(select, other.select),
      statusOnline: l(statusOnline, other.statusOnline),
      danger: l(danger, other.danger),
      dangerBg: l(dangerBg, other.dangerBg),
      dangerInk: l(dangerInk, other.dangerInk),
      star: l(star, other.star),
      scrim: l(scrim, other.scrim),
      shadow: l(shadow, other.shadow),
    );
  }
}

/// Maps owner typography preferences (`Preferences.font`) onto a family.
/// Without a choice, Fluent uses Segoe UI Variable and macOS the system
/// font; everything else uses the bundled Mona Sans.
String? resolveFontFamily(String? pref, {ColectoPlatformStyle? style}) {
  if (pref == null || pref.isEmpty) {
    return switch (style) {
      ColectoPlatformStyle.fluent => 'Segoe UI Variable Text',
      ColectoPlatformStyle.macos => '.AppleSystemUIFont',
      _ => 'Mona Sans',
    };
  }
  return switch (pref) {
    'mona' => 'Mona Sans',
    'inter' => 'Inter',
    'geist' => 'Geist',
    'plex' => 'IBM Plex Sans',
    'atkinson' => 'Atkinson Hyperlegible Next',
    _ => pref,
  };
}

/// One text style helper so every label uses the variable font's exact
/// weight (Colecto uses 540–660, not just 400/600) and, for headings, the
/// slightly wider width instead of a second typeface.
TextStyle kStyle(
  double size, {
  double weight = 450,
  Color? color,
  double? width,
  double? height,
  double? letterSpacing,
  List<FontFeature>? features,
}) {
  return TextStyle(
    fontSize: size,
    fontWeight: FontWeight.values[((weight / 100).round() - 1).clamp(0, 8)],
    fontVariations: [
      FontVariation.weight(weight),
      if (width != null) FontVariation.width(width),
    ],
    color: color,
    height: height,
    letterSpacing: letterSpacing,
    fontFeatures: features,
  );
}

const tabularFigures = [FontFeature.tabularFigures()];

/// Applies WCAG 1.4.12 text spacing (`Preferences.text_spacing`).
TextStyle applyTextSpacing(
  TextStyle base,
  TextSpacing spacing, {
  double? baseSize,
}) {
  final fontSize = baseSize ?? base.fontSize ?? 14.0;
  final letterSpacing = switch (spacing.letter) {
    1 => fontSize * 0.06,
    2 => fontSize * 0.12,
    _ => base.letterSpacing,
  };
  final wordSpacing = switch (spacing.word) {
    1 => fontSize * 0.08,
    2 => fontSize * 0.16,
    _ => base.wordSpacing,
  };
  final height = switch (spacing.line) {
    1 => 1.6,
    2 => 1.85,
    _ => base.height ?? 1.45,
  };
  return base.copyWith(
    fontSize: fontSize,
    letterSpacing: letterSpacing,
    wordSpacing: wordSpacing,
    height: height,
  );
}

ThemeData buildColectoTheme({
  required Brightness brightness,
  required Preferences preferences,
  required ColectoPlatformStyle style,
  ColorScheme? dynamicScheme,
}) {
  final palette = dynamicScheme != null
      ? ColectoPalette.fromScheme(dynamicScheme)
      : brightness == Brightness.dark
      ? ColectoPalette.dark(accentHex: preferences.accent)
      : ColectoPalette.light(accentHex: preferences.accent);
  final platform = style.platform ?? defaultTargetPlatform;
  final isDesktopOrWeb = style.pointer;
  final fontFamily = resolveFontFamily(preferences.font, style: style);
  const fallback = ['Segoe UI', 'SF Pro Text', 'Mona Sans'];
  final spacing = preferences.textSpacing;

  final baseText = applyTextSpacing(
    TextStyle(
      fontFamily: fontFamily,
      fontFamilyFallback: fallback,
      color: palette.ink,
      fontSize: 14,
      height: 1.45,
      fontVariations: const [FontVariation.weight(450)],
    ),
    spacing,
    baseSize: 14,
  );

  final scheme =
      dynamicScheme ??
      ColorScheme.fromSeed(
        seedColor: palette.accent,
        brightness: brightness,
      ).copyWith(
        primary: palette.accent,
        onPrimary: palette.accentInk,
        secondaryContainer: palette.select,
        onSecondaryContainer: palette.accentHi,
        error: palette.danger,
        surface: palette.bg,
        onSurface: palette.ink,
        onSurfaceVariant: palette.ink2,
        surfaceContainerLowest: palette.sunk,
        surfaceContainerLow: palette.bg,
        surfaceContainer: palette.float,
        surfaceContainerHigh: palette.raise,
        surfaceContainerHighest: palette.control,
        outline: palette.line,
        outlineVariant: palette.lineSoft,
      );

  return ThemeData(
    useMaterial3: true,
    platform: platform,
    brightness: brightness,
    colorScheme: scheme,
    scaffoldBackgroundColor: palette.bg,
    canvasColor: palette.bg,
    cardColor: palette.float,
    dividerColor: palette.lineSoft,
    fontFamily: fontFamily,
    fontFamilyFallback: fallback,
    // No Material ripples on Desktop and Web (`AGENTS.md` rule 4).
    splashFactory: isDesktopOrWeb
        ? NoSplash.splashFactory
        : InkSparkle.splashFactory,
    highlightColor: isDesktopOrWeb ? Colors.transparent : null,
    splashColor: isDesktopOrWeb ? Colors.transparent : null,
    hoverColor: palette.raise,
    focusColor: palette.accent.withValues(alpha: 0.18),
    visualDensity: isDesktopOrWeb
        ? VisualDensity.compact
        : VisualDensity.standard,
    textSelectionTheme: TextSelectionThemeData(
      cursorColor: palette.accent,
      selectionColor: palette.accent.withValues(alpha: 0.32),
      selectionHandleColor: palette.accent,
    ),
    cupertinoOverrideTheme: CupertinoThemeData(
      brightness: brightness,
      primaryColor: palette.accent,
      scaffoldBackgroundColor: palette.bg,
      barBackgroundColor: palette.bg.withValues(alpha: 0.86),
    ),
    pageTransitionsTheme: const PageTransitionsTheme(
      builders: {
        TargetPlatform.android: PredictiveBackPageTransitionsBuilder(),
        TargetPlatform.iOS: CupertinoPageTransitionsBuilder(),
        TargetPlatform.macOS: FadeForwardsPageTransitionsBuilder(),
        TargetPlatform.linux: FadeForwardsPageTransitionsBuilder(),
        TargetPlatform.windows: FadeForwardsPageTransitionsBuilder(),
      },
    ),
    tooltipTheme: TooltipThemeData(
      waitDuration: const Duration(milliseconds: 450),
      preferBelow: true,
      textStyle: kStyle(12, weight: 520, color: palette.ink),
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
      decoration: BoxDecoration(
        color: palette.float,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: palette.line),
        boxShadow: palette.floatShadow,
      ),
    ),
    textTheme: TextTheme(
      bodyLarge: baseText.copyWith(fontSize: 15.5, height: 1.68),
      bodyMedium: baseText,
      bodySmall: baseText.copyWith(fontSize: 12.5, color: palette.ink2),
      titleMedium: baseText.merge(kStyle(15, weight: 640)),
      titleLarge: baseText.merge(
        kStyle(24, weight: 640, width: 108, letterSpacing: -0.48, height: 1.2),
      ),
      labelMedium: baseText.merge(kStyle(13, weight: 560)),
      labelSmall: baseText.merge(kStyle(12, color: palette.ink3)),
    ),
    scrollbarTheme: ScrollbarThemeData(
      thickness: WidgetStatePropertyAll(isDesktopOrWeb ? 6 : 3),
      radius: const Radius.circular(3),
      crossAxisMargin: 2,
      thumbColor: WidgetStateProperty.resolveWith((states) {
        if (states.contains(WidgetState.hovered) ||
            states.contains(WidgetState.dragged)) {
          return palette.ink3.withValues(alpha: 0.6);
        }
        return palette.line;
      }),
    ),
    extensions: [palette, ColectoDesign(style)],
  );
}
