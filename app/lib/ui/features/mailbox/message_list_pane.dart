import 'dart:async';

import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/account_models.dart';
import '../../../domain/models/mail_models.dart';
import '../../../domain/models/preferences_models.dart';
import '../../core/format/when.dart';
import '../../core/theme/colecto_palette.dart';
import '../../core/kit/kit.dart';
import '../shell/mail_controller.dart';

/// Colecto's mail pane: search with Compose beside it, label filter chips,
/// the mailbox list and its status footer. The list adapts to its own
/// width, not the window's: in a wide pane rows become single lines.
class MessageListPane extends StatefulWidget {
  const MessageListPane({
    super.key,
    required this.controller,
    required this.searchFocusNode,
    required this.onOpenDraft,
    required this.onThreadSelected,
    this.onCompose,
    this.headerSlivers = const [],
    this.phone = false,
  });

  final MailController controller;
  final FocusNode searchFocusNode;
  final ValueChanged<Draft> onOpenDraft;
  final ValueChanged<ConversationThread> onThreadSelected;

  /// Shown beside search; null on phones, where Compose lives in the chrome.
  final VoidCallback? onCompose;

  /// Native navigation bars (phones) scroll with the list.
  final List<Widget> headerSlivers;
  final bool phone;

  @override
  State<MessageListPane> createState() => _MessageListPaneState();
}

class _MessageListPaneState extends State<MessageListPane> {
  final _searchCtrl = TextEditingController();
  final _scroll = ScrollController();
  Timer? _debounce;
  String? _anchorId;

  MailController get c => widget.controller;

  @override
  void initState() {
    super.initState();
    _searchCtrl.text = c.searchQuery;
    c.addListener(_syncSearchQuery);
    _scroll.addListener(_maybeLoadMore);
  }

  void _maybeLoadMore() {
    if (!c.preferences.autoLoadMore || !c.canLoadMore || !_scroll.hasClients) {
      return;
    }
    if (_scroll.position.extentAfter <= 400) {
      unawaited(c.loadMoreMessages());
    }
  }

  void _syncSearchQuery() {
    if (!widget.searchFocusNode.hasFocus &&
        _searchCtrl.text.trim() != c.searchQuery) {
      _searchCtrl.text = c.searchQuery;
    }
  }

  @override
  void dispose() {
    c.removeListener(_syncSearchQuery);
    _scroll.removeListener(_maybeLoadMore);
    _debounce?.cancel();
    _searchCtrl.dispose();
    _scroll.dispose();
    super.dispose();
  }

