import 'package:dynamic_color/dynamic_color.dart';
import 'package:fluent_ui/fluent_ui.dart' show FluentLocalizations;
import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';

import 'ui/core/kit/kit.dart';
import 'ui/core/theme/colecto_palette.dart';
import 'ui/core/theme/colecto_scroll_behavior.dart';
import 'ui/features/shell/mail_controller.dart';
import 'ui/features/shell/mail_shell.dart';

/// Whole-interface zoom (Ctrl/Cmd +, -, 0): lays the app out at
/// `size / factor` and scales it up, like a browser's page zoom, so layouts
/// reflow instead of overflowing.
class _Zoom extends StatelessWidget {
  const _Zoom({required this.factor, required this.child});

  final double factor;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (factor == 1) return child;
    final mq = MediaQuery.of(context);
    final size = mq.size / factor;
    return MediaQuery(
      data: mq.copyWith(
        size: size,
        padding: mq.padding / factor,
        viewPadding: mq.viewPadding / factor,
        viewInsets: mq.viewInsets / factor,
      ),
      child: FittedBox(
        fit: BoxFit.fill,
        alignment: Alignment.topLeft,
        child: SizedBox(width: size.width, height: size.height, child: child),
      ),
    );
  }
}

class ColectoApp extends StatelessWidget {
  const ColectoApp({super.key, required this.controller});

  final MailController controller;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final prefs = controller.preferences;
        final style = controller.design;

        Widget app(ColorScheme? light, ColorScheme? dark) => MaterialApp(
          title: 'Colecto',
          debugShowCheckedModeBanner: false,
          // Fluent widgets (ComboBox, dialogs) need their own strings.
          localizationsDelegates: const [FluentLocalizations.delegate],
          themeMode: controller.themeMode,
          themeAnimationDuration: const Duration(milliseconds: 160),
          scrollBehavior: style.pointer
              ? const ColectoDesktopScrollBehavior()
              : const MaterialScrollBehavior(),
          theme: buildColectoTheme(
            brightness: Brightness.light,
            preferences: prefs,
            style: style,
            dynamicScheme: light?.harmonized(),
          ),
          darkTheme: buildColectoTheme(
            brightness: Brightness.dark,
            preferences: prefs,
            style: style,
            dynamicScheme: dark?.harmonized(),
          ),
          // `Preferences.font_size` (13–18 px, 14 default) scales every
          // label with the system text size rather than replacing it.
          builder: (context, child) {
            final mq = MediaQuery.of(context);
            final factor = (prefs.fontSize ?? 14) / 14;
            final zoomed = MediaQuery(
              data: mq.copyWith(
                textScaler: TextScaler.linear(mq.textScaler.scale(1) * factor),
              ),
              child: ColectoDesignScope(child: child!),
            );
            return _Zoom(factor: controller.zoom, child: zoomed);
          },
          home: MailShell(controller: controller),
        );

        // Material You: on the Material design, with no accent picked, the
        // system's dynamic colours drive the whole palette.
        final useSystemColors =
            style == ColectoPlatformStyle.android &&
            !kIsWeb &&
            (prefs.accent == null || prefs.accent!.isEmpty);
        return useSystemColors
            ? DynamicColorBuilder(builder: app)
            : app(null, null);
      },
    );
  }
}
