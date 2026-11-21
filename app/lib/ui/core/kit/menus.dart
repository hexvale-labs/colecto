part of 'kit.dart';

class ColectoMenuItem {
  const ColectoMenuItem({
    required this.label,
    required this.onSelected,
    this.icon,
    this.danger = false,
    this.checked,
    this.shortcut,
    this.enabled = true,
    this.subtitle,
  }) : divider = false;

  const ColectoMenuItem.divider()
    : label = '',
      onSelected = null,
      icon = null,
      danger = false,
      checked = null,
      shortcut = null,
      enabled = false,
      subtitle = null,
      divider = true;

  final String label;
  final VoidCallback? onSelected;
  final IconData? icon;
  final bool danger;

  /// Non-null renders a trailing check slot (listbox options).
  final bool? checked;
  final String? shortcut;
  final bool enabled;
  final String? subtitle;
  final bool divider;
}

/// Kept for call sites written against the first scaffold.
typedef ColectoContextMenuItem = ColectoMenuItem;

/// Shows a context menu: a popover that grows from the pointer on desktop
/// and web, a Cupertino action sheet on iOS, a Material bottom sheet on
/// Android.
Future<void> showColectoContextMenu({
  required BuildContext context,
  required Offset globalPosition,
  required List<ColectoMenuItem> items,
  String? title,
}) {
  switch (colectoPlatformStyle(context)) {
    case ColectoPlatformStyle.ios:
      return _showCupertinoActions(context, items, title);
    case ColectoPlatformStyle.android:
      return _showMaterialActions(context, items, title);
    case ColectoPlatformStyle.desktop ||
        ColectoPlatformStyle.fluent ||
        ColectoPlatformStyle.macos:
      return showColectoMenu(
        context: context,
        anchor: globalPosition & Size.zero,
        items: items,
      );
  }
}

Future<void> _showCupertinoActions(
  BuildContext context,
  List<ColectoMenuItem> items,
  String? title,
) async {
  VoidCallback? chosen;
  await showCupertinoModalPopup<void>(
    context: context,
    builder: (ctx) => CupertinoActionSheet(
      title: title == null ? null : Text(title),
      actions: [
        for (final item in items.where((i) => !i.divider))
          CupertinoActionSheetAction(
            isDestructiveAction: item.danger,
            onPressed: () {
              chosen = item.onSelected;
              Navigator.of(ctx).pop();
            },
            child: Text(item.label),
          ),
      ],
      cancelButton: CupertinoActionSheetAction(
        isDefaultAction: true,
        onPressed: () => Navigator.of(ctx).pop(),
        child: const Text('Cancel'),
      ),
    ),
  );
  chosen?.call();
}

Future<void> _showMaterialActions(
  BuildContext context,
  List<ColectoMenuItem> items,
  String? title,
) async {
  final p = ColectoPalette.of(context);
  VoidCallback? chosen;
  await showModalBottomSheet<void>(
    context: context,
    showDragHandle: true,
    backgroundColor: p.float,
    builder: (ctx) => SafeArea(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (title != null)
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 0, 24, 8),
              child: Text(
                title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: kStyle(14, weight: 600, color: p.ink2),
              ),
            ),
          for (final item in items)
            if (item.divider)
              Divider(height: 9, color: p.lineSoft)
            else
              ListTile(
                enabled: item.enabled,
                leading: item.icon == null
                    ? null
                    : Icon(item.icon, color: item.danger ? p.danger : p.ink2),
                title: Text(
                  item.label,
                  style: kStyle(
                    15,
                    weight: 500,
                    color: item.danger ? p.danger : p.ink,
                  ),
                ),
                trailing: item.checked == true
                    ? Icon(LucideIcons.check, color: p.accent)
                    : null,
                onTap: () {
                  chosen = item.onSelected;
                  Navigator.of(ctx).pop();
                },
              ),
          const SizedBox(height: 8),
        ],
      ),
    ),
  );
  chosen?.call();
}