  void _onQuery(String v) {
    setState(() {});
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 350), () {
      if (v.trim() != c.searchQuery) c.setSearchQuery(v.trim());
    });
  }

  void _clearSearch() {
    _debounce?.cancel();
    _searchCtrl.clear();
    setState(() {});
    if (c.searchQuery.isNotEmpty) c.setSearchQuery('');
  }

  void _openThread(ConversationThread t) {
    final keys = HardwareKeyboard.instance;
    final additive = keys.isControlPressed || keys.isMetaPressed;
    if (additive) {
      c.toggleBulkSelect(t.latest.id);
      _anchorId = t.latest.id;
      return;
    }
    if (keys.isShiftPressed && _anchorId != null) {
      final ids = c.threads.map((x) => x.latest.id).toList();
      final a = ids.indexOf(_anchorId!);
      final b = ids.indexOf(t.latest.id);
      if (a >= 0 && b >= 0) {
        for (final id in ids.sublist(a < b ? a : b, (a < b ? b : a) + 1)) {
          if (!c.bulkSelectedIds.contains(id)) c.toggleBulkSelect(id);
        }
        return;
      }
    }
    if (c.bulkSelectedIds.isNotEmpty) {
      c.toggleBulkSelect(t.latest.id);
      return;
    }
    _anchorId = t.latest.id;
    widget.onThreadSelected(t);
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final isDrafts = c.activeFolder == 'drafts' && c.activeLabel == null;
    final touch = colectoTouch(context);

    Widget list = CustomScrollView(
      controller: _scroll,
      slivers: [
        ...widget.headerSlivers,
        SliverToBoxAdapter(child: _head(p, isDrafts)),
        if (!isDrafts) SliverToBoxAdapter(child: _labelBar(p)),
        if (c.bulkSelectedIds.isNotEmpty)
          SliverToBoxAdapter(child: _bulkBar(p)),
        SliverToBoxAdapter(child: Container(height: 1, color: p.lineSoft)),
        ...(isDrafts ? _draftSlivers(p) : _threadSlivers(p)),
        const SliverToBoxAdapter(child: SizedBox(height: 12)),
      ],
    );

    if (touch) {
      list = RefreshIndicator.adaptive(
        onRefresh: () => c.loadMailbox(refresh: true),
        color: p.accent,
        backgroundColor: p.float,
        edgeOffset: widget.headerSlivers.isEmpty ? 0 : 100,
        child: list,
      );
    } else {
      list = Scrollbar(controller: _scroll, child: list);
    }

    return ColoredBox(
      color: p.bg,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(child: list),
          if (!widget.phone) _footer(p, isDrafts),
        ],
      ),
    );
  }

  // ── Head: search and compose ──────────────────────────────────────────

  Widget _head(ColectoPalette p, bool isDrafts) {
    final touch = colectoTouch(context);
    final h = touch ? 44.0 : 34.0;
    return Padding(
      padding: const EdgeInsets.fromLTRB(10, 10, 10, 8),
      child: Row(
        children: [
          Expanded(
            child: isDrafts
                ? Padding(
                    padding: const EdgeInsets.only(left: 4),
                    child: Text(
                      'Drafts are saved on your mail server.',
                      style: kStyle(12.5, color: p.ink3),
                    ),
                  )
                : _SearchField(
                    controller: _searchCtrl,
                    focusNode: widget.searchFocusNode,
                    height: h,
                    hint: 'Search ${c.currentFolderName.toLowerCase()}',
                    onChanged: _onQuery,
                    onSubmitted: (v) {
                      _debounce?.cancel();
                      c.setSearchQuery(v.trim());
                    },
                    onClear: _clearSearch,
                  ),
          ),
          if (widget.onCompose != null) ...[
            const SizedBox(width: 6),
            ColectoPressable(
              onTap: widget.onCompose,
              tooltip: 'Compose (C)',
              semanticLabel: 'Compose',
              pressScale: 0.95,
              builder: (context, s) => AnimatedContainer(
                duration: ColectoMotion.hover,
                width: h,
                height: h,
                decoration: BoxDecoration(
                  color: s.hovered ? p.accentHi : p.accent,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Icon(
                  LucideIcons.squarePen,
                  size: 16,
                  color: p.accentInk,
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }

  // ── Label filter chips ────────────────────────────────────────────────

  Widget _labelBar(ColectoPalette p) {
    final names = c.labels.keys.toList()..sort();
    // One row that scrolls sideways, so twenty labels never push the
    // list off screen.
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      padding: const EdgeInsets.fromLTRB(10, 0, 10, 10),
      child: Row(
        spacing: 6,
        children: [
          if (names.isNotEmpty)
            ColectoChip(
              label: 'All',
              selected: c.activeLabel == null,
              onTap: () => c.selectFolder(c.activeFolder),
            ),
          for (final name in names)
            ColectoChip(
              label: name,
              selected: c.activeLabel == name,
              onTap: () => c.activeLabel == name
                  ? c.selectFolder(c.activeFolder)
                  : c.selectLabel(name),
              onSecondaryTapDown: (pos) => showColectoContextMenu(
                context: context,
                globalPosition: pos,
                title: name,
                items: [
                  ColectoMenuItem(
                    label: 'Delete label',
                    icon: LucideIcons.trash2,
                    danger: true,
                    onSelected: () => _deleteLabel(name),
                  ),
                ],
              ),
            ),
          _NewLabelField(onCreate: (name) => c.updateLabel(name: name)),
        ],
      ),
    );
  }

  Future<void> _deleteLabel(String name) async {
    final ok = await showColectoConfirm(
      context: context,
      title: 'Delete “$name”?',
      message: 'Messages keep everything else; only the label goes.',
      confirmLabel: 'Delete label',
      danger: true,
    );
    if (!ok) return;
    if (c.activeLabel == name) await c.selectFolder(c.activeFolder);
    await c.updateLabel(name: name, remove: true);
  }

  // ── Bulk selection bar ────────────────────────────────────────────────

  Widget _bulkBar(ColectoPalette p) {
    final ids = c.bulkSelectedIds.toList();
    final all = ids.length == c.threads.length;
    // Act on every message of each selected conversation.
    List<String> expand() => [
      for (final t in c.threads)
        if (ids.contains(t.latest.id)) ...t.messages.map((m) => m.id),
    ];
    return Container(
      margin: const EdgeInsets.fromLTRB(10, 0, 10, 10),
      padding: const EdgeInsets.only(left: 12, right: 4),
      height: 40,
      decoration: BoxDecoration(
        color: p.select,
        borderRadius: BorderRadius.circular(9),
      ),
      child: Row(
        children: [
          ColectoCheckbox(
            value: all,
            semanticLabel: 'Select all',
            onChanged: (v) => v ? c.selectAllVisible() : c.clearBulkSelection(),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              '${ids.length} selected',
              style: kStyle(13, weight: 600, color: p.accentHi),
            ),
          ),
          ColectoIconButton(
            icon: LucideIcons.archive,
            tooltip: 'Archive',
            onPressed: () => c.organizeMessages('archive', ids: expand()),
          ),
          ColectoIconButton(
            icon: LucideIcons.mailOpen,
            tooltip: 'Mark as read',
            onPressed: () async {
              await c.organizeMessages('flags', ids: expand(), seen: true);
              c.clearBulkSelection();
            },
          ),
          ColectoIconButton(
            icon: LucideIcons.sparkles,
            tooltip: 'Relabel with AI',
            onPressed: () => c.reclassifyMessages(ids),
          ),
          ColectoIconButton(
            icon: LucideIcons.trash2,
            tooltip: 'Move to Trash',
            danger: true,
            onPressed: () => c.organizeMessages('delete', ids: expand()),
          ),
          ColectoIconButton(
            icon: LucideIcons.x,
            tooltip: 'Clear selection (Esc)',
            onPressed: c.clearBulkSelection,
          ),
        ],
      ),
    );
  }

  // ── Threads ───────────────────────────────────────────────────────────

  List<Widget> _threadSlivers(ColectoPalette p) {
    if (c.threads.isEmpty) {
      if (c.refreshing || c.loading) {
        return [const SliverToBoxAdapter(child: _ListSkeleton())];
      }
      final searching = c.searchQuery.isNotEmpty;
      return [
        SliverToBoxAdapter(
          child: _ListState(
            title: searching
                ? 'No results for “${c.searchQuery}”'
                : c.activeLabel != null
                ? 'Nothing labelled ${c.activeLabel}'
                : 'Nothing in ${c.currentFolderName}',
            body: searching
                ? 'Try fewer words, or search another folder.'
                : c.activeFolder == 'inbox' && c.activeLabel == null
                ? 'You’re all caught up.'
                : 'Messages you move here will show up in this list.',
            action: searching
                ? ColectoButton(
                    label: 'Clear search',
                    compact: true,
                    onPressed: _clearSearch,
                  )
                : null,
          ),
        ),
      ];
    }

    final density = c.preferences.listDensity;
    final groups = <(String?, List<ConversationThread>)>[];
    if (c.preferences.dateGroups) {
      for (final t in c.threads) {
        final g = when(t.latestDate).group;
        if (groups.isEmpty || groups.last.$1 != g) groups.add((g, []));
        groups.last.$2.add(t);
      }
    } else {
      groups.add((null, c.threads));
    }

    return [
      for (final (label, threads) in groups)
        SliverMainAxisGroup(
          slivers: [
            if (label != null)
              SliverPersistentHeader(
                pinned: true,
                delegate: _DayHeader(label, p.bg, p.ink3),
              ),
            SliverPadding(
              padding: const EdgeInsets.symmetric(horizontal: 6),
              sliver: SliverList.builder(
                itemCount: threads.length,
                itemBuilder: (context, i) => _threadRow(p, threads[i], density),
              ),
            ),
          ],
        ),
      if (c.canPaginate) SliverToBoxAdapter(child: _paginationFooter(p)),
    ];
  }

  Widget _paginationFooter(ColectoPalette p) {
    Widget content;
    if (c.loadingMore) {
      content = Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            width: 12,
            height: 12,
            child: CircularProgressIndicator.adaptive(
              strokeWidth: 1.8,
              valueColor: AlwaysStoppedAnimation(p.ink3),
            ),
          ),
          const SizedBox(width: 8),
          Text('Loading older messages…', style: kStyle(12.5, color: p.ink3)),
        ],
      );
    } else if (c.loadMoreError != null) {
      content = Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            c.loadMoreError!,
            textAlign: TextAlign.center,
            style: kStyle(12.5, color: p.danger),
          ),
          const SizedBox(height: 4),
          _inlineLink(p, 'Try again', () => unawaited(c.loadMoreMessages())),
        ],
      );
    } else if (c.hasMore) {
      if (!c.preferences.autoLoadMore) {
        content = _inlineLink(
          p,
          'Load more',
          () => unawaited(c.loadMoreMessages()),
        );
      } else {
        return const SizedBox.shrink();
      }
    } else {
      content = Text('No older messages', style: kStyle(12, color: p.ink3));
    }

    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
      child: Center(child: content),
    );
  }

  Widget _inlineLink(ColectoPalette p, String label, VoidCallback onTap) {
    return ColectoPressable(
      onTap: onTap,
      semanticLabel: label,
      focusRadius: 4,
      builder: (context, s) => Padding(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
        child: Text(
          label,
          style:
              kStyle(
                12.5,
                weight: 550,
                color: s.hovered ? p.accentHi : p.accent,
              ).copyWith(
                decoration: TextDecoration.underline,
                decorationColor: s.hovered ? p.accentHi : p.accent,
              ),
        ),
      ),
    );
  }

  Widget _threadRow(
    ColectoPalette p,
    ConversationThread t,
    ListDensity density,
  ) {
    final ids = t.messages.map((m) => m.id).toList(growable: false);
    final selected = t.messages.any((m) => m.id == c.selectedMessageId);
    final row = _MailRow(
      key: ValueKey(t.id),
      sender: t.participants.join(', '),
      hueKey: t.latest.address,
      subject: t.subject,
      preview: t.latest.preview,
      date: when(t.latestDate).label,
      unread: t.hasUnread,
      flagged: t.flagged,
      count: t.messages.length,
      density: density,
      selected: selected,
      checked: c.bulkSelectedIds.contains(t.latest.id),
      bulkMode: c.bulkSelectedIds.isNotEmpty,
      onTap: () => _openThread(t),
      onToggleCheck: () => c.toggleBulkSelect(t.latest.id),
      onMenu: (pos) => _threadMenu(pos, t),
      hoverActions: [
        _HoverAction(
          LucideIcons.archive,
          'Archive (E)',
          () => c.organizeMessages('archive', ids: ids),
        ),
        _HoverAction(
          t.hasUnread ? LucideIcons.mailOpen : LucideIcons.mail,
          t.hasUnread ? 'Mark as read' : 'Mark as unread (U)',
          () => c.organizeMessages('flags', ids: ids, seen: t.hasUnread),
        ),
        _HoverAction(
          LucideIcons.trash2,
          'Move to Trash',
          () => c.organizeMessages('delete', ids: ids),
          danger: true,
        ),
      ],
    );
    if (!colectoTouch(context)) return row;
    return _Swipe(
      key: ValueKey('swipe-${t.id}'),
      onArchive: () => c.organizeMessages('archive', ids: ids),
      onDelete: () => c.organizeMessages('delete', ids: ids),
      child: row,
    );
  }

  void _threadMenu(Offset pos, ConversationThread t) {
    final ids = t.messages.map((m) => m.id).toList(growable: false);
    showColectoContextMenu(
      context: context,
      globalPosition: pos,
      title: t.subject,
      items: [
        ColectoMenuItem(
          label: 'Open',
          icon: LucideIcons.mailOpen,
          onSelected: () => widget.onThreadSelected(t),
        ),
        ColectoMenuItem(
          label: c.bulkSelectedIds.contains(t.latest.id)
              ? 'Deselect'
              : 'Select',
          icon: LucideIcons.squareCheck,
          shortcut: colectoTouch(context)
              ? null
              : '${colectoModKey(context)}+Click',
          onSelected: () => c.toggleBulkSelect(t.latest.id),
        ),
        const ColectoMenuItem.divider(),
        ColectoMenuItem(
          label: t.hasUnread ? 'Mark as read' : 'Mark as unread',
          icon: t.hasUnread ? LucideIcons.mailOpen : LucideIcons.mail,
          onSelected: () =>
              c.organizeMessages('flags', ids: ids, seen: t.hasUnread),
        ),
        ColectoMenuItem(
          label: t.flagged ? 'Unstar' : 'Star',
          icon: LucideIcons.star,
          shortcut: colectoTouch(context) ? null : 'S',
          onSelected: () => c.organizeMessages(
            'flags',
            ids: [t.latest.id],
            flagged: !t.flagged,
          ),
        ),
        ColectoMenuItem(
          label: 'Archive',
          icon: LucideIcons.archive,
          shortcut: colectoTouch(context) ? null : 'E',
          onSelected: () => c.organizeMessages('archive', ids: ids),
        ),
        const ColectoMenuItem.divider(),
        ColectoMenuItem(
          label: 'Move to Trash',
          icon: LucideIcons.trash2,
          danger: true,
          shortcut: colectoTouch(context) ? null : 'Del',
          onSelected: () => c.organizeMessages('delete', ids: ids),
        ),
      ],
    );
  }

  // ── Drafts ────────────────────────────────────────────────────────────

  List<Widget> _draftSlivers(ColectoPalette p) {
    if (c.drafts.isEmpty) {
      return [
        SliverToBoxAdapter(
          child: _ListState(
            title: 'No drafts',
            body: 'Messages you save without sending will wait here.',
            action: widget.onCompose == null
                ? null
                : ColectoButton(
                    label: 'New message',
                    icon: LucideIcons.squarePen,
                    compact: true,
                    onPressed: widget.onCompose,
                  ),
          ),
        ),
      ];
    }
    return [
      SliverPadding(
        padding: const EdgeInsets.fromLTRB(6, 6, 6, 0),
        sliver: SliverList.builder(
          itemCount: c.drafts.length,
          itemBuilder: (context, i) {
            final d = c.drafts[i];
            return _MailRow(
              key: ValueKey(d.id),
              sender: d.recipient.isEmpty
                  ? 'No recipient'
                  : 'To ${d.recipient}',
              hueKey: d.recipient,
              subject: d.subject.isEmpty ? '(No subject)' : d.subject,
              preview: d.body.replaceAll(RegExp(r'\s+'), ' ').trim(),
              date: 'Draft',
              unread: false,
              flagged: false,
              count: 1,
              density: c.preferences.listDensity,
              selected: false,
              checked: false,
              bulkMode: false,
              onTap: () => widget.onOpenDraft(d),
              onToggleCheck: null,
              onMenu: null,
              hoverActions: const [],
            );
          },
        ),
      ),
    ];
  }

  // ── Footer ────────────────────────────────────────────────────────────

  Widget _footer(ColectoPalette p, bool isDrafts) {
    final count = isDrafts ? c.drafts.length : c.threads.length;
    final noun = isDrafts ? 'draft' : 'conversation';
    final sync = c.currentAccountSync;
    final syncing = c.refreshing || sync?.activity == SyncActivity.syncing;
    final status = sync?.error != null
        ? 'Sync paused'
        : syncing
        ? 'Syncing…'
        : (c.syncedAt != null ? syncedAgo(c.syncedAt!) : '');
    return Container(
      height: 40,
      padding: const EdgeInsets.only(left: 16, right: 6),
      decoration: BoxDecoration(
        border: Border(top: BorderSide(color: p.lineSoft)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(
              count == 1 ? '1 $noun' : '$count ${noun}s',
              style: kStyle(12, color: p.ink3, features: tabularFigures),
            ),
          ),
          if (syncing) ...[const ColectoPulse(), const SizedBox(width: 8)],
          Tooltip(
            message: sync?.error ?? sync?.mode.label ?? '',
            child: Text(
              status,
              style: kStyle(12, color: sync?.error != null ? p.danger : p.ink3),
            ),
          ),
          const SizedBox(width: 2),
          ColectoIconButton(
            icon: LucideIcons.refreshCw,
            tooltip: 'Check for new mail',
            size: 14,
            dimension: 28,
            onPressed: c.refreshing ? null : () => c.loadMailbox(refresh: true),
          ),
        ],
      ),
    );
  }
}

