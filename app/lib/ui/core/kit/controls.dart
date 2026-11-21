part of 'kit.dart';

/// An on/off switch: `ShadSwitch`, Fluent `ToggleSwitch`, `MacosSwitch`,
/// Material 3 `Switch` or `CupertinoSwitch`.
class ColectoSwitch extends StatelessWidget {
  const ColectoSwitch({
    super.key,
    required this.value,
    required this.onChanged,
    this.semanticLabel,
  });

  final bool value;
  final ValueChanged<bool>? onChanged;
  final String? semanticLabel;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return switch (colectoPlatformStyle(context)) {
      ColectoPlatformStyle.desktop => shad.ShadSwitch(
        value: value,
        enabled: onChanged != null,
        onChanged: onChanged,
        checkedTrackColor: p.accent,
        uncheckedTrackColor: p.control,
      ),
      ColectoPlatformStyle.fluent => fl.ToggleSwitch(
        checked: value,
        onChanged: onChanged,
        semanticLabel: semanticLabel,
      ),
      ColectoPlatformStyle.macos => mac.MacosSwitch(
        value: value,
        size: mac.ControlSize.small,
        activeColor: mac.MacosColor(p.accent.toARGB32()),
        onChanged: onChanged,
        semanticLabel: semanticLabel,
      ),
      ColectoPlatformStyle.android => Switch(
        value: value,
        onChanged: onChanged,
      ),
      ColectoPlatformStyle.ios => CupertinoSwitch(
        value: value,
        activeTrackColor: p.accent,
        onChanged: onChanged,
      ),
    };
  }
}

/// A checkbox: `ShadCheckbox`, Fluent `Checkbox`, `MacosCheckbox`, Material
/// `Checkbox` or `CupertinoCheckbox`.
class ColectoCheckbox extends StatelessWidget {
  const ColectoCheckbox({
    super.key,
    required this.value,
    required this.onChanged,
    this.semanticLabel,
  });

  final bool value;
  final ValueChanged<bool>? onChanged;
  final String? semanticLabel;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final cb = onChanged;
    return switch (colectoPlatformStyle(context)) {
      ColectoPlatformStyle.desktop => shad.ShadCheckbox(
        value: value,
        enabled: cb != null,
        onChanged: cb,
        color: p.accent,
      ),
      ColectoPlatformStyle.fluent => fl.Checkbox(
        checked: value,
        onChanged: cb == null ? null : (v) => cb(v ?? false),
        semanticLabel: semanticLabel,
      ),
      ColectoPlatformStyle.macos => mac.MacosCheckbox(
        value: value,
        activeColor: p.accent,
        onChanged: cb,
        semanticLabel: semanticLabel,
      ),
      ColectoPlatformStyle.android => Checkbox(
        value: value,
        onChanged: cb == null ? null : (v) => cb(v ?? false),
      ),
      ColectoPlatformStyle.ios => CupertinoCheckbox(
        value: value,
        activeColor: p.accent,
        onChanged: cb == null ? null : (v) => cb(v ?? false),
      ),
    };
  }
}

/// A filter or toggle pill (label filters, message labels). Colecto's own
/// component on every design; only the corner radius follows the design.
class ColectoChip extends StatelessWidget {
  const ColectoChip({
    super.key,
    required this.label,
    required this.onTap,
    this.selected = false,
    this.icon,
    this.onSecondaryTapDown,
    this.showCheck = false,
  });

  final String label;
  final VoidCallback? onTap;
  final bool selected;
  final IconData? icon;
  final ValueChanged<Offset>? onSecondaryTapDown;

