part of 'kit.dart';

/// True on phones and tablets, where targets grow to 44 px and inputs use a
/// 16 px font so iOS never zooms the page.
bool colectoTouch(BuildContext context) => colectoPlatformStyle(context).touch;

@immutable
class ColectoInteraction {
  const ColectoInteraction({
    required this.hovered,
    required this.pressed,
    required this.focused,
    required this.enabled,
  });

  final bool hovered;
  final bool pressed;

  /// Keyboard focus only (never shown for pointer clicks).
  final bool focused;
  final bool enabled;
}

/// The one pressable behind every Colecto control: hover, keyboard activation
/// (Enter/Space), a sharp focus ring, and Colecto's `scale(0.97)` press.
class ColectoPressable extends StatefulWidget {
  const ColectoPressable({
    super.key,
    required this.builder,
    this.onTap,
    this.onDoubleTap,
    this.onSecondaryTapDown,
    this.onLongPress,
    this.tooltip,
    this.semanticLabel,
    this.selected,
    this.pressScale = ColectoMotion.pressScale,
    this.focusRadius = 8,
    this.focusNode,
    this.autofocus = false,
    this.cursor,
    this.onHoverChanged,
  });

  final Widget Function(BuildContext context, ColectoInteraction state) builder;
  final VoidCallback? onTap;
  final VoidCallback? onDoubleTap;
  final ValueChanged<Offset>? onSecondaryTapDown;
  final ValueChanged<Offset>? onLongPress;
  final String? tooltip;
  final String? semanticLabel;
  final bool? selected;
  final double pressScale;
  final double focusRadius;
  final FocusNode? focusNode;
  final bool autofocus;
  final MouseCursor? cursor;
  final ValueChanged<bool>? onHoverChanged;

  @override
  State<ColectoPressable> createState() => _ColectoPressableState();
}

class _ColectoPressableState extends State<ColectoPressable> {
  bool _hovered = false;
  bool _pressed = false;
  bool _focused = false;

  bool get _enabled => widget.onTap != null || widget.onDoubleTap != null;

  void _setPressed(bool v) {
    if (_pressed != v) setState(() => _pressed = v);
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final state = ColectoInteraction(
      hovered: _hovered && _enabled,
      pressed: _pressed,
      focused: _focused,
      enabled: _enabled,
    );
    Widget child = widget.builder(context, state);
    if (widget.pressScale != 1) {
      child = AnimatedScale(
        scale: _pressed ? widget.pressScale : 1,
        duration: ColectoMotion.of(context, ColectoMotion.press),
        curve: ColectoMotion.easeOut,
        child: child,
      );
    }
    child = CustomPaint(
      foregroundPainter: _FocusRingPainter(
        visible: _focused,
        color: p.accent,
        radius: widget.focusRadius,
      ),
      child: child,
    );

    child = FocusableActionDetector(
      enabled: _enabled,
      focusNode: widget.focusNode,
      autofocus: widget.autofocus,
      mouseCursor:
          widget.cursor ??
          (_enabled ? SystemMouseCursors.click : SystemMouseCursors.basic),
      onShowHoverHighlight: (v) {
        setState(() => _hovered = v);
        widget.onHoverChanged?.call(v);
      },
      onShowFocusHighlight: (v) => setState(() => _focused = v),
      actions: {
        ActivateIntent: CallbackAction<ActivateIntent>(
          onInvoke: (_) {
            widget.onTap?.call();
            return null;
          },
        ),
      },
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.onTap,
        onDoubleTap: widget.onDoubleTap,
        onTapDown: _enabled ? (_) => _setPressed(true) : null,
        onTapUp: _enabled ? (_) => _setPressed(false) : null,
        onTapCancel: _enabled ? () => _setPressed(false) : null,
        onSecondaryTapDown: widget.onSecondaryTapDown == null
            ? null
            : (d) => widget.onSecondaryTapDown!(d.globalPosition),
        onLongPressStart: widget.onLongPress == null
            ? null
            : (d) {
                _setPressed(false);
                HapticFeedback.mediumImpact();
                widget.onLongPress!(d.globalPosition);
              },
        child: child,
      ),
    );

    child = Semantics(
      button: true,
      enabled: _enabled,
      selected: widget.selected,
      label: widget.semanticLabel,
      child: child,
    );

    if (widget.tooltip != null && widget.tooltip!.isNotEmpty) {
      child = Tooltip(message: widget.tooltip!, child: child);
    }
    return child;
  }
}

class _FocusRingPainter extends CustomPainter {
  _FocusRingPainter({
    required this.visible,
    required this.color,
    required this.radius,
  });

  final bool visible;
  final Color color;
  final double radius;

  @override
  void paint(Canvas canvas, Size size) {
    if (!visible) return;
    // `outline: 2px solid var(--mint); outline-offset: 2px`.
    final rect = (Offset.zero & size).inflate(3);
    canvas.drawRRect(
      RRect.fromRectAndRadius(rect, Radius.circular(radius + 3)),
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2
        ..color = color,
    );
  }

  @override
  bool shouldRepaint(_FocusRingPainter old) =>
      old.visible != visible || old.color != color || old.radius != radius;
}

Rect _rectOf(BuildContext context) {
  final box = context.findRenderObject() as RenderBox?;
  if (box == null || !box.hasSize) return Rect.zero;
  return box.localToGlobal(Offset.zero) & box.size;
}

/// The global rect of [context]'s render box, for anchoring popovers.
Rect colectoRectOf(BuildContext context) => _rectOf(context);