class _SearchField extends StatefulWidget {
  const _SearchField({
    required this.controller,
    required this.focusNode,
    required this.height,
    required this.hint,
    required this.onChanged,
    required this.onSubmitted,
    required this.onClear,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final double height;
  final String hint;
  final ValueChanged<String> onChanged;
  final ValueChanged<String> onSubmitted;
  final VoidCallback onClear;

  @override
  State<_SearchField> createState() => _SearchFieldState();
}

class _SearchFieldState extends State<_SearchField> {
  @override
  void initState() {
    super.initState();
    widget.focusNode.addListener(_focus);
  }

  @override
  void dispose() {
    widget.focusNode.removeListener(_focus);
    super.dispose();
  }

  void _focus() => setState(() {});

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final focused = widget.focusNode.hasFocus;
    final size = colectoTouch(context) ? 16.0 : 13.5;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): () {
          if (widget.controller.text.isNotEmpty) {
            widget.onClear();
          } else {
            widget.focusNode.unfocus();
          }
        },
      },
      child: AnimatedContainer(
        duration: ColectoMotion.hover,
        height: widget.height,
        decoration: BoxDecoration(
          color: p.sunk,
          borderRadius: BorderRadius.circular(9),
          border: Border.all(
            color: focused ? Color.lerp(p.line, p.accent, 0.55)! : p.lineSoft,
          ),
        ),
        child: Row(
          children: [
            const SizedBox(width: 11),
            Icon(LucideIcons.search, size: 15, color: p.ink3),
            Expanded(
              child: TextField(
                controller: widget.controller,
                focusNode: widget.focusNode,
                onChanged: widget.onChanged,
                onSubmitted: widget.onSubmitted,
                textInputAction: TextInputAction.search,
                autocorrect: false,
                cursorColor: p.accent,
                style: kStyle(size, color: p.ink),
                decoration: InputDecoration(
                  isCollapsed: true,
                  border: InputBorder.none,
                  hintText: widget.hint,
                  hintStyle: kStyle(size, color: p.ink3),
                  contentPadding: const EdgeInsets.fromLTRB(9, 8, 6, 8),
                ),
              ),
            ),
            if (widget.controller.text.isNotEmpty)
              ColectoIconButton(
                icon: LucideIcons.x,
                tooltip: 'Clear search',
                size: 13,
                dimension: widget.height - 8,
                onPressed: widget.onClear,
              )
            else if (!focused && !colectoTouch(context))
              const Padding(
                padding: EdgeInsets.only(right: 7),
                child: ColectoKbd('/'),
              ),
            const SizedBox(width: 3),
          ],
        ),
      ),
    );
  }
}

