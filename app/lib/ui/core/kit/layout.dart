part of 'kit.dart';

/// A settings page head: a 24 px wide-width title and a short lead.
class ColectoPageHead extends StatelessWidget {
  const ColectoPageHead({
    super.key,
    required this.title,
    this.description,
    this.action,
  });

  final String title;
  final String? description;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 24),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                ColectoText.title(title),
                if (description != null) ...[
                  const SizedBox(height: 8),
                  ColectoParagraph(description!),
                ],
              ],
            ),
          ),
          if (action != null) ...[const SizedBox(width: 16), action!],
        ],
      ),
    );
  }
}

/// Colecto's `.settings-card`: not a card at all, but a section divided from
/// the one above by a hairline.
class ColectoSection extends StatelessWidget {
  const ColectoSection({
    super.key,
    this.title,
    this.description,
    this.trailing,
    required this.children,
  });

  final String? title;
  final String? description;
  final Widget? trailing;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Container(
      padding: const EdgeInsets.only(top: 22, bottom: 24),
      decoration: BoxDecoration(
        border: Border(top: BorderSide(color: p.lineSoft)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (title != null)
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      ColectoText.heading(title!),
                      if (description != null) ...[
                        const SizedBox(height: 6),
                        ColectoParagraph(description!),
                      ],
                    ],
                  ),
                ),
                if (trailing != null) ...[
                  const SizedBox(width: 24),
                  Padding(
                    padding: const EdgeInsets.only(top: 2),
                    child: trailing!,
                  ),
                ],
              ],
            ),
          if (title != null && children.isNotEmpty) const SizedBox(height: 16),
          ...children,
        ],
      ),
    );
  }
}

/// Colecto's `.setting-row`: a title and description with a control on the
/// right (usually a switch).
class ColectoSettingRow extends StatelessWidget {
  const ColectoSettingRow({
    super.key,
    required this.title,
    this.description,
    required this.trailing,
  });

  final String title;
  final String? description;
  final Widget trailing;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                ColectoText(title, role: ColectoTextRole.bodyStrong),
                if (description != null) ...[
                  const SizedBox(height: 4),
                  ColectoParagraph(
                    description!,
                    small: true,
                    tone: ColectoTone.secondary,
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(width: 24),
          Padding(padding: const EdgeInsets.only(top: 1), child: trailing),
        ],
      ),
    );
  }
}

/// A small field label (Colecto's `legend` / `.row-label`).
class ColectoLabel extends StatelessWidget {
  const ColectoLabel(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: ColectoText.label(text),
    );
  }
}
