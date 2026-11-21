import 'package:flutter/gestures.dart';
import 'package:material_ui/material_ui.dart';

/// Desktop/Web scroll behavior without mobile overscroll glow or bounce,
/// with crisp scrollbars and mouse/trackpad support (`docs/ui-design.md`).
class ColectoDesktopScrollBehavior extends MaterialScrollBehavior {
  const ColectoDesktopScrollBehavior();

  @override
  Set<PointerDeviceKind> get dragDevices => const {
    PointerDeviceKind.touch,
    PointerDeviceKind.mouse,
    PointerDeviceKind.trackpad,
    PointerDeviceKind.stylus,
  };

  @override
  Widget buildOverscrollIndicator(
    BuildContext context,
    Widget child,
    ScrollableDetails details,
  ) {
    return child;
  }

  @override
  ScrollPhysics getScrollPhysics(BuildContext context) {
    return const ClampingScrollPhysics();
  }
}
