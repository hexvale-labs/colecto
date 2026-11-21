import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/mail_models.dart';
import '../../core/format/when.dart';
import '../../core/theme/colecto_palette.dart';
import '../../core/kit/kit.dart';
import 'mail_controller.dart';
import 'sidebar_pane.dart';

class _Command {
  const _Command({
    required this.title,
    required this.group,
    required this.icon,
    required this.onRun,
    this.hint,
    this.shortcut,
    this.keywords = '',
  });

  final String title;
  final String group;
  final IconData icon;
  final VoidCallback onRun;
  final String? hint;
  final String? shortcut;
  final String keywords;
}

/// The `Cmd/Ctrl+K` command palette. It opens instantly, because it is used
/// hundreds of times a day (Colecto `DESIGN.md`, "Motion").
Future<void> showCommandPalette({
  required BuildContext context,
  required MailController controller,
  required VoidCallback onCompose,
  required ValueChanged<ConversationThread> onOpenThread,
}) {
  final p = ColectoPalette.of(context);
  return showGeneralDialog<void>(
    context: context,
    barrierDismissible: true,
    barrierLabel: 'Close command palette',
    barrierColor: p.scrim,
    transitionDuration: Duration.zero,
    pageBuilder: (ctx, _, _) => _CommandPalette(
      controller: controller,
      onCompose: onCompose,
      onOpenThread: onOpenThread,
    ),
  );
}

class _CommandPalette extends StatefulWidget {
  const _CommandPalette({
    required this.controller,
    required this.onCompose,
    required this.onOpenThread,
  });

  final MailController controller;
  final VoidCallback onCompose;
  final ValueChanged<ConversationThread> onOpenThread;

  @override
  State<_CommandPalette> createState() => _CommandPaletteState();
}

class _CommandPaletteState extends State<_CommandPalette> {
  final _query = TextEditingController();
  final _scroll = ScrollController();
  int _active = 0;

  MailController get c => widget.controller;

  @override
  void dispose() {
    _query.dispose();
    _scroll.dispose();
    super.dispose();
  }

