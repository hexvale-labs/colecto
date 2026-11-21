import 'dart:convert';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/mail_models.dart';
import '../../core/theme/colecto_palette.dart';
import '../../core/kit/kit.dart';
import '../shell/mail_controller.dart';

/// Colecto's composer: it lives where the reader does (a full-screen sheet on
/// phones), with hairline From/To/Cc/Subject rows, a roomy body, and a sticky
/// footer that always says what happened to your writing. Closing with
/// unsaved changes keeps them as a draft, so nothing is lost.
class ComposerView extends StatefulWidget {
  const ComposerView({
    super.key,
    required this.controller,
    required this.request,
    required this.onClose,
    this.phone = false,
  });

  final MailController controller;
  final ComposeRequest request;
  final VoidCallback onClose;
  final bool phone;

  @override
  State<ComposerView> createState() => _ComposerViewState();
}

enum _SaveState { idle, saving, saved, sending, sent }

class _ComposerViewState extends State<ComposerView> {
  late final TextEditingController _to;
  late final TextEditingController _cc;
  late final TextEditingController _subject;
  late final TextEditingController _body;
  final _toFocus = FocusNode();
  final _bodyFocus = FocusNode();
  final List<OutgoingAttachment> _attachments = [];
  late String _baseline;
  String? _draftId;
  _SaveState _state = _SaveState.idle;
  bool _discarded = false;

  MailController get c => widget.controller;