/// Colecto's `.label-new`: a dashed pill that widens as you type.
class _NewLabelField extends StatefulWidget {
  const _NewLabelField({required this.onCreate});

  final ValueChanged<String> onCreate;

  @override
  State<_NewLabelField> createState() => _NewLabelFieldState();
}

class _NewLabelFieldState extends State<_NewLabelField> {
  final _ctrl = TextEditingController();
  final _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _ctrl.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _submit() {
    final name = _ctrl.text.trim();
    if (name.isEmpty) return;
    widget.onCreate(name);
    _ctrl.clear();
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final touch = colectoTouch(context);
    final active = _focus.hasFocus;
    final ready = _ctrl.text.trim().isNotEmpty;
    return CustomPaint(
      painter: active ? null : _DashedPill(p.line),
      child: AnimatedContainer(
        duration: ColectoMotion.of(context, const Duration(milliseconds: 180)),
        curve: ColectoMotion.easeOut,
        width: active || ready ? 168 : (touch ? 132 : 112),
        height: touch ? 32 : 26,
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(999),
          border: active
              ? Border.all(color: Color.lerp(p.line, p.accent, 0.55)!)
              : null,
        ),
        child: Row(
          children: [
            Expanded(
              child: TextField(
                controller: _ctrl,
                focusNode: _focus,
                onChanged: (_) => setState(() {}),
                onSubmitted: (_) => _submit(),
                cursorColor: p.accent,
                maxLength: 80,
                style: kStyle(touch ? 16 : 12.5, color: p.ink),
                decoration: InputDecoration(
                  isCollapsed: true,
                  border: InputBorder.none,
                  counterText: '',
                  hintText: 'New label',
                  hintStyle: kStyle(touch ? 16 : 12.5, color: p.ink3),
                  contentPadding: const EdgeInsets.only(
                    left: 11,
                    top: 5,
                    bottom: 5,
                  ),
                ),
              ),
            ),
            ColectoPressable(
              onTap: ready ? _submit : null,
              tooltip: 'Create label',
              focusRadius: 999,
              builder: (context, s) => SizedBox(
                width: 24,
                height: 24,
                child: Icon(
                  LucideIcons.plus,
                  size: 14,
                  color: ready ? p.accent : p.ink3,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _DashedPill extends CustomPainter {
  _DashedPill(this.color);

  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final path = Path()
      ..addRRect(
        RRect.fromRectAndRadius(
          (Offset.zero & size).deflate(0.5),
          Radius.circular(size.height / 2),
        ),
      );
    final paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1
      ..color = color;
    for (final metric in path.computeMetrics()) {
      for (var d = 0.0; d < metric.length; d += 6) {
        canvas.drawPath(metric.extractPath(d, d + 3), paint);
      }
    }
  }

  @override
  bool shouldRepaint(_DashedPill old) => old.color != color;
}

class _DayHeader extends SliverPersistentHeaderDelegate {
  _DayHeader(this.label, this.bg, this.ink);

  final String label;
  final Color bg;
  final Color ink;

  @override
  double get minExtent => 36;
  @override
  double get maxExtent => 36;

  @override
  Widget build(BuildContext context, double shrinkOffset, bool overlaps) {
    return Container(
      color: bg,
      alignment: Alignment.bottomLeft,
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 6),
      child: Text(label, style: kStyle(12, weight: 600, color: ink)),
    );
  }

  @override
  bool shouldRebuild(_DayHeader old) =>
      old.label != label || old.bg != bg || old.ink != ink;
}

class _HoverAction {
  const _HoverAction(
    this.icon,
    this.tooltip,
    this.onTap, {
    this.danger = false,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback onTap;
  final bool danger;
}

/// Colecto's `.mail-item`: sender and date, subject, preview; a soft mint
/// node marks the one you are reading, and unread mail shows with weight,
/// not another dot.
class _MailRow extends StatefulWidget {
  const _MailRow({
    super.key,
    required this.sender,
    required this.hueKey,
    required this.subject,
    required this.preview,
    required this.date,
    required this.unread,
    required this.flagged,
    required this.count,
    required this.density,
    required this.selected,
    required this.checked,
    required this.bulkMode,
    required this.onTap,
    required this.onToggleCheck,
    required this.onMenu,
    required this.hoverActions,
  });

  final String sender;
  final String hueKey;
  final String subject;
  final String preview;
  final String date;
  final bool unread;
  final bool flagged;
  final int count;
  final ListDensity density;
  final bool selected;
  final bool checked;
  final bool bulkMode;
  final VoidCallback onTap;
  final VoidCallback? onToggleCheck;
  final ValueChanged<Offset>? onMenu;
  final List<_HoverAction> hoverActions;

  @override
  State<_MailRow> createState() => _MailRowState();
}

class _MailRowState extends State<_MailRow> {
  bool _hovered = false;

  /// macOS draws the selected row in the accent with inverted text.
  bool _invert = false;

  Color _c(ColectoPalette p, Color normal, {bool muted = false}) => _invert
      ? (muted ? p.accentInk.withValues(alpha: 0.72) : p.accentInk)
      : normal;

  @override
  void didUpdateWidget(_MailRow old) {
    super.didUpdateWidget(old);
    if (widget.selected && !old.selected) {
      // Keyboard (j/k) selection keeps the row in view.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        Scrollable.ensureVisible(
          context,
          alignmentPolicy: ScrollPositionAlignmentPolicy.keepVisibleAtEnd,
        );
        Scrollable.ensureVisible(
          context,
          alignmentPolicy: ScrollPositionAlignmentPolicy.keepVisibleAtStart,
        );
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final touch = colectoTouch(context);
    return LayoutBuilder(
      builder: (context, constraints) {
        final wide =
            constraints.maxWidth >= 640 ||
            widget.density == ListDensity.compact;
        return ColectoPressable(
          onTap: widget.onTap,
          onSecondaryTapDown: widget.onMenu,
          onLongPress: touch ? widget.onMenu : null,
          onHoverChanged: (v) => setState(() => _hovered = v),
          selected: widget.selected,
          focusRadius: 9,
          pressScale: touch ? 1 : 0.985,
          semanticLabel:
              '${widget.unread ? 'Unread. ' : ''}${widget.sender}. ${widget.subject}. ${widget.date}',
          builder: (context, s) {
            final style = colectoPlatformStyle(context);
            _invert = style == ColectoPlatformStyle.macos && widget.selected;
            final radius = switch (style) {
              ColectoPlatformStyle.fluent => 4.0,
              ColectoPlatformStyle.macos => 6.0,
              _ => 9.0,
            };
            final bg = _invert
                ? p.accent
                : style == ColectoPlatformStyle.fluent && widget.selected
                ? p.ink.withValues(alpha: p.isDark ? 0.07 : 0.05)
                : widget.selected || widget.checked
                ? p.select
                : (s.hovered ? p.raise : const Color(0x00000000));
            return AnimatedContainer(
              duration: const Duration(milliseconds: 120),
              margin: const EdgeInsets.only(bottom: 1),
              padding: wide
                  ? EdgeInsets.fromLTRB(22, touch ? 13 : 9, 12, touch ? 13 : 9)
                  : EdgeInsets.fromLTRB(
                      18,
                      touch ? 11 : 9,
                      10,
                      touch ? 12 : 10,
                    ),
              decoration: BoxDecoration(
                color: bg,
                borderRadius: BorderRadius.circular(radius),
              ),
              child: Stack(
                clipBehavior: Clip.none,
                children: [
                  if (widget.bulkMode)
                    Positioned(
                      left: wide ? -17 : -13,
                      top: wide ? 0 : 1,
                      bottom: wide ? 0 : null,
                      child: Center(
                        child: Transform.scale(
                          scale: 0.8,
                          child: ColectoCheckbox(
                            value: widget.checked,
                            onChanged: widget.onToggleCheck == null
                                ? null
                                : (_) => widget.onToggleCheck!(),
                          ),
                        ),
                      ),
                    )
                  else if (widget.selected &&
                      style == ColectoPlatformStyle.fluent)
                    // Fluent's selection pill.
                    Positioned(
                      left: wide ? -21 : -17,
                      top: 0,
                      bottom: 0,
                      child: Center(
                        child: Container(
                          width: 3,
                          height: 16,
                          decoration: BoxDecoration(
                            color: p.accent,
                            borderRadius: BorderRadius.circular(2),
                          ),
                        ),
                      ),
                    )
                  else if (style == ColectoPlatformStyle.macos)
                    // Mail.app's unread dot.
                    if (widget.unread)
                      Positioned(
                        left: wide ? -15 : -12,
                        top: wide ? 0 : 6,
                        bottom: wide ? 0 : null,
                        child: Center(
                          child: Container(
                            width: 7,
                            height: 7,
                            decoration: BoxDecoration(
                              color: _invert ? p.accentInk : p.accent,
                              shape: BoxShape.circle,
                            ),
                          ),
                        ),
                      )
                    else
                      const SizedBox.shrink()
                  else if (widget.selected)
                    Positioned(
                      left: wide ? -13 : -11,
                      top: wide ? 0 : 6,
                      bottom: wide ? 0 : null,
                      child: Center(child: _SelectionNode(p: p)),
                    ),
                  wide ? _single(p) : _stacked(p),
                ],
              ),
            );
          },
        );
      },
    );
  }

  TextStyle _senderStyle(ColectoPalette p, double size) => kStyle(
    size,
    weight: widget.unread ? 680 : 560,
    color: _c(p, widget.unread || widget.selected ? p.ink : p.ink2),
  );

  TextStyle _subjectStyle(ColectoPalette p) => kStyle(
    13,
    weight: widget.unread ? 600 : 450,
    color: _c(p, widget.unread || widget.selected ? p.ink : p.ink2),
  );

  Widget _meta(ColectoPalette p) {
    final showActions =
        _hovered && widget.hoverActions.isNotEmpty && !colectoTouch(context);
    if (showActions) {
      return Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final a in widget.hoverActions)
            ColectoIconButton(
              icon: a.icon,
              tooltip: a.tooltip,
              danger: a.danger,
              size: 14,
              dimension: 24,
              onPressed: a.onTap,
            ),
        ],
      );
    }
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (widget.flagged) ...[
          Icon(LucideIcons.star, size: 12, color: p.star),
          const SizedBox(width: 6),
        ],
        Text(
          widget.date,
          style: kStyle(
            12,
            color: _c(p, widget.selected ? p.ink2 : p.ink3, muted: true),
            features: tabularFigures,
          ),
        ),
      ],
    );
  }

  Widget _senderText(ColectoPalette p, double size) {
    return Text.rich(
      TextSpan(
        children: [
          TextSpan(text: widget.sender),
          if (widget.count > 1)
            TextSpan(
              text: '  ${widget.count}',
              style: kStyle(
                12,
                weight: 560,
                color: _c(p, p.ink3, muted: true),
                features: tabularFigures,
              ),
            ),
        ],
      ),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: _senderStyle(p, size),
    );
  }

  Widget _stacked(ColectoPalette p) {
    final style = colectoPlatformStyle(context);
    // Gmail and Outlook lead each row with the sender's picture.
    final avatar =
        style == ColectoPlatformStyle.android ||
        style == ColectoPlatformStyle.fluent;
    final column = _stackedText(p);
    if (!avatar) return column;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(top: 2, right: 12),
          child: ColectoAvatar(
            name: widget.sender,
            hueKey: widget.hueKey,
            size: style == ColectoPlatformStyle.android ? 40 : 32,
          ),
        ),
        Expanded(child: column),
      ],
    );
  }