  List<_Command> _commands() {
    final q = _query.text.trim();
    final lower = q.toLowerCase();
    final selected = c.conversation.lastOrNull?.message;
    final mod = colectoModKey(context);

    final all = <_Command>[
      _Command(
        title: 'New message',
        group: 'Mail',
        icon: LucideIcons.squarePen,
        shortcut: 'C',
        keywords: 'compose write',
        onRun: widget.onCompose,
      ),
      _Command(
        title: 'Check for new mail',
        group: 'Mail',
        icon: LucideIcons.refreshCw,
        keywords: 'refresh sync',
        onRun: () => c.loadMailbox(refresh: true),
      ),
      if (selected != null) ...[
        _Command(
          title: 'Archive conversation',
          group: 'Mail',
          icon: LucideIcons.archive,
          shortcut: 'E',
          onRun: () => c.organizeMessages(
            'archive',
            ids: [for (final t in c.conversation) t.message.id],
          ),
        ),
        _Command(
          title: 'Summarize with AI',
          group: 'Mail',
          icon: LucideIcons.sparkles,
          keywords: 'ai summary',
          onRun: () => c.generateSummary(selected.id),
        ),
      ],
      for (final folder in c.folders)
        _Command(
          title: folder.name,
          group: 'Go to',
          icon: folderIcon(folder.role),
          hint: folder.unread > 0 ? '${folder.unread} unread' : null,
          keywords: 'folder mailbox open',
          onRun: () => c.selectFolder(folder.id),
        ),
      for (final label in c.labels.keys)
        _Command(
          title: label,
          group: 'Labels',
          icon: LucideIcons.tag,
          hint: 'Filter',
          keywords: 'label filter',
          onRun: () => c.selectLabel(label),
        ),
      for (final acc in c.enabledAccounts)
        if (acc.id != c.activeAccount)
          _Command(
            title: 'Switch to ${acc.displayName}',
            group: 'Accounts',
            icon: LucideIcons.userRound,
            hint: acc.address,
            keywords: 'account',
            onRun: () => c.selectAccount(acc.id),
          ),
      for (final section in c.visibleSettingsSections)
        _Command(
          title: section.label,
          group: 'Settings',
          icon: section.icon,
          shortcut: section == SettingsSection.accounts ? '$mod ,' : null,
          keywords: 'settings preferences',
          onRun: () => c.openSettings(section),
        ),
      _Command(
        title: switch (c.themeMode) {
          ThemeMode.dark => 'Use light theme',
          ThemeMode.light => 'Match system theme',
          ThemeMode.system => 'Use dark theme',
        },
        group: 'Settings',
        icon: LucideIcons.sunMoon,
        keywords: 'theme appearance dark light mode',
        onRun: () => c.setThemeMode(switch (c.themeMode) {
          ThemeMode.dark => ThemeMode.light,
          ThemeMode.light => ThemeMode.system,
          ThemeMode.system => ThemeMode.dark,
        }),
      ),
    ];

    if (q.isEmpty) return all;

    final hits = <_Command>[
      _Command(
        title: 'Search ${c.currentFolderName} for “$q”',
        group: 'Search',
        icon: LucideIcons.search,
        onRun: () => c.setSearchQuery(q),
      ),
      for (final t
          in c.threads
              .where(
                (t) =>
                    t.subject.toLowerCase().contains(lower) ||
                    t.participants.any((n) => n.toLowerCase().contains(lower)),
              )
              .take(5))
        _Command(
          title: t.subject.isEmpty ? '(No subject)' : t.subject,
          group: 'Messages',
          icon: LucideIcons.mail,
          hint: '${t.participants.first} · ${when(t.latestDate).label}',
          onRun: () => widget.onOpenThread(t),
        ),
    ];
    final words = lower.split(RegExp(r'\s+'));
    final matched = all.where((cmd) {
      final hay = '${cmd.title} ${cmd.group} ${cmd.keywords}'.toLowerCase();
      return words.every(hay.contains);
    });
    return [...matched, ...hits];
  }

  void _run(_Command cmd) {
    Navigator.of(context).pop();
    cmd.onRun();
  }