  @override
  void initState() {
    super.initState();
    final r = widget.request;
    _draftId = r.draftId;
    _to = TextEditingController(text: r.to);
    _cc = TextEditingController(text: r.cc);
    _subject = TextEditingController(text: r.subject);
    _body = TextEditingController(text: r.body);
    _baseline = _snapshot();
    for (final t in [_to, _cc, _subject, _body]) {
      t.addListener(_changed);
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      // Replies start in the body, above the quote; new mail starts at To.
      if (r.to.isEmpty) {
        _toFocus.requestFocus();
      } else {
        _bodyFocus.requestFocus();
        _body.selection = const TextSelection.collapsed(offset: 0);
      }
    });
  }

  String _snapshot() =>
      [_to.text, _cc.text, _subject.text, _body.text].join('\u0000');

  bool get _dirty =>
      _state != _SaveState.sent &&
      (_snapshot() != _baseline || _attachments.isNotEmpty);

  bool get _hasContent =>
      _to.text.trim().isNotEmpty ||
      _subject.text.trim().isNotEmpty ||
      _body.text.trim().isNotEmpty;

  void _changed() {
    if (_state == _SaveState.saved && _snapshot() != _baseline) {
      setState(() => _state = _SaveState.idle);
    } else {
      setState(() {});
    }
  }

  @override
  void dispose() {
    // Leaving the composer (another message opened, the window changed)
    // never loses writing: unsaved changes become a draft.
    if (!_discarded && _dirty && _hasContent) {
      c.saveDraft(_input());
    }
    for (final t in [_to, _cc, _subject, _body]) {
      t.dispose();
    }
    _toFocus.dispose();
    _bodyFocus.dispose();
    super.dispose();
  }

  DraftInput _input() => DraftInput(
    id: _draftId,
    recipient: _to.text.trim(),
    cc: _cc.text.trim(),
    subject: _subject.text.trim(),
    body: _body.text,
    inReplyTo: widget.request.inReplyTo,
    references: widget.request.references,
    forwardOf: widget.request.forwardOf,
    attachments: List.unmodifiable(_attachments),
  );

  Future<void> _save() async {
    if (_state == _SaveState.saving || _state == _SaveState.sending) return;
    setState(() => _state = _SaveState.saving);
    final saved = await c.saveDraft(_input(), quiet: true);
    if (!mounted) return;
    setState(() {
      if (saved != null) {
        _draftId = saved.id;
        _baseline = _snapshot();
        _state = _SaveState.saved;
      } else {
        _state = _SaveState.idle;
      }
    });
  }

  Future<void> _send() async {
    if (_state == _SaveState.sending || _state == _SaveState.sent) return;
    if (_to.text.trim().isEmpty) {
      c.showToast('Add a recipient', isError: true);
      _toFocus.requestFocus();
      return;
    }
    if (_subject.text.trim().isEmpty) {
      final ok = await showColectoConfirm(
        context: context,
        title: 'Send without a subject?',
        message: 'Recipients will see “(No subject)”.',
        confirmLabel: 'Send anyway',
      );
      if (!ok || !mounted) return;
    }
    setState(() => _state = _SaveState.sending);
    final outcome = await c.sendMail(_input());
    if (!mounted) return;
    if (outcome != null) {
      setState(() => _state = _SaveState.sent);
      widget.onClose();
    } else {
      setState(() => _state = _SaveState.idle);
    }
  }

  Future<void> _close() async {
    if (_dirty && _hasContent) {
      await _save();
      if (!mounted) return;
      c.showToast('Draft saved', detail: 'Find it in Drafts.');
    }
    _discarded = true;
    widget.onClose();
  }

  Future<void> _discard() async {
    if (_dirty && _hasContent) {
      final ok = await showColectoConfirm(
        context: context,
        title: _draftId == null ? 'Discard this message?' : 'Discard changes?',
        message: _draftId == null
            ? 'What you have written will be lost.'
            : 'The draft keeps its last saved version.',
        confirmLabel: 'Discard',
        danger: true,
      );
      if (!ok || !mounted) return;
    }
    _discarded = true;
    widget.onClose();
  }

  Future<void> _attach() async {
    try {
      final files = await openFiles();
      if (files.isEmpty) return;
      final added = <OutgoingAttachment>[];
      for (final f in files) {
        final bytes = await f.readAsBytes();
        added.add(
          OutgoingAttachment(
            name: f.name,
            contentType: f.mimeType?.isNotEmpty == true
                ? f.mimeType!
                : 'application/octet-stream',
            data: base64Encode(bytes),
          ),
        );
      }
      if (mounted) setState(() => _attachments.addAll(added));
    } catch (e) {
      c.showToast('Could not attach files', detail: '$e', isError: true);
    }
  }

  String get _stateLabel => switch (_state) {
    _SaveState.sent => 'Accepted by mail server',
    _SaveState.sending => 'Sending…',
    _SaveState.saving => 'Saving draft…',
    _SaveState.saved => 'Draft saved',
    _SaveState.idle =>
      _dirty && _hasContent
          ? 'Unsaved changes'
          : (_draftId != null ? 'Saved in Drafts' : 'Not saved yet'),
  };

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final busy = _state == _SaveState.sending || _state == _SaveState.sent;
    final mod = Theme.of(context).platform == TargetPlatform.macOS;

    return CallbackShortcuts(
      bindings: {
        SingleActivator(LogicalKeyboardKey.enter, control: !mod, meta: mod):
            _send,
        SingleActivator(LogicalKeyboardKey.keyS, control: !mod, meta: mod):
            _save,
        const SingleActivator(LogicalKeyboardKey.escape): _close,
      },
      child: LayoutBuilder(
        builder: (context, constraints) {
          final narrow = constraints.maxWidth < 560;
          final side = narrow ? 18.0 : 40.0;
          // Material (not just a colour) so text fields work when the
          // composer is pushed as its own page on phones.
          return Material(
            color: p.bg,
            child: Column(
              children: [
                _toolbar(p, narrow),
                Expanded(
                  child: Center(
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 820),
                      child: ListView(
                        padding: EdgeInsets.symmetric(horizontal: side),
                        children: [
                          _row(
                            p,
                            'From',
                            Text(
                              c.currentAccountInfo?.address ?? c.activeAccount,
                              overflow: TextOverflow.ellipsis,
                              style: kStyle(13.5, weight: 560, color: p.ink),
                            ),
                          ),
                          _row(
                            p,
                            'To',
                            _field(
                              p,
                              _to,
                              focus: _toFocus,
                              hint: 'name@example.com',
                              email: true,
                              enabled: !busy,
                            ),
                          ),
                          _row(
                            p,
                            'Cc',
                            _field(p, _cc, email: true, enabled: !busy),
                          ),
                          _row(
                            p,
                            'Subject',
                            _field(p, _subject, enabled: !busy),
                          ),
                          TextField(
                            controller: _body,
                            focusNode: _bodyFocus,
                            enabled: !busy,
                            maxLines: null,
                            minLines: narrow ? 12 : 16,
                            keyboardType: TextInputType.multiline,
                            textCapitalization: TextCapitalization.sentences,
                            cursorColor: p.accent,
                            style: kStyle(
                              colectoTouch(context) ? 16 : 15.5,
                              color: p.bodyInk,
                              height: 1.68,
                            ),
                            decoration: InputDecoration(
                              border: InputBorder.none,
                              hintText: 'Write your message',
                              hintStyle: kStyle(15.5, color: p.ink3),
                              contentPadding: const EdgeInsets.symmetric(
                                vertical: 20,
                              ),
                            ),
                          ),
                          if (_attachments.isNotEmpty ||
                              widget.request.forwardOf != null)
                            _attachmentsBlock(p),
                          const SizedBox(height: 24),
                        ],
                      ),
                    ),
                  ),
                ),
                _footer(
                  p,
                  side,
                  narrow,
                  busy,
                  mod,
                  tight: constraints.maxWidth < 720,
                ),
              ],
            ),
          );
        },
      ),
    );
  }

  Widget _toolbar(ColectoPalette p, bool narrow) {
    return Container(
      height: widget.phone ? 54 : 48,
      padding: EdgeInsets.only(left: narrow ? 8 : 16, right: 12),
      child: Row(
        children: [
          ColectoIconButton(
            icon: widget.phone ? LucideIcons.chevronDown : LucideIcons.x,
            tooltip: 'Close (Esc)',
            size: 18,
            onPressed: _close,
          ),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              _draftId != null && widget.request.heading == 'New message'
                  ? 'Your draft'
                  : widget.request.heading,
              overflow: TextOverflow.ellipsis,
              style: kStyle(15, weight: 640, color: p.ink),
            ),
          ),
          ColectoIconButton(
            icon: LucideIcons.trash2,
            tooltip: 'Discard',
            danger: true,
            onPressed: _discard,
          ),
        ],
      ),
    );
  }

  Widget _row(ColectoPalette p, String label, Widget field) {
    return _FocusHairline(
      child: Row(
        children: [
          SizedBox(
            width: 68,
            child: Text(label, style: kStyle(13, weight: 540, color: p.ink3)),
          ),
          Expanded(child: field),
        ],
      ),
    );
  }

  Widget _field(
    ColectoPalette p,
    TextEditingController ctrl, {
    FocusNode? focus,
    String? hint,
    bool email = false,
    bool enabled = true,
  }) {
    return TextField(
      controller: ctrl,
      focusNode: focus,
      enabled: enabled,
      autocorrect: !email,
      enableSuggestions: !email,
      keyboardType: email ? TextInputType.emailAddress : TextInputType.text,
      textInputAction: TextInputAction.next,
      autofillHints: email ? const [AutofillHints.email] : null,
      cursorColor: p.accent,
      style: kStyle(colectoTouch(context) ? 16 : 14, color: p.ink),
      decoration: InputDecoration(
        isCollapsed: true,
        border: InputBorder.none,
        hintText: hint,
        hintStyle: kStyle(14, color: p.ink3),
        contentPadding: const EdgeInsets.symmetric(vertical: 13),
      ),
    );
  }

  Widget _attachmentsBlock(ColectoPalette p) {
    return Padding(
      padding: const EdgeInsets.only(top: 8, bottom: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (widget.request.forwardOf != null)
            Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Text(
                'The original message’s attachments are included when you send.',
                style: kStyle(12.5, color: p.ink3),
              ),
            ),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              for (var i = 0; i < _attachments.length; i++)
                Container(
                  height: 40,
                  padding: const EdgeInsets.only(left: 12, right: 4),
                  decoration: BoxDecoration(
                    color: p.raise,
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(LucideIcons.paperclip, size: 15, color: p.ink3),
                      const SizedBox(width: 8),
                      ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 220),
                        child: Text(
                          _attachments[i].name,
                          overflow: TextOverflow.ellipsis,
                          style: kStyle(13, weight: 540, color: p.ink),
                        ),
                      ),
                      const SizedBox(width: 8),
                      Text(
                        _size(_attachments[i].data),
                        style: kStyle(12, color: p.ink3),
                      ),
                      ColectoIconButton(
                        icon: LucideIcons.x,
                        tooltip: 'Remove ${_attachments[i].name}',
                        size: 13,
                        dimension: 28,
                        onPressed: () =>
                            setState(() => _attachments.removeAt(i)),
                      ),
                    ],
                  ),
                ),
            ],
          ),
          if (_attachments.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                'Attached files are sent but not saved with drafts.',
                style: kStyle(12.5, color: p.ink3),
              ),
            ),
        ],
      ),
    );
  }

  static String _size(String base64) {
    final bytes = (base64.length * 3) ~/ 4;
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1048576) return '${(bytes / 1024).round()} KB';
    return '${(bytes / 1048576).toStringAsFixed(1)} MB';
  }

  Widget _footer(
    ColectoPalette p,
    double side,
    bool narrow,
    bool busy,
    bool mod, {
    bool tight = false,
  }) {
    final bottom = MediaQuery.paddingOf(context).bottom;
    final pending = _state == _SaveState.idle && _dirty && _hasContent;
    return Container(
      decoration: BoxDecoration(
        color: p.bg,
        border: Border(top: BorderSide(color: p.lineSoft)),
      ),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 820),
          child: Padding(
            padding: EdgeInsets.fromLTRB(side - 11, 12, side, 12 + bottom),
            child: Row(
              children: [
                ColectoButton(
                  // Wide platform buttons (Cupertino, Material) need the room.
                  label: narrow || tight ? '' : 'Attach files',
                  icon: LucideIcons.paperclip,
                  variant: ColectoButtonVariant.ghost,
                  tooltip: 'Attach files',
                  onPressed: busy ? null : _attach,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    _stateLabel,
                    overflow: TextOverflow.ellipsis,
                    style: kStyle(12.5, color: pending ? p.ink2 : p.ink3),
                  ),
                ),
                const SizedBox(width: 8),
                if (!narrow) ...[
                  ColectoButton(
                    label: _state == _SaveState.saving
                        ? 'Saving…'
                        : 'Save draft',
                    tooltip: 'Save draft (${mod ? '⌘' : 'Ctrl'}+S)',
                    onPressed:
                        busy ||
                            _state == _SaveState.saving ||
                            (_draftId != null && !_dirty)
                        ? null
                        : _save,
                  ),
                  const SizedBox(width: 8),
                ],
                ColectoButton(
                  label: _state == _SaveState.sent
                      ? 'Sent'
                      : _state == _SaveState.sending
                      ? 'Sending…'
                      : 'Send',
                  trailingIcon: LucideIcons.send,
                  variant: ColectoButtonVariant.primary,
                  tooltip: 'Send (${mod ? '⌘' : 'Ctrl'}+Enter)',
                  onPressed: busy ? null : _send,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// An input row whose hairline takes a hint of the accent while focused.
class _FocusHairline extends StatefulWidget {
  const _FocusHairline({required this.child});

  final Widget child;

  @override
  State<_FocusHairline> createState() => _FocusHairlineState();
}

class _FocusHairlineState extends State<_FocusHairline> {
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Focus(
      canRequestFocus: false,
      skipTraversal: true,
      onFocusChange: (v) => setState(() => _focused = v),
      child: AnimatedContainer(
        duration: ColectoMotion.of(context, ColectoMotion.hover),
        constraints: const BoxConstraints(minHeight: 44),
        decoration: BoxDecoration(
          border: Border(
            bottom: BorderSide(
              color: _focused
                  ? Color.lerp(p.lineSoft, p.accent, 0.45)!
                  : p.lineSoft,
            ),
          ),
        ),
        child: widget.child,
      ),
    );
  }
}