/// A Colecto listbox popover anchored below [anchor] (or above, when there
/// is no room). It grows from its trigger with a little scale, a little
/// travel and a 2 px blur that clears as it arrives; it leaves in 120 ms.
Future<void> showColectoMenu({
  required BuildContext context,
  required Rect anchor,
  required List<ColectoMenuItem> items,
  double width = 220,
  bool alignRight = false,
}) async {
  // Anchors arrive in screen coordinates; the menu lays out in the
  // navigator's (which differ while the interface is zoomed).
  final overlay =
      Navigator.of(context).overlay?.context.findRenderObject() as RenderBox?;
  final local = overlay == null
      ? anchor
      : Rect.fromPoints(
          overlay.globalToLocal(anchor.topLeft),
          overlay.globalToLocal(anchor.bottomRight),
        );
  final chosen = await Navigator.of(context).push<VoidCallback>(
    _ColectoMenuRoute(
      anchor: local,
      items: items,
      width: width,
      alignRight: alignRight,
      capturedThemes: InheritedTheme.capture(
        from: context,
        to: Navigator.of(context).context,
      ),
    ),
  );
  chosen?.call();
}

class _ColectoMenuRoute extends PopupRoute<VoidCallback> {
  _ColectoMenuRoute({
    required this.anchor,
    required this.items,
    required this.width,
    required this.alignRight,
    required this.capturedThemes,
  });

  final Rect anchor;
  final List<ColectoMenuItem> items;
  final double width;
  final bool alignRight;
  final CapturedThemes capturedThemes;

  @override
  Color? get barrierColor => null;

  @override
  bool get barrierDismissible => true;

  @override
  String? get barrierLabel => 'Close menu';

  @override
  Duration get transitionDuration => ColectoMotion.popIn;

  @override
  Duration get reverseTransitionDuration => ColectoMotion.popOut;

  @override
  Widget buildPage(
    BuildContext context,
    Animation<double> animation,
    Animation<double> secondaryAnimation,
  ) {
    return capturedThemes.wrap(
      _ColectoMenuLayout(
        anchor: anchor,
        width: width,
        alignRight: alignRight,
        animation: animation,
        items: items,
      ),
    );
  }
}

class _ColectoMenuLayout extends StatelessWidget {
  const _ColectoMenuLayout({
    required this.anchor,
    required this.width,
    required this.alignRight,
    required this.animation,
    required this.items,
  });

  final Rect anchor;
  final double width;
  final bool alignRight;
  final Animation<double> animation;
  final List<ColectoMenuItem> items;

  @override
  Widget build(BuildContext context) {
    final screen = MediaQuery.sizeOf(context);
    final padding = MediaQuery.paddingOf(context);
    final estimated = math.min(
      320.0,
      items.fold<double>(
        10,
        (h, i) => h + (i.divider ? 9 : (i.subtitle != null ? 46 : 34)),
      ),
    );
    final below =
        anchor.bottom + 6 + estimated < screen.height - padding.bottom;
    final top = below ? anchor.bottom + 6 : null;
    final bottom = below ? null : screen.height - anchor.top + 6;
    var left = alignRight ? anchor.right - width : anchor.left;
    left = left.clamp(8.0, math.max(8.0, screen.width - width - 8));
    final reduce = MediaQuery.maybeDisableAnimationsOf(context) ?? false;

    return Stack(
      children: [
        Positioned(
          left: left,
          top: top,
          bottom: bottom,
          width: width,
          child: AnimatedBuilder(
            animation: animation,
            builder: (context, child) {
              final forward = animation.status != AnimationStatus.reverse;
              final t = reduce
                  ? 1.0
                  : (forward ? ColectoMotion.easeOut : Curves.easeIn).transform(
                      animation.value,
                    );
              final blur = (1 - t) * 2;
              Widget out = Opacity(
                opacity: t.clamp(0.0, 1.0),
                child: Transform.translate(
                  offset: Offset(0, (1 - t) * (below ? -6 : 6)),
                  child: Transform.scale(
                    scale: 0.95 + 0.05 * t,
                    alignment: Alignment(alignRight ? 1 : -1, below ? -1 : 1),
                    child: child,
                  ),
                ),
              );
              if (blur > 0.05) {
                out = ImageFiltered(
                  imageFilter: ImageFilter.blur(sigmaX: blur, sigmaY: blur),
                  child: out,
                );
              }
              return out;
            },
            child: _ColectoMenuPanel(items: items),
          ),
        ),
      ],
    );
  }
}

class _ColectoMenuPanel extends StatefulWidget {
  const _ColectoMenuPanel({required this.items});

  final List<ColectoMenuItem> items;

  @override
  State<_ColectoMenuPanel> createState() => _ColectoMenuPanelState();
}

class _ColectoMenuPanelState extends State<_ColectoMenuPanel> {
  late int _active = widget.items.indexWhere((i) => i.checked == true);
  final _focus = FocusNode();

