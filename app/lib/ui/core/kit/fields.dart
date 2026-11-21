part of 'kit.dart';

/// A labelled text input.
///
/// `ShadInput` on Colecto, Fluent `TextBox`, `MacosTextField`, a Material 3
/// outlined `TextField` (label inside) or `CupertinoTextField`. Passwords
/// get a show/hide toggle on every design.
class ColectoTextField extends StatefulWidget {
  const ColectoTextField({
    super.key,
    this.controller,
    this.focusNode,
    this.hintText,
    this.labelText,
    this.helperText,
    this.obscureText = false,
    this.maxLines = 1,
    this.minLines,
    this.onChanged,
    this.onSubmitted,
    this.prefixIcon,
    this.suffix,
    this.readOnly = false,
    this.keyboardType,
    this.textInputAction,
    this.autofocus = false,
    this.autofillHints,
    this.enabled = true,
    this.monospace = false,
    this.height,
  });

  final TextEditingController? controller;
  final FocusNode? focusNode;
  final String? hintText;
  final String? labelText;
  final String? helperText;
  final bool obscureText;
  final int? maxLines;
  final int? minLines;
  final ValueChanged<String>? onChanged;
  final ValueChanged<String>? onSubmitted;
  final IconData? prefixIcon;
  final Widget? suffix;
  final bool readOnly;
  final TextInputType? keyboardType;
  final TextInputAction? textInputAction;
  final bool autofocus;
  final Iterable<String>? autofillHints;
  final bool enabled;
  final bool monospace;
  final double? height;

  @override
  State<ColectoTextField> createState() => _ColectoTextFieldState();
}

class _ColectoTextFieldState extends State<ColectoTextField> {
  FocusNode? _ownFocus;
  FocusNode get _focus => widget.focusNode ?? (_ownFocus ??= FocusNode());
  bool _hasFocus = false;
  bool _reveal = false;

  @override
  void initState() {
    super.initState();
    _focus.addListener(_onFocus);
  }

  @override
  void didUpdateWidget(ColectoTextField old) {
    super.didUpdateWidget(old);
    if (old.focusNode != widget.focusNode) {
      (old.focusNode ?? _ownFocus)?.removeListener(_onFocus);
      _focus.addListener(_onFocus);
    }
  }

  void _onFocus() {
    if (_hasFocus != _focus.hasFocus) {
      setState(() => _hasFocus = _focus.hasFocus);
    }
  }

  @override
  void dispose() {
    _focus.removeListener(_onFocus);
    _ownFocus?.dispose();
    super.dispose();
  }

  Widget? get _reveal_ => widget.obscureText
      ? ColectoIconButton(
          icon: _reveal ? LucideIcons.eyeOff : LucideIcons.eye,
          tooltip: _reveal ? 'Hide' : 'Show',
          size: 15,
          dimension: 28,
          onPressed: () => setState(() => _reveal = !_reveal),
        )
      : null;