  /// Toggle chips (message labels) show a check when on.
  final bool showCheck;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final touch = colectoTouch(context);
    final style = colectoPlatformStyle(context);
    final radius = switch (style) {
      ColectoPlatformStyle.fluent => 4.0,
      ColectoPlatformStyle.android => 8.0,
      _ => 999.0,
    };
    return ColectoPressable(
      onTap: onTap,
      onSecondaryTapDown: onSecondaryTapDown,
      onLongPress: onSecondaryTapDown,
      selected: selected,
      focusRadius: radius,
      semanticLabel: label,
      builder: (context, s) => AnimatedContainer(
        duration: ColectoMotion.of(context, ColectoMotion.hover),
        // Long label names truncate rather than push the chip off screen.
        constraints: const BoxConstraints(maxWidth: 240),
        height: touch ? 32 : (style == ColectoPlatformStyle.macos ? 22 : 26),
        padding: EdgeInsets.only(
          left: (showCheck && selected) || icon != null ? 8 : 10,
          right: 10,
        ),
        decoration: BoxDecoration(
          color: selected ? p.select : const Color(0x00000000),
          borderRadius: BorderRadius.circular(radius),
          border: Border.all(
            color: selected
                ? const Color(0x00000000)
                : (s.hovered ? p.ink3.withValues(alpha: 0.6) : p.line),
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (showCheck && selected) ...[
              _PopIn(
                child: Icon(LucideIcons.check, size: 12, color: p.accentHi),
              ),
              const SizedBox(width: 5),
            ] else if (icon != null) ...[
              Icon(icon, size: 12, color: selected ? p.accentHi : p.ink3),
              const SizedBox(width: 5),
            ],
            Flexible(
              child: Text(
                label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: kStyle(
                  touch ? 13.5 : 12.5,
                  weight: 540,
                  color: selected ? p.accentHi : (s.hovered ? p.ink : p.ink2),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The chosen option's check pops in (`@keyframes check-in`).
class _PopIn extends StatelessWidget {
  const _PopIn({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return TweenAnimationBuilder<double>(
      tween: Tween(begin: 0, end: 1),
      duration: ColectoMotion.of(context, const Duration(milliseconds: 220)),
      curve: ColectoMotion.easeOut,
      builder: (context, t, child) => Opacity(
        opacity: t,
        child: Transform.scale(scale: 0.6 + 0.4 * t, child: child),
      ),
      child: child,
    );
  }
}

/// A choice between two to four options, shown side by side.
///
/// Colecto's segmented control (shadcn tabs-list style), Fluent toggle buttons,
/// `MacosSegmentedControl`, Material 3 `SegmentedButton` or
/// `CupertinoSlidingSegmentedControl`.
class ColectoSegmented<T extends Object> extends StatelessWidget {
  const ColectoSegmented({
    super.key,
    required this.value,
    required this.options,
    required this.onChanged,
  });

  final T value;
  final List<(T, String)> options;
  final ValueChanged<T>? onChanged;

  @override
  Widget build(BuildContext context) {
    // Narrow pages (phones, zoomed windows) scroll the choices sideways
    // instead of clipping them.
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: _control(context),
    );
  }

  Widget _control(BuildContext context) {
    final p = ColectoPalette.of(context);
    switch (colectoPlatformStyle(context)) {
      case ColectoPlatformStyle.ios:
        return CupertinoSlidingSegmentedControl<T>(
          groupValue: value,
          onValueChanged: (v) {
            if (v != null) onChanged?.call(v);
          },
          children: {
            for (final o in options)
              o.$1: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
                child: Text(o.$2),
              ),
          },
        );
      case ColectoPlatformStyle.android:
        return SegmentedButton<T>(
          showSelectedIcon: false,
          segments: [
            for (final o in options)
              ButtonSegment<T>(value: o.$1, label: Text(o.$2)),
          ],
          selected: {value},
          onSelectionChanged: onChanged == null
              ? null
              : (s) => onChanged!(s.first),
        );
      case ColectoPlatformStyle.macos:
        return _MacosSegmented<T>(
          value: value,
          options: options,
          onChanged: onChanged,
        );
      case ColectoPlatformStyle.fluent:
        return Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (var i = 0; i < options.length; i++)
              Padding(
                padding: EdgeInsets.only(left: i == 0 ? 0 : 4),
                child: fl.ToggleButton(
                  checked: options[i].$1 == value,
                  onChanged: onChanged == null
                      ? null
                      : (_) => onChanged!(options[i].$1),
                  child: Text(options[i].$2),
                ),
              ),
          ],
        );
      case ColectoPlatformStyle.desktop:
        break;
    }
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: p.sunk,
        borderRadius: BorderRadius.circular(9),
        border: Border.all(color: p.line),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final o in options)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 1),
              child: ColectoPressable(
                onTap: onChanged == null ? null : () => onChanged!(o.$1),
                selected: o.$1 == value,
                semanticLabel: o.$2,
                focusRadius: 6,
                builder: (context, s) {
                  final on = o.$1 == value;
                  return AnimatedContainer(
                    duration: ColectoMotion.of(context, ColectoMotion.hover),
                    height: 30,
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: on
                          ? p.control
                          : (s.hovered
                                ? p.raise.withValues(alpha: 0.6)
                                : const Color(0x00000000)),
                      borderRadius: BorderRadius.circular(6),
                    ),
                    child: Text(
                      o.$2,
                      style: kStyle(
                        13,
                        weight: on ? 580 : 500,
                        color: on || s.hovered ? p.ink : p.ink2,
                      ),
                    ),
                  );
                },
              ),
            ),
        ],
      ),
    );
  }
}