  List<int> get _selectable => [
    for (var i = 0; i < widget.items.length; i++)
      if (!widget.items[i].divider && widget.items[i].enabled) i,
  ];

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  void _move(int delta) {
    final sel = _selectable;
    if (sel.isEmpty) return;
    final at = sel.indexOf(_active);
    final next = at < 0
        ? (delta > 0 ? 0 : sel.length - 1)
        : (at + delta) % sel.length;
    setState(() => _active = sel[next < 0 ? sel.length - 1 : next]);
  }

  void _choose(int i) {
    final item = widget.items[i];
    if (!item.enabled || item.divider) return;
    Navigator.of(context).pop(item.onSelected);
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final key = event.logicalKey;
    if (key == LogicalKeyboardKey.arrowDown) {
      _move(1);
    } else if (key == LogicalKeyboardKey.arrowUp) {
      _move(-1);
    } else if (key == LogicalKeyboardKey.enter ||
        key == LogicalKeyboardKey.space) {
      if (_active >= 0) _choose(_active);
    } else if (key == LogicalKeyboardKey.escape) {
      Navigator.of(context).pop();
    } else if (event.character != null && event.character!.trim().length == 1) {
      // Typing a letter jumps to a matching option.
      final ch = event.character!.toLowerCase();
      for (final i in _selectable) {
        if (widget.items[i].label.toLowerCase().startsWith(ch)) {
          setState(() => _active = i);
          break;
        }
      }
    } else {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Focus(
      focusNode: _focus,
      autofocus: true,
      onKeyEvent: _onKey,
      child: Container(
        constraints: const BoxConstraints(maxHeight: 320),
        decoration: BoxDecoration(
          color: p.float,
          borderRadius: BorderRadius.circular(_panelRadius(context)),
          border: Border.all(color: p.line),
          boxShadow: p.floatShadow,
        ),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(_panelRadius(context) - 1),
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(5),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                for (var i = 0; i < widget.items.length; i++)
                  widget.items[i].divider
                      ? Padding(
                          padding: const EdgeInsets.symmetric(vertical: 4),
                          child: Container(height: 1, color: p.lineSoft),
                        )
                      : _menuRow(context, p, i),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _menuRow(BuildContext context, ColectoPalette p, int i) {
    final style = colectoPlatformStyle(context);
    // macOS highlights the hovered item in the accent; Fluent and Colecto tint.
    final mac = style == ColectoPlatformStyle.macos;
    final fluent = style == ColectoPlatformStyle.fluent;
    final item = widget.items[i];
    final active = i == _active;
    final fg = !item.enabled
        ? p.ink3
        : item.danger
        ? p.danger
        : p.ink;
    return MouseRegion(
      cursor: item.enabled ? SystemMouseCursors.click : MouseCursor.defer,
      onEnter: (_) {
        if (item.enabled) setState(() => _active = i);
      },
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => _choose(i),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 90),
          padding: EdgeInsets.symmetric(
            horizontal: 10,
            vertical: item.subtitle != null ? 6 : (mac ? 3 : 7),
          ),
          decoration: BoxDecoration(
            color: active && item.enabled
                ? (item.danger
                      ? p.dangerBg
                      : mac
                      ? p.accent
                      : p.raise)
                : const Color(0x00000000),
            borderRadius: BorderRadius.circular(mac || fluent ? 4 : 8),
          ),
          child: Row(
            children: [
              if (item.icon != null) ...[
                Icon(
                  item.icon,
                  size: 15,
                  color: item.danger ? p.danger : p.ink2,
                ),
                const SizedBox(width: 10),
              ],
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      item.label,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: kStyle(
                        mac ? 13 : (fluent ? 14 : 13.5),
                        color: mac && active && !item.danger ? p.accentInk : fg,
                      ),
                    ),
                    if (item.subtitle != null)
                      Text(
                        item.subtitle!,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: kStyle(12, color: p.ink3),
                      ),
                  ],
                ),
              ),
              if (item.shortcut != null) ...[
                const SizedBox(width: 12),
                Text(item.shortcut!, style: kStyle(12, color: p.ink3)),
              ],
              if (item.checked == true) ...[
                const SizedBox(width: 12),
                _PopIn(
                  child: Icon(LucideIcons.check, size: 15, color: p.accent),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

double _panelRadius(BuildContext context) =>
    switch (colectoPlatformStyle(context)) {
      ColectoPlatformStyle.fluent => 8,
      ColectoPlatformStyle.macos => 7,
      _ => 12,
    };