  Widget? get _suffix => widget.suffix ?? _reveal_;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final style = colectoPlatformStyle(context);
    final lines = widget.obscureText ? 1 : widget.maxLines;
    // Library fields don't all read the Colecto ink, so set it explicitly.
    final mono = TextStyle(
      color: p.ink,
      fontFamily: widget.monospace ? 'monospace' : null,
    );
    final prefix = widget.prefixIcon == null
        ? null
        : Padding(
            padding: const EdgeInsets.only(left: 8),
            child: Icon(widget.prefixIcon, size: 15, color: p.ink3),
          );
    final Widget field;
    switch (style) {
      case ColectoPlatformStyle.android:
        // Material 3 puts the label inside the outlined field.
        return TextField(
          controller: widget.controller,
          focusNode: _focus,
          obscureText: widget.obscureText && !_reveal,
          maxLines: lines,
          minLines: widget.minLines,
          onChanged: widget.onChanged,
          onSubmitted: widget.onSubmitted,
          readOnly: widget.readOnly,
          enabled: widget.enabled,
          autofocus: widget.autofocus,
          keyboardType: widget.keyboardType,
          textInputAction: widget.textInputAction,
          autofillHints: widget.autofillHints,
          style: mono,
          decoration: InputDecoration(
            border: const OutlineInputBorder(),
            labelText: widget.labelText,
            hintText: widget.hintText,
            helperText: widget.helperText,
            helperMaxLines: 3,
            prefixIcon: widget.prefixIcon == null
                ? null
                : Icon(widget.prefixIcon),
            suffixIcon: _suffix,
          ),
        );
      case ColectoPlatformStyle.ios:
        field = CupertinoTextField(
          controller: widget.controller,
          focusNode: _focus,
          placeholder: widget.hintText,
          obscureText: widget.obscureText && !_reveal,
          maxLines: lines,
          minLines: widget.minLines,
          onChanged: widget.onChanged,
          onSubmitted: widget.onSubmitted,
          readOnly: widget.readOnly,
          enabled: widget.enabled,
          autofocus: widget.autofocus,
          keyboardType: widget.keyboardType,
          textInputAction: widget.textInputAction,
          autofillHints: widget.autofillHints,
          style: mono,
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 11),
          prefix: prefix,
          suffix: _suffix,
        );
      case ColectoPlatformStyle.macos:
        field = mac.MacosTextField(
          controller: widget.controller,
          focusNode: _focus,
          placeholder: widget.hintText,
          placeholderStyle: TextStyle(color: p.ink3),
          obscureText: widget.obscureText && !_reveal,
          maxLines: lines,
          minLines: widget.minLines,
          onChanged: widget.onChanged,
          onSubmitted: widget.onSubmitted,
          readOnly: widget.readOnly,
          enabled: widget.enabled,
          autofocus: widget.autofocus,
          keyboardType: widget.keyboardType,
          textInputAction: widget.textInputAction,
          autofillHints: widget.autofillHints,
          style: mono,
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 5),
          prefix: prefix,
          suffix: _suffix,
        );
      case ColectoPlatformStyle.fluent:
        field = fl.TextBox(
          controller: widget.controller,
          focusNode: _focus,
          placeholder: widget.hintText,
          obscureText: widget.obscureText && !_reveal,
          maxLines: lines,
          minLines: widget.minLines,
          onChanged: widget.onChanged,
          onSubmitted: widget.onSubmitted,
          readOnly: widget.readOnly,
          enabled: widget.enabled,
          autofocus: widget.autofocus,
          keyboardType: widget.keyboardType,
          textInputAction: widget.textInputAction,
          autofillHints: widget.autofillHints,
          style: mono,
          prefix: prefix,
          suffix: _suffix,
        );
      case ColectoPlatformStyle.desktop:
        field = shad.ShadInput(
          controller: widget.controller,
          focusNode: _focus,
          placeholder: widget.hintText == null ? null : Text(widget.hintText!),
          obscureText: widget.obscureText && !_reveal,
          maxLines: lines,
          minLines: widget.minLines,
          onChanged: widget.onChanged,
          onSubmitted: widget.onSubmitted,
          readOnly: widget.readOnly,
          enabled: widget.enabled,
          autofocus: widget.autofocus,
          keyboardType: widget.keyboardType,
          textInputAction: widget.textInputAction,
          autofillHints: widget.autofillHints?.toList() ?? const [],
          style: mono,
          leading: widget.prefixIcon == null
              ? null
              : Icon(widget.prefixIcon, size: 15, color: p.ink3),
          trailing: _suffix,
        );
    }

    if (widget.labelText == null && widget.helperText == null) return field;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (widget.labelText != null) ...[
          ColectoText.label(widget.labelText!),
          const SizedBox(height: 6),
        ],
        field,
        if (widget.helperText != null) ...[
          const SizedBox(height: 6),
          ColectoParagraph(
            widget.helperText!,
            tone: ColectoTone.muted,
            small: true,
          ),
        ],
      ],
    );
  }
}
