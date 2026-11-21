import 'package:cupertino_ui/cupertino_ui.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/mail_models.dart';
import '../../core/theme/colecto_palette.dart';
import '../../core/widgets/colecto_mark.dart';
import '../../core/kit/kit.dart';
import '../../core/widgets/toast_overlay.dart';
import '../composer/compose_requests.dart';
import '../composer/composer_view.dart';
import '../mailbox/message_list_pane.dart';
import '../reader/reader_pane.dart';
import '../settings/settings_view.dart';
import 'command_palette.dart';
import 'mail_controller.dart';
import 'sidebar_pane.dart';
import 'top_bar.dart';

/// Adaptive 3-tier mail shell (`docs/ui-design.md`):
/// - Phone (`< 600px`): a native navigation stack — large-title list, a
///   bottom folder bar, and pushed reader, composer and settings pages.
/// - Tablet / compact desktop (`600–1050px`): icon rail, list and reader.
/// - Desktop / wide web (`> 1050px`): sidebar, list and reader under
///   Colecto's top bar, with Settings as a card over the workspace.
class MailShell extends StatefulWidget {
  const MailShell({super.key, required this.controller});

  final MailController controller;

  @override
  State<MailShell> createState() => _MailShellState();
}

class _MailShellState extends State<MailShell> {
  final _searchFocus = FocusNode();
  final _shellFocus = FocusNode();

  MailController get c => widget.controller;

  @override
  void dispose() {
    _searchFocus.dispose();
    _shellFocus.dispose();
    super.dispose();
  }

  void _compose([ComposeRequest request = const ComposeRequest()]) {
    c.closeSettings();
    c.startCompose(request);
  }

  void _openDraft(Draft d) => _compose(
    ComposeRequest(
      heading: 'Your draft',
      draftId: d.id,
      to: d.recipient,
      cc: d.cc,
      subject: d.subject,
      body: d.body,
      inReplyTo: d.inReplyTo,
      references: d.references,
      forwardOf: d.forwardOf,
    ),
  );

  void _openCommandPalette() {
    if (c.activeSettings != null) c.closeSettings();
    showCommandPalette(
      context: context,
      controller: c,
      onCompose: _compose,
      onOpenThread: c.selectThread,
    );
  }

  bool _editingText() {
    final ctx = FocusManager.instance.primaryFocus?.context;
    if (ctx == null) return false;
    return ctx.widget is EditableText ||
        ctx.findAncestorStateOfType<EditableTextState>() != null;
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) return KeyEventResult.ignored;
    final keys = HardwareKeyboard.instance;
    final mod = keys.isControlPressed || keys.isMetaPressed;
    final key = event.logicalKey;

    if (mod && key == LogicalKeyboardKey.keyK) {
      _openCommandPalette();
      return KeyEventResult.handled;
    }
    // Page zoom, as in a browser: Ctrl/Cmd +, - and 0.
    if (mod &&
        (key == LogicalKeyboardKey.equal ||
            key == LogicalKeyboardKey.add ||
            key == LogicalKeyboardKey.numpadAdd)) {
      c.zoomIn();
      return KeyEventResult.handled;
    }
    if (mod &&
        (key == LogicalKeyboardKey.minus ||
            key == LogicalKeyboardKey.numpadSubtract)) {
      c.zoomOut();
      return KeyEventResult.handled;
    }
    if (mod &&
        (key == LogicalKeyboardKey.digit0 ||
            key == LogicalKeyboardKey.numpad0)) {
      c.resetZoom();
      return KeyEventResult.handled;
    }
    if (mod && key == LogicalKeyboardKey.comma) {
      c.openSettings();
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.escape) {
      if (c.activeSettings != null) {
        c.closeSettings();
        return KeyEventResult.handled;
      }
      if (c.bulkSelectedIds.isNotEmpty) {
        c.clearBulkSelection();
        return KeyEventResult.handled;
      }
      if (c.selectedMessageId != null && c.compose == null) {
        c.clearSelection();
        return KeyEventResult.handled;
      }
      return KeyEventResult.ignored;
    }
    if (mod || keys.isAltPressed) return KeyEventResult.ignored;
    if (_editingText() || c.activeSettings != null || c.compose != null) {
      return KeyEventResult.ignored;
    }

