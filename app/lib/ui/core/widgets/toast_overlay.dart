import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/preferences_models.dart';
import '../theme/colecto_palette.dart';
import '../kit/kit.dart';

@immutable
class ColectoToast {
  const ColectoToast({
    required this.id,
    required this.message,
    this.detail,
    this.isError = false,
  });

  final int id;
  final String message;
  final String? detail;
  final bool isError;
}

/// Colecto's notifications: they enter from the edge they are docked to,
/// leave the same way, the stack slides when one goes, and hovering one
/// keeps it on screen.
class ToastOverlay extends StatefulWidget {
  const ToastOverlay({
    super.key,
    required this.toasts,
    required this.position,
    required this.onDismiss,
    required this.onHold,
    this.topInset = 64,
  });

  final List<ColectoToast> toasts;
  final NotificationCorner position;
  final ValueChanged<int> onDismiss;

  /// Called with true while a toast is hovered (timers pause), false after.
  final void Function(int id, bool hold) onHold;
  final double topInset;

  @override
  State<ToastOverlay> createState() => _ToastOverlayState();
}

class _ToastOverlayState extends State<ToastOverlay> {
  final List<ColectoToast> _shown = [];
  final Set<int> _leaving = {};

  @override
  void initState() {
    super.initState();
    _shown.addAll(widget.toasts);
  }

  @override
  void didUpdateWidget(ToastOverlay old) {
    super.didUpdateWidget(old);
    final live = {for (final t in widget.toasts) t.id};
    for (final t in _shown) {
      if (!live.contains(t.id)) _leaving.add(t.id);
    }
    for (final t in widget.toasts) {
      if (!_shown.any((s) => s.id == t.id)) _shown.add(t);
    }
  }

  void _gone(int id) {
    setState(() {
      _shown.removeWhere((t) => t.id == id);
      _leaving.remove(id);
    });
  }

  Alignment get _alignment => switch (widget.position) {
    NotificationCorner.topLeft => Alignment.topLeft,
    NotificationCorner.top => Alignment.topCenter,
    NotificationCorner.topRight => Alignment.topRight,
    NotificationCorner.right => Alignment.centerRight,
    NotificationCorner.bottomRight => Alignment.bottomRight,
    NotificationCorner.bottom => Alignment.bottomCenter,
    NotificationCorner.bottomLeft => Alignment.bottomLeft,
    NotificationCorner.left => Alignment.centerLeft,
  };

  Offset get _enter => switch (widget.position) {
    NotificationCorner.left => const Offset(-16, 0),
    NotificationCorner.right => const Offset(16, 0),
    NotificationCorner.topLeft ||
    NotificationCorner.top ||
    NotificationCorner.topRight => const Offset(0, -10),
    _ => const Offset(0, 10),
  };

  @override
  Widget build(BuildContext context) {
    if (_shown.isEmpty) return const SizedBox.shrink();
    final top = _alignment.y < 0;
    final pad = MediaQuery.paddingOf(context);
    final items = [
      for (final t in _shown)
        _ToastCard(
          key: ValueKey(t.id),
          toast: t,
          enter: _enter,
          leaving: _leaving.contains(t.id),
          onGone: () => _gone(t.id),
          onDismiss: () => widget.onDismiss(t.id),
          onHold: (hold) => widget.onHold(t.id, hold),
        ),
    ];
    return Align(
      alignment: _alignment,
      child: Padding(
        padding: EdgeInsets.fromLTRB(
          16 + pad.left,
          top ? widget.topInset + pad.top : 16,
          16 + pad.right,
          16 + pad.bottom,
        ),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 360),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            // Newest sits nearest the edge it is docked to.
            children: top ? items.reversed.toList() : items,
          ),
        ),
      ),
    );
  }
}

class _ToastCard extends StatefulWidget {
  const _ToastCard({
    super.key,
    required this.toast,
    required this.enter,
    required this.leaving,
    required this.onGone,
    required this.onDismiss,
    required this.onHold,
  });

  final ColectoToast toast;
  final Offset enter;
  final bool leaving;
  final VoidCallback onGone;
  final VoidCallback onDismiss;
  final ValueChanged<bool> onHold;

  @override
  State<_ToastCard> createState() => _ToastCardState();
}

class _ToastCardState extends State<_ToastCard>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 300),
    reverseDuration: const Duration(milliseconds: 220),
  );

  @override
  void initState() {
    super.initState();
    _c.forward();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (MediaQuery.maybeDisableAnimationsOf(context) ?? false) {
      _c.value = widget.leaving ? 0 : 1;
    }
  }

  @override
  void didUpdateWidget(_ToastCard old) {
    super.didUpdateWidget(old);
    if (widget.leaving && !old.leaving) {
      _c.reverse().whenComplete(() {
        if (mounted) widget.onGone();
      });
    }
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final t = widget.toast;
    final curve = CurvedAnimation(
      parent: _c,
      curve: ColectoMotion.easeOut,
      reverseCurve: Curves.easeIn,
    );
    return SizeTransition(
      sizeFactor: curve,
      alignment: Alignment.topCenter,
      child: FadeTransition(
        opacity: curve,
        child: AnimatedBuilder(
          animation: curve,
          builder: (context, child) => Transform.translate(
            offset: widget.enter * (1 - curve.value),
            child: child,
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 4),
            child: MouseRegion(
              onEnter: (_) => widget.onHold(true),
              onExit: (_) => widget.onHold(false),
              child: Semantics(
                liveRegion: true,
                child: Container(
                  padding: const EdgeInsets.fromLTRB(12, 12, 10, 12),
                  decoration: BoxDecoration(
                    color: p.float,
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(color: p.line),
                    boxShadow: p.floatShadow,
                  ),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Container(
                        width: 30,
                        height: 30,
                        decoration: BoxDecoration(
                          color: t.isError ? p.dangerBg : p.select,
                          borderRadius: BorderRadius.circular(8),
                        ),
                        child: Icon(
                          t.isError
                              ? LucideIcons.circleAlert
                              : LucideIcons.check,
                          size: 15,
                          color: t.isError ? p.danger : p.accentHi,
                        ),
                      ),
                      const SizedBox(width: 12),
                      Expanded(
                        child: Padding(
                          padding: EdgeInsets.only(
                            top: t.detail == null ? 6 : 0,
                          ),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                t.message,
                                style: kStyle(
                                  13.5,
                                  weight: 620,
                                  color: p.ink,
                                  height: 1.35,
                                ),
                              ),
                              if (t.detail != null) ...[
                                const SizedBox(height: 2),
                                Text(
                                  t.detail!,
                                  style: kStyle(
                                    13,
                                    color: p.ink2,
                                    height: 1.45,
                                  ),
                                ),
                              ],
                            ],
                          ),
                        ),
                      ),
                      const SizedBox(width: 6),
                      ColectoIconButton(
                        icon: LucideIcons.x,
                        tooltip: 'Dismiss',
                        size: 14,
                        dimension: 24,
                        onPressed: widget.onDismiss,
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
