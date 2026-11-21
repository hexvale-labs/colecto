part of 'kit.dart';

/// Gives the design libraries their themes, all derived from the one Colecto
/// palette, so a shadcn, Fluent or macOS control always matches the accent
/// and surfaces around it. Only the active design's theme is installed.
class ColectoDesignScope extends StatelessWidget {
  const ColectoDesignScope({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final family =
        DefaultTextStyle.of(context).style.fontFamily ??
        Theme.of(context).textTheme.bodyMedium?.fontFamily;
    switch (colectoPlatformStyle(context)) {
      case ColectoPlatformStyle.desktop:
        return shad.ShadTheme(data: _shadTheme(p, family), child: child);
      case ColectoPlatformStyle.fluent:
        return fl.FluentTheme(data: _fluentTheme(p, family), child: child);
      case ColectoPlatformStyle.macos:
        return mac.MacosTheme(
          data: mac.MacosThemeData(
            brightness: p.brightness,
            primaryColor: p.accent,
            canvasColor: p.bg,
            dividerColor: p.lineSoft,
          ),
          child: child,
        );
      case ColectoPlatformStyle.android:
      case ColectoPlatformStyle.ios:
        return child;
    }
  }

  static shad.ShadThemeData _shadTheme(ColectoPalette p, String? family) {
    return shad.ShadThemeData(
      brightness: p.brightness,
      radius: const BorderRadius.all(Radius.circular(8)),
      textTheme: shad.ShadTextTheme(family: family ?? 'Mona Sans'),
      colorScheme: shad.ShadColorScheme(
        background: p.bg,
        foreground: p.ink,
        card: p.float,
        cardForeground: p.ink,
        popover: p.float,
        popoverForeground: p.ink,
        primary: p.accent,
        primaryForeground: p.accentInk,
        secondary: p.control,
        secondaryForeground: p.ink,
        muted: p.raise,
        mutedForeground: p.ink3,
        accent: p.raise,
        accentForeground: p.ink,
        destructive: p.dangerBg,
        destructiveForeground: p.dangerInk,
        border: p.line,
        input: p.line,
        ring: p.accent,
        selection: p.accent.withValues(alpha: 0.32),
      ),
    );
  }

  static fl.FluentThemeData _fluentTheme(ColectoPalette p, String? family) {
    Color mix(Color c, double t) => Color.lerp(p.accent, c, t)!;
    const black = Color(0xFF000000);
    const white = Color(0xFFFFFFFF);
    final accent = fl.AccentColor.swatch({
      'darkest': mix(black, 0.45),
      'darker': mix(black, 0.3),
      'dark': mix(black, 0.15),
      'normal': p.accent,
      'light': mix(white, 0.15),
      'lighter': mix(white, 0.3),
      'lightest': mix(white, 0.45),
    });
    return fl.FluentThemeData(
      brightness: p.brightness,
      accentColor: accent,
      fontFamily: family,
      scaffoldBackgroundColor: p.bg,
      micaBackgroundColor: p.ground,
      cardColor: p.float,
      menuColor: p.float,
      selectionColor: p.accent,
      visualDensity: VisualDensity.standard,
    );
  }
}