class _MacosSegmented<T extends Object> extends StatefulWidget {
  const _MacosSegmented({
    required this.value,
    required this.options,
    required this.onChanged,
  });

  final T value;
  final List<(T, String)> options;
  final ValueChanged<T>? onChanged;

  @override
  State<_MacosSegmented<T>> createState() => _MacosSegmentedState<T>();
}

class _MacosSegmentedState<T extends Object> extends State<_MacosSegmented<T>> {
  late final mac.MacosTabController _c = mac.MacosTabController(
    length: widget.options.length,
    initialIndex: _index,
  )..addListener(_changed);

  int get _index =>
      widget.options.indexWhere((o) => o.$1 == widget.value).clamp(0, 999);

  void _changed() {
    final v = widget.options[_c.index].$1;
    if (v != widget.value) widget.onChanged?.call(v);
  }

  @override
  void didUpdateWidget(_MacosSegmented<T> old) {
    super.didUpdateWidget(old);
    if (_c.index != _index) _c.index = _index;
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return mac.MacosSegmentedControl(
      controller: _c,
      tabs: [for (final o in widget.options) mac.MacosTab(label: o.$2)],
    );
  }
}

/// Picks one value from a list.
///
/// `ShadSelect`, Fluent `ComboBox`, `MacosPopupButton`, Material 3
/// `DropdownMenu` or a Cupertino action sheet.
class ColectoSelect<T> extends StatelessWidget {
  const ColectoSelect({
    super.key,
    required this.value,
    required this.options,
    required this.onChanged,
    this.minWidth = 200,
  });

  final T value;
  final List<(T, String)> options;
  final ValueChanged<T>? onChanged;
  final double minWidth;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final current = options.where((o) => o.$1 == value).firstOrNull?.$2 ?? '';
    switch (colectoPlatformStyle(context)) {
      case ColectoPlatformStyle.desktop:
        return shad.ShadSelect<T>(
          initialValue: value,
          minWidth: minWidth,
          enabled: onChanged != null,
          onChanged: (v) {
            if (v != null) onChanged?.call(v);
          },
          selectedOptionBuilder: (context, v) =>
              Text(options.where((o) => o.$1 == v).firstOrNull?.$2 ?? ''),
          options: [
            for (final o in options)
              shad.ShadOption<T>(value: o.$1, child: Text(o.$2)),
          ],
        );
      case ColectoPlatformStyle.fluent:
        return ConstrainedBox(
          constraints: BoxConstraints(minWidth: minWidth),
          child: fl.ComboBox<T>(
            value: value,
            items: [
              for (final o in options)
                fl.ComboBoxItem<T>(value: o.$1, child: Text(o.$2)),
            ],
            onChanged: onChanged == null
                ? null
                : (v) {
                    if (v != null) onChanged!(v);
                  },
          ),
        );
      case ColectoPlatformStyle.macos:
        // MacosPopupButton can't truncate its label; let a long one scroll
        // on narrow pages rather than overflow.
        return SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: mac.MacosPopupButton<T>(
            value: value,
            items: [
              for (final o in options)
                mac.MacosPopupMenuItem<T>(value: o.$1, child: Text(o.$2)),
            ],
            onChanged: onChanged == null
                ? null
                : (v) {
                    if (v != null) onChanged!(v);
                  },
          ),
        );
      case ColectoPlatformStyle.android:
        return DropdownMenu<T>(
          initialSelection: value,
          width: minWidth + 40,
          enabled: onChanged != null,
          onSelected: (v) {
            if (v != null) onChanged?.call(v);
          },
          dropdownMenuEntries: [
            for (final o in options)
              DropdownMenuEntry<T>(value: o.$1, label: o.$2),
          ],
        );
      case ColectoPlatformStyle.ios:
        return Builder(
          builder: (ctx) => CupertinoButton(
            padding: EdgeInsets.zero,
            onPressed: onChanged == null
                ? null
                : () => showColectoContextMenu(
                    context: ctx,
                    globalPosition: colectoRectOf(ctx).center,
                    items: [
                      for (final o in options)
                        ColectoMenuItem(
                          label: o.$2,
                          checked: o.$1 == value,
                          onSelected: () => onChanged!(o.$1),
                        ),
                    ],
                  ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(current, style: TextStyle(color: p.accent)),
                const SizedBox(width: 4),
                Icon(LucideIcons.chevronsUpDown, size: 16, color: p.accent),
              ],
            ),
          ),
        );
    }
  }
}
