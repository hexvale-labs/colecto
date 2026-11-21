part of 'kit.dart';

/// What a button does, which each design draws its own way:
/// - [primary]: the one main action (Send, Save, Connect).
/// - [secondary]: an ordinary action.
/// - [ghost]: a toolbar or low-emphasis action.
/// - [danger]: something destructive.
/// - [text]: an inline link-like action.
enum ColectoButtonVariant { primary, secondary, ghost, danger, text }

/// A labelled button.
///
/// | Design | Widget |
/// | --- | --- |
/// | Colecto (Linux, Web) | `ShadButton` (+ `.secondary`, `.ghost`, `.destructive`, `.link`) |
/// | Fluent (Windows) | `FilledButton`, `Button`, `HyperlinkButton` |
/// | macOS | `PushButton` (accent or secondary) |
/// | Material 3 | `FilledButton`, `FilledButton.tonal`, `TextButton` |
/// | Cupertino | `CupertinoButton.filled`, `.tinted`, plain |
class ColectoButton extends StatelessWidget {
  const ColectoButton({
    super.key,
    required this.onPressed,
    required this.label,
    this.icon,
    this.trailingIcon,
    this.variant = ColectoButtonVariant.secondary,
    this.compact = false,
    this.tooltip,
    this.expand = false,
  });

  final VoidCallback? onPressed;
  final String label;
  final IconData? icon;
  final IconData? trailingIcon;
  final ColectoButtonVariant variant;
  final bool compact;
  final String? tooltip;
  final bool expand;

  @override
  Widget build(BuildContext context) {
    if (label.isEmpty && icon != null) {
      return ColectoIconButton(
        icon: icon!,
        tooltip: tooltip,
        onPressed: onPressed,
        danger: variant == ColectoButtonVariant.danger,
      );
    }
    final Widget button = switch (colectoPlatformStyle(context)) {
      ColectoPlatformStyle.desktop => _shad(context),
      ColectoPlatformStyle.fluent => _fluent(context),
      ColectoPlatformStyle.macos => _macos(context),
      ColectoPlatformStyle.android => _material(context),
      ColectoPlatformStyle.ios => _cupertino(context),
    };
    final sized = expand
        ? SizedBox(width: double.infinity, child: button)
        : button;
    return tooltip == null || tooltip!.isEmpty
        ? sized
        : Tooltip(message: tooltip!, child: sized);
  }

