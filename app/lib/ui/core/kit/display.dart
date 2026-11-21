part of 'kit.dart';

/// Colecto's `.count` and `.tag`: a small tabular pill.
class ColectoBadge extends StatelessWidget {
  const ColectoBadge({
    super.key,
    required this.text,
    this.accent = false,
    this.danger = false,
  });

  final String text;
  final bool accent;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final bg = danger ? p.dangerBg : (accent ? p.select : p.control);
    final fg = danger ? p.dangerInk : (accent ? p.accentHi : p.ink2);
    return Container(
      constraints: const BoxConstraints(minWidth: 18),
      padding: const EdgeInsets.symmetric(horizontal: 6),
      height: 18,
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(5),
      ),
      child: Center(
        widthFactor: 1,
        child: Text(
          text,
          style: kStyle(11.5, weight: 600, color: fg, features: tabularFigures),
        ),
      ),
    );
  }
}

/// Colecto's `.kbd`.
class ColectoKbd extends StatelessWidget {
  const ColectoKbd(this.text, {super.key, this.raised = false});

  final String text;
  final bool raised;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Container(
      constraints: const BoxConstraints(minWidth: 20),
      height: 20,
      padding: const EdgeInsets.symmetric(horizontal: 5),
      decoration: BoxDecoration(
        color: raised ? p.raise : p.control,
        borderRadius: BorderRadius.circular(5),
      ),
      child: Center(
        widthFactor: 1,
        child: Text(text, style: kStyle(11.5, weight: 500, color: p.ink2)),
      ),
    );
  }
}

/// Platform modifier label for shortcut hints.
String colectoModKey(BuildContext context) =>
    Theme.of(context).platform == TargetPlatform.macOS ||
        Theme.of(context).platform == TargetPlatform.iOS
    ? '⌘'
    : 'Ctrl';

/// A floating surface (palette, dialogs, connect card).
class ColectoCard extends StatelessWidget {
  const ColectoCard({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.all(14),
    this.radius = 12,
  });

  final Widget child;
  final EdgeInsetsGeometry padding;
  final double radius;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Container(
      padding: padding,
      decoration: BoxDecoration(
        color: p.float,
        borderRadius: BorderRadius.circular(radius),
        border: Border.all(color: p.line),
        boxShadow: p.floatShadow,
      ),
      child: child,
    );
  }
}

/// A sender avatar with a stable hue per correspondent.
class ColectoAvatar extends StatelessWidget {
  const ColectoAvatar({
    super.key,
    required this.name,
    this.hueKey,
    this.size = 28,
  });

  final String name;
  final String? hueKey;
  final double size;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final (bg, fg) = p.avatarColors(
      ColectoPalette.hueFor(hueKey?.isNotEmpty == true ? hueKey! : name),
    );
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(color: bg, shape: BoxShape.circle),
      child: Text(
        initialOf(name),
        style: kStyle(size * 0.42, weight: 640, color: fg, height: 1),
      ),
    );
  }
}

/// Colecto's mint status dot; its glow is never clipped.
class ColectoStatusDot extends StatelessWidget {
  const ColectoStatusDot({super.key, required this.on, this.error = false});

  final bool on;
  final bool error;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final color = error ? p.danger : (on ? p.statusOnline : p.ink3);
    return Container(
      width: 6,
      height: 6,
      decoration: BoxDecoration(
        color: color,
        shape: BoxShape.circle,
        boxShadow: on && !error
            ? [
                BoxShadow(
                  color: p.accent.withValues(alpha: 0.55),
                  blurRadius: 5,
                ),
              ]
            : null,
      ),
    );
  }
}

/// Colecto's `.pulse`: a small breathing dot for background activity.
class ColectoPulse extends StatefulWidget {
  const ColectoPulse({super.key, this.size = 6});

  final double size;

  @override
  State<ColectoPulse> createState() => _ColectoPulseState();
}