    final shift = keys.isShiftPressed;
    if (key == LogicalKeyboardKey.keyJ || key == LogicalKeyboardKey.arrowDown) {
      c.selectAdjacentThread(1);
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.keyK || key == LogicalKeyboardKey.arrowUp) {
      c.selectAdjacentThread(-1);
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.keyC) {
      _compose();
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.slash && !shift) {
      _searchFocus.requestFocus();
      return KeyEventResult.handled;
    }
    if (shift && key == LogicalKeyboardKey.slash) {
      _openCommandPalette();
      return KeyEventResult.handled;
    }

    final msg = c.conversation.lastOrNull?.message;
    if (c.selectedMessageId == null || msg == null) {
      return KeyEventResult.ignored;
    }
    final ids = [for (final t in c.conversation) t.message.id];
    final content = c.contentFor(msg.id);
    final self = c.currentAccountInfo?.address ?? '';
    switch (key) {
      case LogicalKeyboardKey.keyE:
        c.organizeMessages('archive', ids: ids);
      case LogicalKeyboardKey.keyU:
        c.organizeMessages('flags', ids: ids, seen: false);
      case LogicalKeyboardKey.keyS:
        c.organizeMessages('flags', ids: [msg.id], flagged: !msg.flagged);
      case LogicalKeyboardKey.delete ||
          LogicalKeyboardKey.backspace ||
          LogicalKeyboardKey.numberSign:
        c.organizeMessages('delete', ids: ids);
      case LogicalKeyboardKey.digit3 when shift:
        c.organizeMessages('delete', ids: ids);
      case LogicalKeyboardKey.keyR:
        _compose(replyRequest(msg, content, all: false, self: self));
      case LogicalKeyboardKey.keyA:
        _compose(replyRequest(msg, content, all: true, self: self));
      case LogicalKeyboardKey.keyF:
        _compose(forwardRequest(msg, content));
      default:
        return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: c,
      builder: (context, _) {
        if (c.authRequired) return _ConnectScreen(controller: c);
        return Focus(
          focusNode: _shellFocus,
          autofocus: true,
          onKeyEvent: _onKey,
          child: LayoutBuilder(
            builder: (context, constraints) {
              final width = constraints.maxWidth;
              final toasts = ToastOverlay(
                toasts: c.toasts,
                position: c.preferences.notifications.position,
                onDismiss: c.dismissToast,
                onHold: c.holdToast,
                topInset: width < 600 ? 8 : 56,
              );
              if (width < 600) {
                return Stack(
                  children: [
                    _PhoneShell(
                      controller: c,
                      searchFocus: _searchFocus,
                      onCompose: _compose,
                      onOpenDraft: _openDraft,
                      onOpenCommandPalette: _openCommandPalette,
                    ),
                    toasts,
                  ],
                );
              }
              return Material(
                color: ColectoPalette.of(context).ground,
                child: Stack(
                  children: [
                    _Workspace(
                      controller: c,
                      searchFocus: _searchFocus,
                      compactSidebar: width <= 1050,
                      onCompose: _compose,
                      onOpenDraft: _openDraft,
                      onOpenCommandPalette: _openCommandPalette,
                    ),
                    _Presence(
                      visible: c.activeSettings != null,
                      builder: (context, a) =>
                          _SettingsCard(controller: c, animation: a),
                    ),
                    toasts,
                  ],
                ),
              );
            },
          ),
        );
      },
    );
  }
}

// ───────────────────────── Desktop & tablet ─────────────────────────

class _Workspace extends StatefulWidget {
  const _Workspace({
    required this.controller,
    required this.searchFocus,
    required this.compactSidebar,
    required this.onCompose,
    required this.onOpenDraft,
    required this.onOpenCommandPalette,
  });

  final MailController controller;
  final FocusNode searchFocus;
  final bool compactSidebar;
  final ValueChanged<ComposeRequest> onCompose;
  final ValueChanged<Draft> onOpenDraft;
  final VoidCallback onOpenCommandPalette;

  @override
  State<_Workspace> createState() => _WorkspaceState();
}