  Widget _content(Color? color, {double gap = 7, double iconSize = 15}) {
    return Row(
      mainAxisSize: expand ? MainAxisSize.max : MainAxisSize.min,
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        if (icon != null) ...[
          Icon(icon, size: iconSize, color: color),
          SizedBox(width: gap),
        ],
        Flexible(
          child: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis),
        ),
        if (trailingIcon != null) ...[
          SizedBox(width: gap),
          Icon(trailingIcon, size: iconSize - 1, color: color),
        ],
      ],
    );
  }

  Widget _shad(BuildContext context) {
    final p = ColectoPalette.of(context);
    final size = compact ? shad.ShadButtonSize.sm : shad.ShadButtonSize.regular;
    final leading = icon == null ? null : Icon(icon, size: compact ? 14 : 15);
    final trailing = trailingIcon == null
        ? null
        : Icon(trailingIcon, size: compact ? 13 : 14);
    final child = Text(label, maxLines: 1, overflow: TextOverflow.ellipsis);
    final width = expand ? double.infinity : null;
    return switch (variant) {
      ColectoButtonVariant.primary => shad.ShadButton(
        onPressed: onPressed,
        enabled: onPressed != null,
        size: size,
        width: width,
        leading: leading,
        trailing: trailing,
        hoverBackgroundColor: p.accentHi,
        child: child,
      ),
      ColectoButtonVariant.secondary => shad.ShadButton.secondary(
        onPressed: onPressed,
        enabled: onPressed != null,
        size: size,
        width: width,
        leading: leading,
        trailing: trailing,
        hoverBackgroundColor: p.controlHover,
        child: child,
      ),
      ColectoButtonVariant.ghost => shad.ShadButton.ghost(
        onPressed: onPressed,
        enabled: onPressed != null,
        size: size,
        width: width,
        leading: leading,
        trailing: trailing,
        foregroundColor: p.ink2,
        hoverForegroundColor: p.ink,
        hoverBackgroundColor: p.raise,
        child: child,
      ),
      ColectoButtonVariant.danger => shad.ShadButton.destructive(
        onPressed: onPressed,
        enabled: onPressed != null,
        size: size,
        width: width,
        leading: leading,
        trailing: trailing,
        child: child,
      ),
      ColectoButtonVariant.text => shad.ShadButton.link(
        onPressed: onPressed,
        enabled: onPressed != null,
        size: size,
        leading: leading,
        trailing: trailing,
        padding: EdgeInsets.zero,
        foregroundColor: p.accent,
        hoverForegroundColor: p.accentHi,
        child: child,
      ),
    };
  }

  Widget _fluent(BuildContext context) {
    final p = ColectoPalette.of(context);
    switch (variant) {
      case ColectoButtonVariant.primary:
        return fl.FilledButton(
          onPressed: onPressed,
          child: _content(null, gap: 8, iconSize: 14),
        );
      case ColectoButtonVariant.text:
        return fl.HyperlinkButton(
          onPressed: onPressed,
          child: _content(null, gap: 6, iconSize: 14),
        );
      case ColectoButtonVariant.ghost:
        return fl.Button(
          onPressed: onPressed,
          style: fl.ButtonStyle(
            backgroundColor: WidgetStateProperty.resolveWith(
              (s) => s.contains(WidgetState.hovered)
                  ? p.ink.withValues(alpha: 0.06)
                  : const Color(0x00000000),
            ),
            shape: const WidgetStatePropertyAll(
              RoundedRectangleBorder(
                borderRadius: BorderRadius.all(Radius.circular(4)),
              ),
            ),
          ),
          child: _content(null, gap: 8, iconSize: 14),
        );
      case ColectoButtonVariant.danger:
        return fl.Button(
          onPressed: onPressed,
          style: fl.ButtonStyle(
            foregroundColor: WidgetStatePropertyAll(p.danger),
          ),
          child: _content(p.danger, gap: 8, iconSize: 14),
        );
      case ColectoButtonVariant.secondary:
        return fl.Button(
          onPressed: onPressed,
          child: _content(null, gap: 8, iconSize: 14),
        );
    }
  }

  Widget _macos(BuildContext context) {
    final p = ColectoPalette.of(context);
    if (variant == ColectoButtonVariant.text ||
        variant == ColectoButtonVariant.ghost) {
      final color = variant == ColectoButtonVariant.text ? p.accent : p.ink2;
      return ColectoPressable(
        onTap: onPressed,
        pressScale: 1,
        focusRadius: 6,
        builder: (context, s) => AnimatedContainer(
          duration: ColectoMotion.hover,
          height: 26,
          padding: EdgeInsets.symmetric(
            horizontal: variant == ColectoButtonVariant.text ? 0 : 8,
          ),
          decoration: BoxDecoration(
            color: s.hovered && variant == ColectoButtonVariant.ghost
                ? p.ink.withValues(alpha: 0.07)
                : const Color(0x00000000),
            borderRadius: BorderRadius.circular(6),
          ),
          child: DefaultTextStyle.merge(
            style: kStyle(13, color: s.hovered ? p.ink : color),
            child: IconTheme.merge(
              data: IconThemeData(color: s.hovered ? p.ink : color),
              child: _content(null, gap: 5, iconSize: 14),
            ),
          ),
        ),
      );
    }
    final primary = variant == ColectoButtonVariant.primary;
    return mac.PushButton(
      controlSize: compact ? mac.ControlSize.regular : mac.ControlSize.large,
      onPressed: onPressed,
      secondary: !primary,
      color: primary
          ? p.accent
          : variant == ColectoButtonVariant.danger
          ? p.dangerBg
          : null,
      child: DefaultTextStyle.merge(
        style: TextStyle(
          color: primary
              ? p.accentInk
              : variant == ColectoButtonVariant.danger
              ? p.dangerInk
              : null,
        ),
        child: _content(primary ? p.accentInk : null, gap: 5, iconSize: 13),
      ),
    );
  }

  Widget _material(BuildContext context) {
    final p = ColectoPalette.of(context);
    final child = Text(label, maxLines: 1, overflow: TextOverflow.ellipsis);
    final iconWidget = icon == null ? null : Icon(icon, size: 18);
    final style = compact
        ? const ButtonStyle(
            visualDensity: VisualDensity.compact,
            padding: WidgetStatePropertyAll(
              EdgeInsets.symmetric(horizontal: 14),
            ),
          )
        : null;
    Widget withTrailing(Widget w) => trailingIcon == null
        ? w
        : Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              w,
              const SizedBox(width: 8),
              Icon(trailingIcon, size: 18),
            ],
          );
    return switch (variant) {
      ColectoButtonVariant.primary => FilledButton.icon(
        onPressed: onPressed,
        style: style,
        icon: iconWidget,
        label: withTrailing(child),
      ),
      ColectoButtonVariant.secondary => FilledButton.tonalIcon(
        onPressed: onPressed,
        style: style,
        icon: iconWidget,
        label: withTrailing(child),
      ),
      ColectoButtonVariant.danger => TextButton.icon(
        onPressed: onPressed,
        style: (style ?? const ButtonStyle()).copyWith(
          foregroundColor: WidgetStatePropertyAll(p.danger),
        ),
        icon: iconWidget,
        label: child,
      ),
      _ => TextButton.icon(
        onPressed: onPressed,
        style: style,
        icon: iconWidget,
        label: withTrailing(child),
      ),
    };
  }

  Widget _cupertino(BuildContext context) {
    final p = ColectoPalette.of(context);
    final size = compact
        ? CupertinoButtonSize.small
        : CupertinoButtonSize.medium;
    return switch (variant) {
      ColectoButtonVariant.primary => CupertinoButton.filled(
        onPressed: onPressed,
        sizeStyle: size,
        color: p.accent,
        child: DefaultTextStyle.merge(
          style: TextStyle(color: p.accentInk),
          child: _content(p.accentInk, gap: 6, iconSize: 17),
        ),
      ),
      ColectoButtonVariant.secondary => CupertinoButton.tinted(
        onPressed: onPressed,
        sizeStyle: size,
        color: p.accent,
        child: _content(null, gap: 6, iconSize: 17),
      ),
      ColectoButtonVariant.danger => CupertinoButton.tinted(
        onPressed: onPressed,
        sizeStyle: size,
        color: CupertinoColors.destructiveRed,
        child: DefaultTextStyle.merge(
          style: const TextStyle(color: CupertinoColors.destructiveRed),
          child: _content(CupertinoColors.destructiveRed, gap: 6, iconSize: 17),
        ),
      ),
      _ => CupertinoButton(
        onPressed: onPressed,
        sizeStyle: size,
        padding: variant == ColectoButtonVariant.text ? EdgeInsets.zero : null,
        child: _content(null, gap: 6, iconSize: 17),
      ),
    };
  }
}