class _ColectoPulseState extends State<ColectoPulse>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1200),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (MediaQuery.maybeDisableAnimationsOf(context) ?? false) {
      _c.stop();
    } else if (!_c.isAnimating) {
      _c.repeat(reverse: true);
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
    return FadeTransition(
      opacity: Tween(
        begin: 1.0,
        end: 0.25,
      ).animate(CurvedAnimation(parent: _c, curve: Curves.easeInOut)),
      child: Container(
        width: widget.size,
        height: widget.size,
        decoration: BoxDecoration(color: p.accent, shape: BoxShape.circle),
      ),
    );
  }
}

/// A platform-native busy indicator.
class ColectoSpinner extends StatelessWidget {
  const ColectoSpinner({super.key, this.size = 14});

  final double size;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return switch (colectoPlatformStyle(context)) {
      ColectoPlatformStyle.ios => CupertinoActivityIndicator(radius: size / 2),
      _ => SizedBox.square(
        dimension: size,
        child: CircularProgressIndicator(
          strokeWidth: 1.8,
          color: p.accent,
          strokeCap: StrokeCap.round,
        ),
      ),
    };
  }
}

/// Colecto's `.body-skeleton`: three breathing lines.
class ColectoSkeleton extends StatefulWidget {
  const ColectoSkeleton({super.key, this.lines = 3});

  final int lines;

  @override
  State<ColectoSkeleton> createState() => _ColectoSkeletonState();
}

class _ColectoSkeletonState extends State<ColectoSkeleton>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1400),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (MediaQuery.maybeDisableAnimationsOf(context) ?? false) {
      _c.stop();
    } else if (!_c.isAnimating) {
      _c.repeat();
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
    const widths = [1.0, 0.86, 0.58, 0.72];
    return AnimatedBuilder(
      animation: _c,
      builder: (context, _) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          for (var i = 0; i < widget.lines; i++)
            Padding(
              padding: const EdgeInsets.only(bottom: 12),
              child: FractionallySizedBox(
                widthFactor: widths[i % widths.length],
                child: Opacity(
                  opacity:
                      0.625 +
                      0.375 * math.cos(2 * math.pi * (_c.value - i * 0.085)),
                  child: Container(
                    height: 12,
                    decoration: BoxDecoration(
                      color: p.raise,
                      borderRadius: BorderRadius.circular(6),
                    ),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// Colecto's `.settings-notice`: a mint-tinted note.
class ColectoNotice extends StatelessWidget {
  const ColectoNotice({super.key, required this.child, this.icon});

  final Widget child;
  final IconData? icon;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: p.select,
        borderRadius: BorderRadius.circular(9),
      ),
      child: DefaultTextStyle.merge(
        style: kStyle(13.5, color: p.accentHi, height: 1.5),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (icon != null) ...[
              Padding(
                padding: const EdgeInsets.only(top: 2),
                child: Icon(icon, size: 15, color: p.accentHi),
              ),
              const SizedBox(width: 10),
            ],
            Expanded(child: child),
          ],
        ),
      ),
    );
  }
}

/// Colecto's `.error-box`.
class ColectoErrorBox extends StatelessWidget {
  const ColectoErrorBox({super.key, required this.message, this.action});

  final String message;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: p.dangerBg,
        borderRadius: BorderRadius.circular(9),
      ),
      child: Wrap(
        spacing: 14,
        runSpacing: 8,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          SelectableText(
            message,
            style: kStyle(13.5, color: p.dangerInk, height: 1.45),
          ),
          ?action,
        ],
      ),
    );
  }
}

/// A hairline divider.
class ColectoHairline extends StatelessWidget {
  const ColectoHairline({super.key, this.vertical = false, this.soft = true});

  final bool vertical;
  final bool soft;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = soft ? p.lineSoft : p.line;
    return vertical
        ? Container(width: 1, color: c)
        : Container(height: 1, color: c);
  }
}