class _WorkspaceState extends State<_Workspace> {
  double? _listWidth;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = widget.controller;
    final settingsOpen = c.activeSettings != null;
    final style = colectoPlatformStyle(context);
    final sidebar = sidebarWidth(style, compact: widget.compactSidebar);
    void compose() => widget.onCompose(const ComposeRequest());
    // Where Compose lives is part of each platform's idiom: beside search
    // (Colecto, iPadOS), atop the navigation pane (Fluent's New mail,
    // Material's FAB) or in the toolbar (macOS).
    final composeInList =
        style == ColectoPlatformStyle.desktop ||
        style == ColectoPlatformStyle.ios;
    final composeInSidebar =
        style == ColectoPlatformStyle.fluent ||
        style == ColectoPlatformStyle.android;
    return ExcludeFocus(
      excluding: settingsOpen,
      child: Column(
        children: [
          ColectoTopBar(
            controller: c,
            onOpenCommandPalette: widget.onOpenCommandPalette,
            onCompose: compose,
            sidebarWidth: sidebar,
          ),
          Expanded(
            child: LayoutBuilder(
              builder: (context, constraints) {
                final fallback = widget.compactSidebar ? 320.0 : 360.0;
                final maxList = (constraints.maxWidth - sidebar - 360).clamp(
                  260.0,
                  620.0,
                );
                final listWidth = (_listWidth ?? fallback).clamp(
                  260.0,
                  maxList,
                );
                return Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    SidebarPane(
                      controller: c,
                      compact: widget.compactSidebar,
                      width: sidebar,
                      onCompose: composeInSidebar ? compose : null,
                    ),
                    Container(width: 1, color: p.lineSoft),
                    SizedBox(
                      width: listWidth,
                      child: MessageListPane(
                        controller: c,
                        searchFocusNode: widget.searchFocus,
                        onOpenDraft: widget.onOpenDraft,
                        onThreadSelected: c.selectThread,
                        onCompose: composeInList ? compose : null,
                      ),
                    ),
                    _Gutter(
                      onDrag: (dx) => setState(
                        () =>
                            _listWidth = (listWidth + dx).clamp(260.0, maxList),
                      ),
                      onReset: () => setState(() => _listWidth = null),
                    ),
                    Expanded(
                      child: c.compose != null
                          ? ComposerView(
                              key: ValueKey('compose-${c.composeKey}'),
                              controller: c,
                              request: c.compose!,
                              onClose: c.closeCompose,
                            )
                          : ReaderPane(
                              controller: c,
                              onCompose: widget.onCompose,
                            ),
                    ),
                  ],
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}

/// A 1px line you can see, with a wider grip you can hit. Drag to resize,
/// double-click to even it out, or focus it and use the arrow keys.
class _Gutter extends StatefulWidget {
  const _Gutter({required this.onDrag, required this.onReset});

  final ValueChanged<double> onDrag;
  final VoidCallback onReset;

  @override
  State<_Gutter> createState() => _GutterState();
}

class _GutterState extends State<_Gutter> {
  bool _hot = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final lit = _hot || _focused;
    return FocusableActionDetector(
      mouseCursor: SystemMouseCursors.resizeColumn,
      onShowHoverHighlight: (v) => setState(() => _hot = v),
      onShowFocusHighlight: (v) => setState(() => _focused = v),
      shortcuts: const {
        SingleActivator(LogicalKeyboardKey.arrowLeft): _NudgeIntent(-16),
        SingleActivator(LogicalKeyboardKey.arrowRight): _NudgeIntent(16),
        SingleActivator(LogicalKeyboardKey.arrowLeft, shift: true):
            _NudgeIntent(-64),
        SingleActivator(LogicalKeyboardKey.arrowRight, shift: true):
            _NudgeIntent(64),
      },
      actions: {
        _NudgeIntent: CallbackAction<_NudgeIntent>(
          onInvoke: (i) {
            widget.onDrag(i.dx);
            return null;
          },
        ),
      },
      child: Semantics(
        label: 'Resize message list',
        child: GestureDetector(
          behavior: HitTestBehavior.translucent,
          onHorizontalDragUpdate: (d) => widget.onDrag(d.delta.dx),
          onDoubleTap: widget.onReset,
          child: SizedBox(
            width: 1,
            child: OverflowBox(
              maxWidth: 7,
              child: Center(
                child: AnimatedContainer(
                  duration: ColectoMotion.hover,
                  width: 1,
                  color: lit ? Color.lerp(p.line, p.accent, 0.6) : p.lineSoft,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _NudgeIntent extends Intent {
  const _NudgeIntent(this.dx);

  final double dx;
}

/// Keeps a child mounted while it animates out, and builds it with the
/// running animation.
class _Presence extends StatefulWidget {
  const _Presence({required this.visible, required this.builder});

  final bool visible;
  final Widget Function(BuildContext, Animation<double>) builder;

  @override
  State<_Presence> createState() => _PresenceState();
}

class _PresenceState extends State<_Presence>
    with SingleTickerProviderStateMixin {
  late final AnimationController _c = AnimationController(
    vsync: this,
    duration: ColectoMotion.card,
    reverseDuration: const Duration(milliseconds: 160),
    value: widget.visible ? 1 : 0,
  );

  @override
  void didUpdateWidget(_Presence old) {
    super.didUpdateWidget(old);
    final reduce = MediaQuery.maybeDisableAnimationsOf(context) ?? false;
    if (widget.visible != old.visible) {
      if (reduce) {
        _c.value = widget.visible ? 1 : 0;
      } else if (widget.visible) {
        _c.forward();
      } else {
        _c.reverse();
      }
    }
  }

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _c,
      builder: (context, _) => _c.isDismissed
          ? const SizedBox.shrink()
          : widget.builder(context, _c),
    );
  }
}

/// Settings float as a card over the workspace, which stays mounted (and
/// inert) behind a dimmed backdrop. The card rises 6 px and scales from
/// 0.97 in 220 ms; Escape or a click on the backdrop closes it.
class _SettingsCard extends StatelessWidget {
  const _SettingsCard({required this.controller, required this.animation});

  final MailController controller;
  final Animation<double> animation;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final forward = animation.status != AnimationStatus.reverse;
    final t = (forward ? ColectoMotion.easeOut : Curves.easeIn).transform(
      animation.value,
    );
    return Stack(
      children: [
        Positioned.fill(
          child: GestureDetector(
            onTap: controller.closeSettings,
            child: ColoredBox(color: p.scrim.withValues(alpha: p.scrim.a * t)),
          ),
        ),
        LayoutBuilder(
          builder: (context, constraints) {
            final w = (constraints.maxWidth - 56).clamp(0.0, 1080.0);
            final h = (constraints.maxHeight - 56).clamp(0.0, 780.0);
            return Center(
              child: Opacity(
                opacity: t,
                child: Transform.translate(
                  offset: Offset(0, 6 * (1 - t)),
                  child: Transform.scale(
                    scale: 0.97 + 0.03 * t,
                    child: Container(
                      width: w,
                      height: h,
                      decoration: BoxDecoration(
                        color: p.bg,
                        borderRadius: BorderRadius.circular(
                          switch (colectoPlatformStyle(context)) {
                            ColectoPlatformStyle.fluent => 8,
                            ColectoPlatformStyle.macos => 10,
                            ColectoPlatformStyle.android => 28,
                            ColectoPlatformStyle.ios => 14,
                            ColectoPlatformStyle.desktop => 16,
                          },
                        ),
                        border: Border.all(color: p.line),
                        boxShadow: [
                          BoxShadow(
                            color: p.shadow.withValues(
                              alpha: p.isDark ? 0.8 : 0.22,
                            ),
                            blurRadius: 100,
                            spreadRadius: -30,
                            offset: const Offset(0, 40),
                          ),
                          ...p.floatShadow.skip(1),
                        ],
                      ),
                      clipBehavior: Clip.antiAlias,
                      child: FocusScope(
                        autofocus: true,
                        child: SettingsView(controller: controller),
                      ),
                    ),
                  ),
                ),
              ),
            );
          },
        ),
      ],
    );
  }
}

// ───────────────────────────── Phones ─────────────────────────────

class _PhoneShell extends StatefulWidget {
  const _PhoneShell({
    required this.controller,
    required this.searchFocus,
    required this.onCompose,
    required this.onOpenDraft,
    required this.onOpenCommandPalette,
  });

  final MailController controller;
  final FocusNode searchFocus;
  final ValueChanged<ComposeRequest> onCompose;
  final ValueChanged<Draft> onOpenDraft;
  final VoidCallback onOpenCommandPalette;

  @override
  State<_PhoneShell> createState() => _PhoneShellState();
}

/// Phone pages are real routes, so the iOS edge swipe and Android predictive
/// back work. The controller stays the source of truth: routes are pushed
/// when state asks for a page and popped when it no longer does.
class _PhoneShellState extends State<_PhoneShell> {
  Route<void>? _reader;
  Route<void>? _composer;
  Route<void>? _settings;
  int? _composerKey;

  MailController get c => widget.controller;

  @override
  void initState() {
    super.initState();
    c.addListener(_sync);
    WidgetsBinding.instance.addPostFrameCallback((_) => _sync());
  }

  NavigatorState? _navigator;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _navigator = Navigator.maybeOf(context);
  }

  @override
  void dispose() {
    c.removeListener(_sync);
    // Growing past phone width swaps in the desktop shell: take the phone
    // pages down with this one. State stays in the controller.
    final nav = _navigator;
    final pages = [_settings, _composer, _reader];
    _settings = _composer = _reader = null;
    if (nav != null && nav.mounted) {
      for (final r in pages) {
        if (r != null && r.isActive) nav.removeRoute(r);
      }
    }
    super.dispose();
  }

  NavigatorState? get _nav => mounted ? _navigator : null;

  Route<void> _route(Widget page, {bool sheet = false}) {
    switch (colectoPlatformStyle(context)) {
      case ColectoPlatformStyle.ios:
        return CupertinoPageRoute<void>(
          fullscreenDialog: sheet,
          builder: (_) => page,
        );
      case ColectoPlatformStyle.android:
        return MaterialPageRoute<void>(
          fullscreenDialog: sheet,
          builder: (_) => page,
        );
      case ColectoPlatformStyle.desktop ||
          ColectoPlatformStyle.fluent ||
          ColectoPlatformStyle.macos:
        return PageRouteBuilder<void>(
          transitionDuration: ColectoMotion.of(
            context,
            const Duration(milliseconds: 280),
          ),
          reverseTransitionDuration: ColectoMotion.of(
            context,
            const Duration(milliseconds: 200),
          ),
          pageBuilder: (_, _, _) => page,
          transitionsBuilder: (_, a, _, child) {
            final curved = CurvedAnimation(
              parent: a,
              curve: ColectoMotion.easeOut,
              reverseCurve: Curves.easeIn,
            );
            return SlideTransition(
              position: Tween(
                begin: sheet ? const Offset(0, 1) : const Offset(1, 0),
                end: Offset.zero,
              ).animate(curved),
              child: child,
            );
          },
        );
    }
  }

  void _drop(Route<void>? route) {
    final nav = _nav;
    if (route == null || nav == null || !route.isActive) return;
    if (route.isCurrent) {
      nav.pop();
    } else {
      nav.removeRoute(route);
    }
  }

  void _sync() {
    final nav = _nav;
    if (nav == null) return;

    // Reader
    if (c.selectedMessageId != null && _reader == null) {
      final r = _route(
        _PhoneReaderPage(controller: c, onCompose: widget.onCompose),
      );
      _reader = r;
      nav.push(r).whenComplete(() {
        if (_reader == r) {
          _reader = null;
          c.clearSelection();
        }
      });
    } else if (c.selectedMessageId == null && _reader != null) {
      final r = _reader;
      _reader = null;
      _drop(r);
    }

    // Composer
    if (c.compose != null &&
        (_composer == null || _composerKey != c.composeKey)) {
      final old = _composer;
      _composer = null;
      _drop(old);
      _composerKey = c.composeKey;
      final r = _route(
        ComposerView(
          key: ValueKey('compose-${c.composeKey}'),
          controller: c,
          request: c.compose!,
          onClose: c.closeCompose,
          phone: true,
        ),
        sheet: true,
      );
      _composer = r;
      nav.push(r).whenComplete(() {
        if (_composer == r) {
          _composer = null;
          c.closeCompose();
        }
      });
    } else if (c.compose == null && _composer != null) {
      final r = _composer;
      _composer = null;
      _drop(r);
    }

    // Settings
    if (c.activeSettings != null && _settings == null) {
      final r = _route(_PhoneSettingsPage(controller: c), sheet: true);
      _settings = r;
      nav.push(r).whenComplete(() {
        if (_settings == r) {
          _settings = null;
          c.closeSettings();
        }
      });
    } else if (c.activeSettings == null && _settings != null) {
      final r = _settings;
      _settings = null;
      _drop(r);
    }
  }

  // The four folders people visit most; everything else is under More.
  List<Folder> get _tabs {
    Folder? byRole(FolderRole r) =>
        c.folders.where((f) => f.role == r).firstOrNull;
    return [
      for (final r in const [
        FolderRole.inbox,
        FolderRole.drafts,
        FolderRole.sent,
        FolderRole.archive,
      ])
        ?byRole(r),
    ];
  }

  void _more() {
    final rect = Rect.fromCenter(
      center: MediaQuery.sizeOf(context).bottomRight(const Offset(-40, -40)),
      width: 1,
      height: 1,
    );
    showColectoContextMenu(
      context: context,
      globalPosition: rect.center,
      title: 'Mailboxes',
      items: [
        for (final f in c.folders.where((f) => !_tabs.contains(f)))
          ColectoMenuItem(
            label: f.name,
            icon: folderIcon(f.role),
            checked: f.id == c.activeFolder,
            onSelected: () => c.selectFolder(f.id),
          ),
        const ColectoMenuItem.divider(),
        if (c.pendingApprovalCount > 0)
          ColectoMenuItem(
            label: 'Send approvals (${c.pendingApprovalCount})',
            icon: LucideIcons.shieldAlert,
            onSelected: () => c.openSettings(SettingsSection.sendApprovals),
          ),
        ColectoMenuItem(
          label: 'Settings',
          icon: LucideIcons.settings,
          onSelected: () => c.openSettings(),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final style = colectoPlatformStyle(context);
    final tabs = _tabs;
    var index = tabs.indexWhere((f) => f.id == c.activeFolder);
    if (index < 0) index = tabs.length;

    void onTab(int i) {
      if (i >= tabs.length) {
        _more();
      } else {
        c.selectFolder(tabs[i].id);
      }
    }

    final title = c.currentFolderName;
    final account = Builder(
      builder: (ctx) => ColectoPressable(
        onTap: () => _accountSheet(ctx),
        tooltip: 'Accounts',
        focusRadius: 999,
        builder: (context, s) => Padding(
          padding: const EdgeInsets.all(6),
          child: ColectoAvatar(
            name: c.currentAccountInfo?.displayName ?? '?',
            hueKey: c.currentAccountInfo?.address,
            size: 30,
          ),
        ),
      ),
    );

    final List<Widget> header = switch (style) {
      ColectoPlatformStyle.ios => [
        CupertinoSliverNavigationBar(
          largeTitle: Text(title),
          leading: account,
          trailing: CupertinoButton(
            padding: EdgeInsets.zero,
            onPressed: () => widget.onCompose(const ComposeRequest()),
            child: Icon(LucideIcons.squarePen, color: p.accent, size: 22),
          ),
          backgroundColor: p.bg.withValues(alpha: 0.86),
          border: Border(bottom: BorderSide(color: p.lineSoft, width: 0.5)),
        ),
      ],
      ColectoPlatformStyle.android => [
        SliverAppBar.large(
          title: Text(title),
          backgroundColor: p.bg,
          surfaceTintColor: const Color(0x00000000),
          actions: [account, const SizedBox(width: 8)],
        ),
      ],
      _ => [
        SliverToBoxAdapter(
          child: SafeArea(
            bottom: false,
            child: Container(
              height: 54,
              padding: const EdgeInsets.only(left: 14, right: 8),
              child: Row(
                children: [
                  const ColectoMark(size: 26),
                  const Spacer(),
                  ColectoIconButton(
                    icon: LucideIcons.search,
                    tooltip: 'Commands',
                    size: 18,
                    onPressed: widget.onOpenCommandPalette,
                  ),
                  account,
                  const SizedBox(width: 4),
                  ColectoPressable(
                    onTap: () => widget.onCompose(const ComposeRequest()),
                    tooltip: 'Compose',
                    pressScale: 0.95,
                    builder: (context, s) => Container(
                      width: 40,
                      height: 40,
                      decoration: BoxDecoration(
                        color: s.hovered ? p.accentHi : p.accent,
                        borderRadius: BorderRadius.circular(8),
                      ),
                      child: Icon(
                        LucideIcons.squarePen,
                        size: 18,
                        color: p.accentInk,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
        SliverToBoxAdapter(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 6, 16, 4),
            child: Text(
              title,
              style: kStyle(
                28,
                weight: 660,
                width: 108,
                color: p.ink,
                letterSpacing: -0.7,
              ),
            ),
          ),
        ),
      ],
    };

    final list = MessageListPane(
      controller: c,
      searchFocusNode: widget.searchFocus,
      onOpenDraft: widget.onOpenDraft,
      onThreadSelected: c.selectThread,
      headerSlivers: header,
      phone: true,
    );

    final destinations = [
      for (final f in tabs) (folderIcon(f.role), f.name, f.unread),
      (LucideIcons.ellipsis, 'More', c.pendingApprovalCount),
    ];

    final Widget bottom = switch (style) {
      ColectoPlatformStyle.ios => CupertinoTabBar(
        currentIndex: index,
        onTap: onTab,
        activeColor: p.accent,
        inactiveColor: p.ink3,
        backgroundColor: p.bg.withValues(alpha: 0.86),
        border: Border(top: BorderSide(color: p.lineSoft, width: 0.5)),
        items: [
          for (final d in destinations)
            BottomNavigationBarItem(
              icon: Badge(
                isLabelVisible: d.$3 > 0,
                backgroundColor: p.accent,
                smallSize: 7,
                child: Icon(d.$1),
              ),
              label: d.$2,
            ),
        ],
      ),
      ColectoPlatformStyle.android => NavigationBar(
        selectedIndex: index,
        onDestinationSelected: onTab,
        backgroundColor: p.bg,
        indicatorColor: p.select,
        destinations: [
          for (final d in destinations)
            NavigationDestination(
              icon: Badge(
                isLabelVisible: d.$3 > 0,
                label: Text('${d.$3}'),
                child: Icon(d.$1, color: p.ink2),
              ),
              selectedIcon: Icon(d.$1, color: p.accentHi),
              label: d.$2,
            ),
        ],
      ),
      _ => _FolderBar(destinations: destinations, index: index, onTap: onTab),
    };

    return Scaffold(
      backgroundColor: p.bg,
      body: list,
      bottomNavigationBar: bottom,
      floatingActionButton: style == ColectoPlatformStyle.android
          ? FloatingActionButton.extended(
              onPressed: () => widget.onCompose(const ComposeRequest()),
              backgroundColor: p.accent,
              foregroundColor: p.accentInk,
              icon: const Icon(LucideIcons.squarePen),
              label: const Text('Compose'),
            )
          : null,
    );
  }

  void _accountSheet(BuildContext ctx) => showAccountMenu(ctx, c);
}

/// Colecto's phone folder bar: icons over labels, the active one in mint.
class _FolderBar extends StatelessWidget {
  const _FolderBar({
    required this.destinations,
    required this.index,
    required this.onTap,
  });

  final List<(IconData, String, int)> destinations;
  final int index;
  final ValueChanged<int> onTap;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Container(
      decoration: BoxDecoration(
        color: p.bg,
        border: Border(top: BorderSide(color: p.lineSoft)),
      ),
      padding: EdgeInsets.fromLTRB(
        8,
        4,
        8,
        4 + MediaQuery.paddingOf(context).bottom,
      ),
      child: Row(
        children: [
          for (var i = 0; i < destinations.length; i++)
            Expanded(
              child: ColectoPressable(
                onTap: () => onTap(i),
                selected: i == index,
                semanticLabel: destinations[i].$2,
                builder: (context, s) {
                  final on = i == index;
                  final color = on ? p.accent : (s.hovered ? p.ink : p.ink3);
                  return SizedBox(
                    height: 56,
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Stack(
                          clipBehavior: Clip.none,
                          children: [
                            Icon(destinations[i].$1, size: 20, color: color),
                            if (destinations[i].$3 > 0)
                              Positioned(
                                top: -1,
                                right: -3,
                                child: Container(
                                  width: 7,
                                  height: 7,
                                  decoration: BoxDecoration(
                                    color: p.accent,
                                    shape: BoxShape.circle,
                                    border: Border.all(color: p.bg),
                                  ),
                                ),
                              ),
                          ],
                        ),
                        const SizedBox(height: 3),
                        Text(
                          destinations[i].$2,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: kStyle(11.5, weight: 560, color: color),
                        ),
                      ],
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

class _PhoneReaderPage extends StatelessWidget {
  const _PhoneReaderPage({required this.controller, required this.onCompose});

  final MailController controller;
  final ValueChanged<ComposeRequest> onCompose;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Scaffold(
      backgroundColor: p.bg,
      body: SafeArea(
        bottom: false,
        child: ListenableBuilder(
          listenable: controller,
          builder: (context, _) => ReaderPane(
            controller: controller,
            onCompose: onCompose,
            showBackButton: true,
            onBack: () => Navigator.of(context).maybePop(),
          ),
        ),
      ),
    );
  }
}

class _PhoneSettingsPage extends StatelessWidget {
  const _PhoneSettingsPage({required this.controller});

  final MailController controller;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Scaffold(
      backgroundColor: p.bg,
      body: SafeArea(
        bottom: false,
        child: ListenableBuilder(
          listenable: controller,
          builder: (context, _) =>
              SettingsView(controller: controller, phone: true),
        ),
      ),
    );
  }
}

// ───────────────────────────── Connect ─────────────────────────────

/// Colecto's connect page: the mark, one line on what this is, and the two
/// fields it needs.
class _ConnectScreen extends StatefulWidget {
  const _ConnectScreen({required this.controller});

  final MailController controller;

  @override
  State<_ConnectScreen> createState() => _ConnectScreenState();
}

class _ConnectScreenState extends State<_ConnectScreen> {
  late final _url = TextEditingController(text: widget.controller.daemonUrl);
  final _token = TextEditingController();
  bool _busy = false;

  @override
  void dispose() {
    _url.dispose();
    _token.dispose();
    super.dispose();
  }

  Future<void> _connect() async {
    if (_busy) return;
    setState(() => _busy = true);
    await widget.controller.authenticateDaemon(_url.text, _token.text);
    if (mounted) setState(() => _busy = false);
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final error = widget.controller.error;
    return Scaffold(
      backgroundColor: p.ground,
      body: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(16),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 440),
            child: AutofillGroup(
              child: Container(
                padding: const EdgeInsets.all(32),
                decoration: BoxDecoration(
                  color: p.bg,
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(color: p.line),
                  boxShadow: p.floatShadow,
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    const Align(
                      alignment: Alignment.centerLeft,
                      child: ColectoMark(size: 44),
                    ),
                    const SizedBox(height: 18),
                    Text(
                      'Connect to Colecto',
                      style: kStyle(
                        22,
                        weight: 640,
                        width: 106,
                        color: p.ink,
                        letterSpacing: -0.4,
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      'Enter the address of your colecto-daemon and the '
                      'access token it printed when it started.',
                      style: kStyle(13.5, color: p.ink2, height: 1.55),
                    ),
                    const SizedBox(height: 20),
                    ColectoTextField(
                      controller: _url,
                      labelText: 'Daemon address',
                      hintText: 'http://127.0.0.1:4173',
                      keyboardType: TextInputType.url,
                      textInputAction: TextInputAction.next,
                      autofillHints: const [AutofillHints.url],
                    ),
                    const SizedBox(height: 14),
                    ColectoTextField(
                      controller: _token,
                      labelText: 'Access token',
                      obscureText: true,
                      autofocus: true,
                      textInputAction: TextInputAction.go,
                      autofillHints: const [AutofillHints.password],
                      onSubmitted: (_) => _connect(),
                    ),
                    if (error != null) ...[
                      const SizedBox(height: 14),
                      ColectoErrorBox(message: error),
                    ],
                    const SizedBox(height: 20),
                    ColectoButton(
                      label: _busy ? 'Connecting…' : 'Connect',
                      variant: ColectoButtonVariant.primary,
                      expand: true,
                      onPressed: _busy ? null : _connect,
                    ),
                    const SizedBox(height: 10),
                    ColectoButton(
                      label: 'Try the demo instead',
                      variant: ColectoButtonVariant.ghost,
                      expand: true,
                      onPressed: _busy
                          ? null
                          : () => widget.controller.switchBackend(
                              kind: BackendKind.demo,
                            ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The navigation pane's width in each design.
double sidebarWidth(ColectoPlatformStyle style, {required bool compact}) =>
    switch ((style, compact)) {
      (ColectoPlatformStyle.android, true) => 80,
      (_, true) => 56,
      (ColectoPlatformStyle.android, false) => 256,
      (ColectoPlatformStyle.ios, false) => 260,
      (ColectoPlatformStyle.fluent, false) => 240,
      _ => 220,
    };