/// An icon-only button: toolbar actions, close buttons, row actions.
///
/// shadcn `ShadIconButton.ghost` on Colecto, Fluent `IconButton`, `MacosIconButton`,
/// Material 3 `IconButton` and a plain `CupertinoButton`.
class ColectoIconButton extends StatelessWidget {
  const ColectoIconButton({
    super.key,
    required this.icon,
    required this.onPressed,
    this.tooltip,
    this.active = false,
    this.danger = false,
    this.size = 16,
    this.dimension,
    this.color,
  });

  final IconData icon;
  final VoidCallback? onPressed;
  final String? tooltip;
  final bool active;
  final bool danger;
  final double size;
  final double? dimension;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final enabled = onPressed != null;
    final style = colectoPlatformStyle(context);
    final fg = !enabled
        ? p.ink3.withValues(alpha: 0.45)
        : color ?? (danger ? p.danger : (active ? p.ink : p.ink2));
    Widget button;
    switch (style) {
      case ColectoPlatformStyle.desktop:
        final d = dimension ?? 32.0;
        button = shad.ShadIconButton.ghost(
          onPressed: onPressed,
          enabled: enabled,
          width: d,
          height: d,
          padding: EdgeInsets.zero,
          iconSize: size,
          foregroundColor: fg,
          hoverForegroundColor: danger ? p.danger : p.ink,
          hoverBackgroundColor: danger ? p.dangerBg : p.raise,
          backgroundColor: active ? p.raise : null,
          icon: Icon(icon, size: size),
        );
      case ColectoPlatformStyle.fluent:
        button = SizedBox.square(
          dimension: dimension ?? 32,
          child: fl.IconButton(
            onPressed: onPressed,
            icon: Icon(icon, size: size, color: fg),
          ),
        );
      case ColectoPlatformStyle.macos:
        button = mac.MacosIconButton(
          onPressed: onPressed,
          semanticLabel: tooltip,
          backgroundColor: active ? p.ink.withValues(alpha: 0.08) : null,
          hoverColor: p.ink.withValues(alpha: 0.08),
          borderRadius: BorderRadius.circular(6),
          padding: EdgeInsets.all(((dimension ?? 28) - size) / 2),
          boxConstraints: BoxConstraints.tightFor(
            width: dimension ?? 28,
            height: dimension ?? 28,
          ),
          icon: Icon(icon, size: size, color: fg),
        );
      case ColectoPlatformStyle.android:
        button = IconButton(
          onPressed: onPressed,
          isSelected: active,
          constraints: dimension == null
              ? null
              : BoxConstraints.tightFor(width: dimension, height: dimension),
          padding: dimension == null ? null : EdgeInsets.zero,
          iconSize: size + 4,
          icon: Icon(icon, color: color ?? (danger ? p.danger : null)),
        );
      case ColectoPlatformStyle.ios:
        button = CupertinoButton(
          onPressed: onPressed,
          padding: EdgeInsets.zero,
          minimumSize: Size.square(dimension ?? 44),
          child: Icon(
            icon,
            size: size + 4,
            color:
                color ?? (danger ? CupertinoColors.destructiveRed : p.accent),
          ),
        );
    }
    if (tooltip == null || tooltip!.isEmpty) return button;
    return Tooltip(message: tooltip!, child: button);
  }
}