  Widget _stackedText(ColectoPalette p) {
    final touch = colectoTouch(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          height: 20,
          child: Row(
            children: [
              Expanded(child: _senderText(p, touch ? 15 : 13.5)),
              const SizedBox(width: 10),
              _meta(p),
            ],
          ),
        ),
        const SizedBox(height: 1),
        Text(
          widget.subject.isEmpty ? '(No subject)' : widget.subject,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: _subjectStyle(p).copyWith(fontSize: touch ? 14.5 : 13),
        ),
        if (widget.density == ListDensity.full &&
            widget.preview.isNotEmpty) ...[
          const SizedBox(height: 1),
          Text(
            widget.preview,
            maxLines: touch ? 2 : 1,
            overflow: TextOverflow.ellipsis,
            style: kStyle(
              touch ? 14 : 12.5,
              color: _c(p, p.ink3, muted: true),
              height: 1.4,
            ),
          ),
        ],
      ],
    );
  }

  Widget _single(ColectoPalette p) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final senderW = (constraints.maxWidth * 0.22).clamp(120.0, 220.0);
        return SizedBox(
          height: 22,
          child: Row(
            children: [
              SizedBox(width: senderW, child: _senderText(p, 13.5)),
              const SizedBox(width: 14),
              Flexible(
                flex: 0,
                child: ConstrainedBox(
                  constraints: BoxConstraints(
                    maxWidth: constraints.maxWidth * 0.45,
                  ),
                  child: Text(
                    widget.subject.isEmpty ? '(No subject)' : widget.subject,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: _subjectStyle(p),
                  ),
                ),
              ),
              if (widget.density != ListDensity.compact &&
                  widget.preview.isNotEmpty) ...[
                const SizedBox(width: 14),
                Expanded(
                  child: Text(
                    widget.preview,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: kStyle(12.5, color: _c(p, p.ink3, muted: true)),
                  ),
                ),
              ] else
                const Spacer(),
              const SizedBox(width: 14),
              _meta(p),
            ],
          ),
        );
      },
    );
  }
}

