import 'dart:ui' show ImageFilter;

import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/mail_models.dart';
import '../../../domain/models/preferences_models.dart';
import '../../core/format/when.dart';
import '../../core/platform/save_file.dart';
import '../../core/theme/colecto_palette.dart';
import '../../core/widgets/html_message_view.dart';
import '../../core/widgets/colecto_mark.dart';
import '../../core/kit/kit.dart';
import '../composer/compose_requests.dart';
import '../shell/mail_controller.dart';

/// Colecto's reader: a sticky, translucent toolbar over a centred letter
/// with a large subject, the sender row, labels, attachments and a body set
/// for long reading. Earlier messages in a conversation fold into one line.
class ReaderPane extends StatefulWidget {
  const ReaderPane({
    super.key,
    required this.controller,
    required this.onCompose,
    this.showBackButton = false,
    this.onBack,
  });

  final MailController controller;
  final ValueChanged<ComposeRequest> onCompose;
  final bool showBackButton;
  final VoidCallback? onBack;

  @override
  State<ReaderPane> createState() => _ReaderPaneState();
}

class _ReaderPaneState extends State<ReaderPane> {
  final _scroll = ScrollController();
  final _aiNotes = TextEditingController();
  final Set<String> _expanded = {};
  String? _forId;
  bool _plain = false;
  bool _aiReplyOpen = false;
  String _aiTone = 'professional';
  bool _drafting = false;
  String? _downloading;
  bool _copied = false;

  MailController get c => widget.controller;

  @override
  void dispose() {
    _scroll.dispose();
    _aiNotes.dispose();
    super.dispose();
  }