  void _move(int delta, int length) {
    if (length == 0) return;
    setState(() => _active = (_active + delta) % length);
    if (_active < 0) _active += length;
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final commands = _commands();
    if (_active >= commands.length) _active = 0;
    final size = MediaQuery.sizeOf(context);

    final rows = <Widget>[];
    String? group;
    for (var i = 0; i < commands.length; i++) {
      final cmd = commands[i];
      if (cmd.group != group) {
        group = cmd.group;
        rows.add(
          Padding(
            padding: EdgeInsets.fromLTRB(12, rows.isEmpty ? 4 : 10, 12, 4),
            child: Text(group, style: kStyle(12, weight: 600, color: p.ink3)),
          ),
        );
      }
      rows.add(_row(p, cmd, i == _active, i));
    }

    return Align(
      alignment: Alignment.topCenter,
      child: Padding(
        padding: EdgeInsets.only(top: size.height * 0.12, left: 12, right: 12),
        child: Material(
          type: MaterialType.transparency,
          child: CallbackShortcuts(
            bindings: {
              const SingleActivator(LogicalKeyboardKey.arrowDown): () =>
                  _move(1, commands.length),
              const SingleActivator(LogicalKeyboardKey.arrowUp): () =>
                  _move(-1, commands.length),
              const SingleActivator(LogicalKeyboardKey.escape): () =>
                  Navigator.of(context).pop(),
            },
            child: Container(
              width: 600,
              constraints: BoxConstraints(
                maxHeight: (size.height * 0.52 + 110).clamp(240, 530),
              ),
              decoration: BoxDecoration(
                color: p.float,
                borderRadius: BorderRadius.circular(14),
                border: Border.all(color: p.line),
                boxShadow: p.floatShadow,
              ),
              clipBehavior: Clip.antiAlias,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Container(
                    height: 52,
                    padding: const EdgeInsets.only(left: 16, right: 12),
                    decoration: BoxDecoration(
                      border: Border(bottom: BorderSide(color: p.lineSoft)),
                    ),
                    child: Row(
                      children: [
                        Icon(LucideIcons.search, size: 17, color: p.ink3),
                        const SizedBox(width: 10),
                        Expanded(
                          child: TextField(
                            controller: _query,
                            autofocus: true,
                            cursorColor: p.accent,
                            style: kStyle(16, color: p.ink),
                            decoration: InputDecoration(
                              isCollapsed: true,
                              border: InputBorder.none,
                              hintText: 'Search mail or run a command',
                              hintStyle: kStyle(16, color: p.ink3),
                            ),
                            onChanged: (_) => setState(() => _active = 0),
                            onSubmitted: (_) {
                              if (commands.isNotEmpty) {
                                _run(commands[_active]);
                              }
                            },
                          ),
                        ),
                        const ColectoKbd('Esc'),
                      ],
                    ),
                  ),
                  Flexible(
                    child: commands.isEmpty
                        ? Padding(
                            padding: const EdgeInsets.all(18),
                            child: Align(
                              alignment: Alignment.centerLeft,
                              child: Text(
                                'Nothing matches “${_query.text.trim()}”.',
                                style: kStyle(13.5, color: p.ink3),
                              ),
                            ),
                          )
                        : ListView(
                            controller: _scroll,
                            shrinkWrap: true,
                            padding: const EdgeInsets.all(6),
                            children: rows,
                          ),
                  ),
                  Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 14,
                      vertical: 9,
                    ),
                    decoration: BoxDecoration(
                      border: Border(top: BorderSide(color: p.lineSoft)),
                    ),
                    child: Row(
                      children: [
                        _hint(p, ['↑', '↓'], 'to move'),
                        const SizedBox(width: 16),
                        _hint(p, ['↵'], 'to run'),
                        const SizedBox(width: 16),
                        _hint(p, ['Esc'], 'to close'),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _hint(ColectoPalette p, List<String> keys, String text) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      for (final k in keys) ...[ColectoKbd(k), const SizedBox(width: 3)],
      const SizedBox(width: 2),
      Text(text, style: kStyle(12, color: p.ink3)),
    ],
  );

  Widget _row(ColectoPalette p, _Command cmd, bool active, int index) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onHover: (_) {
        if (_active != index) setState(() => _active = index);
      },
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => _run(cmd),
        child: _EnsureVisible(
          active: active,
          child: Container(
            height: 36,
            padding: const EdgeInsets.symmetric(horizontal: 12),
            decoration: BoxDecoration(
              color: active ? p.raise : const Color(0x00000000),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Row(
              children: [
                Icon(cmd.icon, size: 15, color: active ? p.ink2 : p.ink3),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    cmd.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: kStyle(13.5, color: active ? p.ink : p.ink2),
                  ),
                ),
                if (cmd.hint != null) ...[
                  const SizedBox(width: 12),
                  ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 220),
                    child: Text(
                      cmd.hint!,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: kStyle(12, color: p.ink3),
                    ),
                  ),
                ],
                if (cmd.shortcut != null) ...[
                  const SizedBox(width: 10),
                  ColectoKbd(cmd.shortcut!),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Keeps the keyboard-highlighted command scrolled into view.
class _EnsureVisible extends StatefulWidget {
  const _EnsureVisible({required this.active, required this.child});

  final bool active;
  final Widget child;

  @override
  State<_EnsureVisible> createState() => _EnsureVisibleState();
}

class _EnsureVisibleState extends State<_EnsureVisible> {
  @override
  void didUpdateWidget(_EnsureVisible old) {
    super.didUpdateWidget(old);
    if (widget.active && !old.active) {
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
  Widget build(BuildContext context) => widget.child;
}