/// The selection node: the incoming-message dot from the logo, softly lit.
class _SelectionNode extends StatelessWidget {
  const _SelectionNode({required this.p});

  final ColectoPalette p;

  @override
  Widget build(BuildContext context) {
    return TweenAnimationBuilder<double>(
      tween: Tween(begin: 0, end: 1),
      duration: ColectoMotion.of(context, const Duration(milliseconds: 220)),
      curve: ColectoMotion.easeOut,
      builder: (context, t, _) => Transform.scale(
        scale: 0.4 + 0.6 * t,
        child: Container(
          width: 5,
          height: 5,
          decoration: BoxDecoration(
            color: p.accentHi,
            shape: BoxShape.circle,
            boxShadow: [
              BoxShadow(
                color: p.accent.withValues(alpha: 0.7 * t),
                blurRadius: 8,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Swipe right to archive, left to delete (phones and tablets).
class _Swipe extends StatelessWidget {
  const _Swipe({
    super.key,
    required this.child,
    required this.onArchive,
    required this.onDelete,
  });

  final Widget child;
  final VoidCallback onArchive;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    Widget bg(Color color, Color fg, IconData icon, String label, bool left) =>
        Container(
          margin: const EdgeInsets.only(bottom: 1),
          decoration: BoxDecoration(
            color: color,
            borderRadius: BorderRadius.circular(9),
          ),
          padding: const EdgeInsets.symmetric(horizontal: 20),
          alignment: left ? Alignment.centerLeft : Alignment.centerRight,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 18, color: fg),
              const SizedBox(width: 8),
              Text(label, style: kStyle(13.5, weight: 600, color: fg)),
            ],
          ),
        );
    return Dismissible(
      key: key!,
      dismissThresholds: const {
        DismissDirection.startToEnd: 0.3,
        DismissDirection.endToStart: 0.3,
      },
      background: bg(
        p.select,
        p.accentHi,
        LucideIcons.archive,
        'Archive',
        true,
      ),
      secondaryBackground: bg(
        p.dangerBg,
        p.danger,
        LucideIcons.trash2,
        'Trash',
        false,
      ),
      confirmDismiss: (dir) async {
        HapticFeedback.mediumImpact();
        dir == DismissDirection.startToEnd ? onArchive() : onDelete();
        return false;
      },
      child: child,
    );
  }
}

class _ListState extends StatelessWidget {
  const _ListState({required this.title, required this.body, this.action});

  final String title;
  final String body;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 28, 16, 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: kStyle(15, weight: 640, color: p.ink)),
          const SizedBox(height: 4),
          Text(body, style: kStyle(13.5, color: p.ink3, height: 1.5)),
          if (action != null) ...[const SizedBox(height: 14), action!],
        ],
      ),
    );
  }
}

class _ListSkeleton extends StatelessWidget {
  const _ListSkeleton();

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.fromLTRB(24, 20, 24, 0),
      child: Column(
        children: [
          ColectoSkeleton(lines: 3),
          SizedBox(height: 14),
          ColectoSkeleton(lines: 3),
          SizedBox(height: 14),
          ColectoSkeleton(lines: 3),
        ],
      ),
    );
  }
}