  void _resetFor(String id) {
    _forId = id;
    _expanded.clear();
    _plain = false;
    _aiReplyOpen = false;
    _copied = false;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && _scroll.hasClients) _scroll.jumpTo(0);
    });
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final id = c.selectedMessageId;
    final conversation = c.conversation;

    if (id == null) {
      return _Welcome(controller: c);
    }
    if (id != _forId) _resetFor(id);
    if (conversation.isEmpty) {
      return ColoredBox(
        color: p.bg,
        child: Column(
          children: [
            _toolbar(p, null, null, const [], narrow: true),
            const Padding(
              padding: EdgeInsets.fromLTRB(40, 32, 40, 0),
              child: ColectoSkeleton(lines: 4),
            ),
          ],
        ),
      );
    }

    final latest = conversation.last.message;
    final latestContent = c.contentFor(latest.id);
    final allIds = [for (final t in conversation) t.message.id];
    for (final t in conversation) {
      if (!t.message.seen) _expanded.add(t.message.id);
    }
    _expanded.add(latest.id);

    return ColoredBox(
      color: p.bg,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final narrow = constraints.maxWidth < 560;
          final side = narrow ? 18.0 : 40.0;
          final frameHeight = (constraints.maxHeight - 112).clamp(
            320.0,
            double.infinity,
          );
          return Stack(
            children: [
              Positioned.fill(
                child: Scrollbar(
                  controller: _scroll,
                  child: SingleChildScrollView(
                    controller: _scroll,
                    padding: EdgeInsets.only(top: _toolbarHeight),
                    child: Center(
                      child: ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 780),
                        child: Padding(
                          padding: EdgeInsets.fromLTRB(
                            side,
                            narrow ? 12 : 18,
                            side,
                            72 + MediaQuery.paddingOf(context).bottom,
                          ),
                          child: SelectionArea(
                            child: _letter(
                              p,
                              conversation,
                              latest,
                              narrow,
                              frameHeight,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              Positioned(
                top: 0,
                left: 0,
                right: 0,
                child: _toolbar(
                  p,
                  latest,
                  latestContent,
                  allIds,
                  narrow: narrow,
                ),
              ),
            ],
          );
        },
      ),
    );
  }

  double get _toolbarHeight => widget.showBackButton ? 54 : 48;

  // ── Toolbar ───────────────────────────────────────────────────────────

  Widget _toolbar(
    ColectoPalette p,
    MailMessage? latest,
    MessageContent? content,
    List<String> allIds, {
    required bool narrow,
  }) {
    final style = colectoPlatformStyle(context);
    final isIos = style == ColectoPlatformStyle.ios;
    // macOS, Material and iOS toolbars are icon-only; Fluent's command bar
    // and Colecto's tools carry labels when there is room.
    final iconOnly =
        narrow ||
        style == ColectoPlatformStyle.macos ||
        style == ColectoPlatformStyle.android ||
        isIos;
    Widget tool(IconData icon, String label, VoidCallback? onTap) =>
        ColectoButton(
          label: iconOnly ? '' : label,
          icon: icon,
          tooltip: label,
          variant: ColectoButtonVariant.ghost,
          onPressed: onTap,
        );
    return ClipRect(
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 14, sigmaY: 14),
        child: Container(
          height: _toolbarHeight,
          padding: EdgeInsets.only(
            left: widget.showBackButton ? 6 : (narrow ? 16 : 24),
            right: narrow ? 6 : 12,
          ),
          color: p.bg.withValues(alpha: 0.86),
          child: Row(
            children: [
              if (widget.showBackButton)
                ColectoIconButton(
                  icon: isIos ? LucideIcons.chevronLeft : LucideIcons.arrowLeft,
                  tooltip: 'Back',
                  size: isIos ? 22 : 18,
                  onPressed: widget.onBack,
                ),
              Expanded(
                child: Text(
                  c.currentFolderName,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: kStyle(13, weight: 540, color: p.ink3),
                ),
              ),
              if (latest != null) ...[
                tool(
                  LucideIcons.reply,
                  'Reply',
                  () => widget.onCompose(_reply(latest, content, false)),
                ),
                if (!narrow)
                  tool(
                    LucideIcons.replyAll,
                    'Reply all',
                    () => widget.onCompose(_reply(latest, content, true)),
                  ),
                tool(
                  LucideIcons.forward,
                  'Forward',
                  () => widget.onCompose(forwardRequest(latest, content)),
                ),
                Container(
                  width: 1,
                  height: 18,
                  margin: const EdgeInsets.symmetric(horizontal: 6),
                  color: p.line,
                ),
                ColectoIconButton(
                  icon: LucideIcons.archive,
                  tooltip: 'Archive (E)',
                  onPressed: () => c.organizeMessages('archive', ids: allIds),
                ),
                ColectoIconButton(
                  icon: LucideIcons.trash2,
                  tooltip: 'Move to Trash (#)',
                  danger: true,
                  onPressed: () => c.organizeMessages('delete', ids: allIds),
                ),
                if (!narrow)
                  ColectoIconButton(
                    icon: LucideIcons.star,
                    tooltip: latest.flagged ? 'Unstar (S)' : 'Star (S)',
                    color: latest.flagged ? p.star : null,
                    onPressed: () => c.organizeMessages(
                      'flags',
                      ids: [latest.id],
                      flagged: !latest.flagged,
                    ),
                  ),
                Builder(
                  builder: (ctx) => ColectoIconButton(
                    icon: LucideIcons.ellipsis,
                    tooltip: 'More',
                    onPressed: () =>
                        _moreMenu(ctx, latest, content, allIds, narrow),
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  ComposeRequest _reply(MailMessage m, MessageContent? content, bool all) =>
      replyRequest(
        m,
        content,
        all: all,
        self: c.currentAccountInfo?.address ?? '',
      );

  void _moreMenu(
    BuildContext ctx,
    MailMessage latest,
    MessageContent? content,
    List<String> allIds,
    bool narrow,
  ) {
    final rect = colectoRectOf(ctx);
    final items = [
      if (narrow) ...[
        ColectoMenuItem(
          label: 'Reply all',
          icon: LucideIcons.replyAll,
          shortcut: 'A',
          onSelected: () => widget.onCompose(_reply(latest, content, true)),
        ),
        ColectoMenuItem(
          label: latest.flagged ? 'Unstar' : 'Star',
          icon: LucideIcons.star,
          shortcut: 'S',
          onSelected: () => c.organizeMessages(
            'flags',
            ids: [latest.id],
            flagged: !latest.flagged,
          ),
        ),
      ],
      ColectoMenuItem(
        label: 'Mark as unread',
        icon: LucideIcons.mail,
        shortcut: 'U',
        onSelected: () => c.organizeMessages('flags', ids: allIds, seen: false),
      ),
      ColectoMenuItem(
        label: _copied ? 'Copied' : 'Copy message text',
        icon: LucideIcons.copy,
        onSelected: () => _copy(latest, content),
      ),
      if (content?.html != null)
        ColectoMenuItem(
          label: _plain ? 'Show formatted' : 'Show plain text',
          icon: LucideIcons.type,
          onSelected: () => setState(() => _plain = !_plain),
        ),
      const ColectoMenuItem.divider(),
      ColectoMenuItem(
        label: 'Summarize with AI',
        icon: LucideIcons.sparkles,
        onSelected: () => c.generateSummary(latest.id),
      ),
      ColectoMenuItem(
        label: 'Draft a reply with AI',
        icon: LucideIcons.wandSparkles,
        onSelected: () => setState(() => _aiReplyOpen = true),
      ),
    ];
    if (colectoTouch(context)) {
      showColectoContextMenu(
        context: context,
        globalPosition: rect.center,
        items: items,
      );
    } else {
      showColectoMenu(
        context: ctx,
        anchor: rect,
        items: items,
        width: 230,
        alignRight: true,
      );
    }
  }

  Future<void> _copy(MailMessage m, MessageContent? content) async {
    await Clipboard.setData(
      ClipboardData(
        text: content?.text.isNotEmpty == true ? content!.text : m.body,
      ),
    );
    if (!mounted) return;
    setState(() => _copied = true);
    c.showToast('Message text copied');
  }

  // ── Letter ────────────────────────────────────────────────────────────

  Widget _letter(
    ColectoPalette p,
    List<ThreadMessage> conversation,
    MailMessage latest,
    bool narrow,
    double frameHeight,
  ) {
    final labels = c.labels.keys.toList()..sort();
    final assigned = c.labelsForMessage(latest.id).toSet();
    final summary = c.summaryFor(latest.id);
    final summarizing = c.isLoadingSummary(latest.id);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          normalizeSubject(latest.subject).isEmpty
              ? '(No subject)'
              : normalizeSubject(latest.subject),
          style: kStyle(
            narrow ? 22 : 26,
            weight: 660,
            width: 108,
            color: p.ink,
            height: 1.2,
            letterSpacing: narrow ? -0.4 : -0.55,
          ),
        ),
        SizedBox(height: labels.isEmpty ? 22 : 14),
        if (labels.isNotEmpty) ...[
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: [
              for (final name in labels)
                ColectoChip(
                  label: name,
                  showCheck: true,
                  selected: assigned.contains(name),
                  onTap: () => c.updateLabel(
                    name: name,
                    messageId: latest.id,
                    remove: assigned.contains(name),
                  ),
                ),
            ],
          ),
          const SizedBox(height: 20),
        ],
        if (summarizing || summary != null) ...[
          _summary(p, latest.id, summary),
          const SizedBox(height: 20),
        ],
        if (_aiReplyOpen) ...[_aiReply(p, latest), const SizedBox(height: 20)],
        for (var i = 0; i < conversation.length; i++) ...[
          if (i > 0)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Container(height: 1, color: p.lineSoft),
            ),
          _expanded.contains(conversation[i].message.id)
              ? _message(p, conversation[i].message, narrow, frameHeight)
              : _collapsed(p, conversation[i].message),
        ],
      ],
    );
  }

  Widget _summary(ColectoPalette p, String id, String? summary) {
    return ColectoNotice(
      icon: LucideIcons.sparkles,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  'Summary',
                  style: kStyle(13, weight: 640, color: p.accentHi),
                ),
              ),
              if (summary != null)
                ColectoIconButton(
                  icon: LucideIcons.x,
                  tooltip: 'Remove summary',
                  size: 13,
                  dimension: 22,
                  color: p.accentHi,
                  onPressed: () => c.deleteSummary(id),
                ),
            ],
          ),
          const SizedBox(height: 4),
          if (summary == null)
            Text('Summarizing…', style: kStyle(13.5, color: p.ink2))
          else
            Text(summary, style: kStyle(13.5, color: p.ink, height: 1.55)),
        ],
      ),
    );
  }

  Widget _aiReply(ColectoPalette p, MailMessage latest) {
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: p.sunk,
        borderRadius: BorderRadius.circular(11),
        border: Border.all(color: p.lineSoft),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(LucideIcons.wandSparkles, size: 15, color: p.accent),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  'Draft a reply with AI',
                  style: kStyle(14, weight: 620, color: p.ink),
                ),
              ),
              ColectoIconButton(
                icon: LucideIcons.x,
                tooltip: 'Close',
                size: 14,
                dimension: 26,
                onPressed: () => setState(() => _aiReplyOpen = false),
              ),
            ],
          ),
          const SizedBox(height: 12),
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: ColectoSegmented<String>(
              value: _aiTone,
              options: const [
                ('professional', 'Professional'),
                ('lazy', 'Brief'),
                ('roast', 'Roast'),
                ('troll', 'Playful'),
              ],
              onChanged: (v) => setState(() => _aiTone = v),
            ),
          ),
          const SizedBox(height: 10),
          ColectoTextField(
            controller: _aiNotes,
            hintText: 'What should the reply say? (optional)',
            maxLines: 4,
            minLines: 2,
          ),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerRight,
            child: ColectoButton(
              label: _drafting ? 'Drafting…' : 'Draft reply',
              icon: LucideIcons.sparkles,
              variant: ColectoButtonVariant.primary,
              onPressed: _drafting
                  ? null
                  : () async {
                      setState(() => _drafting = true);
                      final text = await c.generateAiReply(
                        messageId: latest.id,
                        notes: _aiNotes.text,
                        tone: _aiTone,
                      );
                      if (!mounted) return;
                      setState(() => _drafting = false);
                      if (text != null) {
                        setState(() => _aiReplyOpen = false);
                        _aiNotes.clear();
                        widget.onCompose(
                          replyRequest(
                            latest,
                            c.contentFor(latest.id),
                            all: false,
                            self: c.currentAccountInfo?.address ?? '',
                            body: text,
                          ),
                        );
                      }
                    },
            ),
          ),
        ],
      ),
    );
  }

  Widget _collapsed(ColectoPalette p, MailMessage m) {
    return ColectoPressable(
      onTap: () => setState(() => _expanded.add(m.id)),
      pressScale: 0.99,
      semanticLabel: 'Show message from ${m.displaySender}',
      builder: (context, s) => AnimatedContainer(
        duration: ColectoMotion.hover,
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 10),
        decoration: BoxDecoration(
          color: s.hovered ? p.raise : const Color(0x00000000),
          borderRadius: BorderRadius.circular(9),
        ),
        child: Row(
          children: [
            ColectoAvatar(name: m.displaySender, hueKey: m.address, size: 28),
            const SizedBox(width: 12),
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 180),
              child: Text(
                m.displaySender,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: kStyle(13.5, weight: 600, color: p.ink),
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Text(
                m.preview,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: kStyle(13, color: p.ink3),
              ),
            ),
            const SizedBox(width: 12),
            Text(
              when(m.date).label,
              style: kStyle(12, color: p.ink3, features: tabularFigures),
            ),
          ],
        ),
      ),
    );
  }

  Widget _message(
    ColectoPalette p,
    MailMessage m,
    bool narrow,
    double frameHeight,
  ) {
    final content = c.contentFor(m.id);
    final loading = c.isLoadingContent(m.id) && content == null;
    final text = content?.text.isNotEmpty == true ? content!.text : m.body;
    final html = content?.html;
    final showHtml = html != null && html.isNotEmpty && !_plain;
    final recipients = [
      if (content?.to.isNotEmpty == true) 'to ${content!.to}',
      if (content?.cc.isNotEmpty == true) 'cc ${content!.cc}',
    ].join(' · ');
    final time = Tooltip(
      message: when(m.date).full,
      child: Text(
        readerTime(m.date),
        style: kStyle(12.5, color: p.ink3, features: tabularFigures),
      ),
    );

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          GestureDetector(
            onSecondaryTapDown: (d) => showColectoContextMenu(
              context: context,
              globalPosition: d.globalPosition,
              items: [
                ColectoMenuItem(
                  label: 'Copy address',
                  icon: LucideIcons.copy,
                  onSelected: () =>
                      Clipboard.setData(ClipboardData(text: m.address)),
                ),
                ColectoMenuItem(
                  label: 'Write to ${m.displaySender}',
                  icon: LucideIcons.squarePen,
                  onSelected: () =>
                      widget.onCompose(ComposeRequest(to: m.address)),
                ),
              ],
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                ColectoAvatar(
                  name: m.displaySender,
                  hueKey: m.address,
                  size: 38,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        m.displaySender,
                        style: kStyle(14.5, weight: 620, color: p.ink),
                      ),
                      if (m.address.isNotEmpty && m.address != m.displaySender)
                        Text(
                          m.address,
                          style: kStyle(12.5, color: p.ink3, height: 1.45),
                        ),
                      if (recipients.isNotEmpty)
                        Text(
                          recipients,
                          style: kStyle(12.5, color: p.ink3, height: 1.45),
                        ),
                      if (narrow) ...[const SizedBox(height: 2), time],
                    ],
                  ),
                ),
                if (!narrow) ...[const SizedBox(width: 12), time],
              ],
            ),
          ),
          const SizedBox(height: 20),
          if (content != null && content.attachments.isNotEmpty) ...[
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final a in content.attachments) _attachment(p, m, a),
              ],
            ),
            const SizedBox(height: 20),
          ],
          if (showHtml) ...[
            _bodyControls(p, m, content!),
            const SizedBox(height: 16),
          ],
          if (loading)
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 640),
              child: const ColectoSkeleton(),
            )
          else if (showHtml)
            HtmlMessageView(
              html: html,
              plainTextFallback: text,
              height: frameHeight,
            )
          else
            Align(
              alignment: Alignment.centerLeft,
              child: ConstrainedBox(
                // A 68ch measure.
                constraints: const BoxConstraints(maxWidth: 640),
                child: Text(
                  text.trim(),
                  style: Theme.of(context).textTheme.bodyLarge!
                      .copyWith(color: p.bodyInk),
                ),
              ),
            ),
        ],
      ),
    );
  }

  Widget _bodyControls(
    ColectoPalette p,
    MailMessage m,
    MessageContent content,
  ) {
    final blocked = content.blockedRemoteImages;
    return Wrap(
      spacing: 12,
      runSpacing: 6,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        if (blocked > 0) ...[
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(LucideIcons.imageOff, size: 15, color: p.ink2),
              const SizedBox(width: 8),
              Text(
                blocked == 1
                    ? '1 remote image blocked.'
                    : '$blocked remote images blocked.',
                style: kStyle(13, color: p.ink2),
              ),
            ],
          ),
          ColectoButton(
            label: 'Load remote images',
            variant: ColectoButtonVariant.text,
            onPressed: () =>
                c.loadMessageContent(m.id, remoteImages: true, force: true),
          ),
        ],
        ColectoButton(
          label: 'Show plain text',
          icon: LucideIcons.type,
          variant: ColectoButtonVariant.text,
          onPressed: () => setState(() => _plain = true),
        ),
      ],
    );
  }

  Widget _attachment(ColectoPalette p, MailMessage m, AttachmentInfo a) {
    final key = '${m.id}:${a.index}';
    final busy = _downloading == key;
    return ColectoPressable(
      onTap: _downloading != null ? null : () => _download(m, a),
      tooltip: 'Download ${a.name}',
      builder: (context, s) => AnimatedContainer(
        duration: ColectoMotion.hover,
        constraints: const BoxConstraints(minHeight: 40, maxWidth: 320),
        padding: const EdgeInsets.symmetric(horizontal: 12),
        decoration: BoxDecoration(
          color: s.hovered ? p.control : p.raise,
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              s.hovered ? LucideIcons.download : LucideIcons.paperclip,
              size: 15,
              color: p.ink3,
            ),
            const SizedBox(width: 8),
            Flexible(
              child: Text(
                a.name,
                overflow: TextOverflow.ellipsis,
                style: kStyle(13, weight: 540, color: p.ink),
              ),
            ),
            const SizedBox(width: 8),
            Text(
              busy ? 'Downloading…' : a.formattedSize,
              style: kStyle(12, color: p.ink3),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _download(MailMessage m, AttachmentInfo a) async {
    setState(() => _downloading = '${m.id}:${a.index}');
    try {
      final file = await c.backend.downloadAttachment(
        c.activeAccount,
        m.id,
        a.index,
      );
      final saved = await saveBytes(
        name: file.name,
        contentType: file.contentType,
        bytes: file.bytes,
      );
      if (saved) c.showToast('Saved ${file.name}');
    } on UnsupportedError catch (e) {
      c.showToast('Can’t save here yet', detail: e.message, isError: true);
    } catch (e) {
      c.showToast('Download failed', detail: '$e', isError: true);
    } finally {
      if (mounted) setState(() => _downloading = null);
    }
  }
}

/// The empty reader: the mark, a time-of-day greeting and how to get around.
/// Before the first account it offers to add one.
class _Welcome extends StatelessWidget {
  const _Welcome({required this.controller});

  final MailController controller;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = controller;
    final onboarding = !c.preferences.onboarded;
    final mod = colectoModKey(context);
    return ColoredBox(
      color: p.bg,
      child: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const ColectoMark(size: 48),
              const SizedBox(height: 18),
              Text(
                onboarding ? 'Welcome to Colecto.' : greeting(),
                textAlign: TextAlign.center,
                style: kStyle(
                  22,
                  weight: 640,
                  width: 108,
                  color: p.ink,
                  letterSpacing: -0.44,
                ),
              ),
              const SizedBox(height: 8),
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 380),
                child: Text(
                  c.loading
                      ? 'Opening your mailbox…'
                      : onboarding
                      ? 'You’re looking at demo mail. Add your IMAP or JMAP '
                            'account when you’re ready; passwords stay in the '
                            'encrypted vault.'
                      : 'Pick a message and it will open here.',
                  textAlign: TextAlign.center,
                  style: kStyle(13.5, color: p.ink3, height: 1.55),
                ),
              ),
              if (onboarding) ...[
                const SizedBox(height: 20),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  alignment: WrapAlignment.center,
                  children: [
                    ColectoButton(
                      label: 'Add account',
                      icon: LucideIcons.plus,
                      variant: ColectoButtonVariant.primary,
                      onPressed: () {
                        c.updatePreferences(
                          const PreferencesPatch(onboarded: true),
                        );
                        c.openSettings(SettingsSection.accounts);
                      },
                    ),
                    ColectoButton(
                      label: 'Explore the demo',
                      onPressed: () => c.updatePreferences(
                        const PreferencesPatch(onboarded: true),
                      ),
                    ),
                  ],
                ),
              ],
              if (!colectoTouch(context)) ...[
                const SizedBox(height: 22),
                Wrap(
                  alignment: WrapAlignment.center,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  spacing: 4,
                  runSpacing: 6,
                  children: [
                    Text('Press', style: kStyle(12.5, color: p.ink3)),
                    ColectoKbd(mod),
                    const ColectoKbd('K'),
                    Text(
                      'to jump anywhere,',
                      style: kStyle(12.5, color: p.ink3),
                    ),
                    const ColectoKbd('J'),
                    const ColectoKbd('K'),
                    Text('to move,', style: kStyle(12.5, color: p.ink3)),
                    const ColectoKbd('C'),
                    Text('to write.', style: kStyle(12.5, color: p.ink3)),
                  ],
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
