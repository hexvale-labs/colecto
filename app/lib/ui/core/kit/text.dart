part of 'kit.dart';

/// What a piece of text is, not how big it is. Each design maps roles onto
/// its own type ramp (Colecto's, the Windows ramp, macOS's, Material 3's
/// and iOS's).
enum ColectoTextRole {
  /// The largest text on screen: a message subject.
  display,

  /// A page title (a Settings section).
  title,

  /// A group heading inside a page.
  heading,
  body,
  bodyStrong,

  /// Control and field labels.
  label,

  /// Metadata and help text.
  caption,
}

/// The colour role of a piece of text.
enum ColectoTone { primary, secondary, muted, accent, danger }

Color colectoToneColor(ColectoPalette p, ColectoTone tone) => switch (tone) {
  ColectoTone.primary => p.ink,
  ColectoTone.secondary => p.ink2,
  ColectoTone.muted => p.ink3,
  ColectoTone.accent => p.accentHi,
  ColectoTone.danger => p.danger,
};

/// The text style for [role] in the active design.
TextStyle colectoType(
  BuildContext context,
  ColectoTextRole role, {
  ColectoTone tone = ColectoTone.primary,
}) {
  final p = ColectoPalette.of(context);
  final c = colectoToneColor(p, tone);
  // (size, weight, width, height, letterSpacing)
  final (double, double, double?, double, double?) spec =
      switch (colectoPlatformStyle(context)) {
        ColectoPlatformStyle.desktop => switch (role) {
          ColectoTextRole.display => (26, 660, 108, 1.2, -0.55),
          ColectoTextRole.title => (24, 640, 108, 1.2, -0.48),
          ColectoTextRole.heading => (15.5, 640, null, 1.35, -0.15),
          ColectoTextRole.body => (14, 450, null, 1.55, null),
          ColectoTextRole.bodyStrong => (14, 600, null, 1.45, null),
          ColectoTextRole.label => (13, 560, null, 1.35, null),
          ColectoTextRole.caption => (12.5, 450, null, 1.45, null),
        },
        ColectoPlatformStyle.fluent => switch (role) {
          ColectoTextRole.display => (28, 600, null, 1.28, null),
          ColectoTextRole.title => (20, 600, null, 1.4, null),
          ColectoTextRole.heading => (14, 600, null, 1.43, null),
          ColectoTextRole.body => (14, 400, null, 1.43, null),
          ColectoTextRole.bodyStrong => (14, 600, null, 1.43, null),
          ColectoTextRole.label => (14, 400, null, 1.43, null),
          ColectoTextRole.caption => (12, 400, null, 1.33, null),
        },
        ColectoPlatformStyle.macos => switch (role) {
          ColectoTextRole.display => (22, 700, null, 1.25, null),
          ColectoTextRole.title => (20, 700, null, 1.25, null),
          ColectoTextRole.heading => (13, 700, null, 1.35, null),
          ColectoTextRole.body => (13, 400, null, 1.45, null),
          ColectoTextRole.bodyStrong => (13, 600, null, 1.4, null),
          ColectoTextRole.label => (13, 500, null, 1.35, null),
          ColectoTextRole.caption => (11, 400, null, 1.35, null),
        },
        ColectoPlatformStyle.android => switch (role) {
          ColectoTextRole.display => (24, 500, null, 1.33, null),
          ColectoTextRole.title => (22, 500, null, 1.27, null),
          ColectoTextRole.heading => (16, 600, null, 1.5, 0.15),
          ColectoTextRole.body => (15, 400, null, 1.5, 0.25),
          ColectoTextRole.bodyStrong => (15, 600, null, 1.5, 0.15),
          ColectoTextRole.label => (14, 500, null, 1.43, 0.1),
          ColectoTextRole.caption => (12.5, 400, null, 1.33, 0.4),
        },
        ColectoPlatformStyle.ios => switch (role) {
          ColectoTextRole.display => (22, 700, null, 1.27, -0.26),
          ColectoTextRole.title => (28, 700, null, 1.21, 0.38),
          ColectoTextRole.heading => (17, 600, null, 1.29, -0.41),
          ColectoTextRole.body => (17, 400, null, 1.29, -0.41),
          ColectoTextRole.bodyStrong => (17, 600, null, 1.29, -0.41),
          ColectoTextRole.label => (15, 400, null, 1.33, -0.24),
          ColectoTextRole.caption => (13, 400, null, 1.38, -0.08),
        },
      };
  return kStyle(
    spec.$1,
    weight: spec.$2,
    width: spec.$3,
    height: spec.$4,
    letterSpacing: spec.$5,
    color: c,
  );
}

/// One line (or a few) of interface text in a [ColectoTextRole].
class ColectoText extends StatelessWidget {
  const ColectoText(
    this.text, {
    super.key,
    this.role = ColectoTextRole.body,
    this.tone = ColectoTone.primary,
    this.maxLines,
    this.align,
  });

  const ColectoText.display(this.text, {super.key, this.maxLines, this.align})
    : role = ColectoTextRole.display,
      tone = ColectoTone.primary;

  const ColectoText.title(this.text, {super.key, this.maxLines, this.align})
    : role = ColectoTextRole.title,
      tone = ColectoTone.primary;

  const ColectoText.heading(this.text, {super.key, this.maxLines, this.align})
    : role = ColectoTextRole.heading,
      tone = ColectoTone.primary;

  const ColectoText.label(
    this.text, {
    super.key,
    this.tone = ColectoTone.secondary,
    this.maxLines,
    this.align,
  }) : role = ColectoTextRole.label;

  const ColectoText.caption(
    this.text, {
    super.key,
    this.tone = ColectoTone.muted,
    this.maxLines,
    this.align,
  }) : role = ColectoTextRole.caption;

  final String text;
  final ColectoTextRole role;
  final ColectoTone tone;
  final int? maxLines;
  final TextAlign? align;

  @override
  Widget build(BuildContext context) {
    return Text(
      text,
      maxLines: maxLines,
      textAlign: align,
      overflow: maxLines == null ? null : TextOverflow.ellipsis,
      style: colectoType(context, role, tone: tone),
    );
  }
}

/// Running prose (descriptions, empty states, help): body text kept to a
/// readable measure.
class ColectoParagraph extends StatelessWidget {
  const ColectoParagraph(
    this.text, {
    super.key,
    this.tone = ColectoTone.secondary,
    this.align,
    this.small = false,
    this.maxWidth = 560,
  });

  final String text;
  final ColectoTone tone;
  final TextAlign? align;
  final bool small;
  final double maxWidth;

  @override
  Widget build(BuildContext context) {
    final base = colectoType(
      context,
      small ? ColectoTextRole.caption : ColectoTextRole.body,
      tone: tone,
    );
    return ConstrainedBox(
      constraints: BoxConstraints(maxWidth: maxWidth),
      child: Text(
        text,
        textAlign: align,
        style: base.copyWith(height: small ? 1.45 : 1.55),
      ),
    );
  }
}
