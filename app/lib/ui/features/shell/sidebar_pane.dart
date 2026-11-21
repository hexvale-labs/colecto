import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/mail_models.dart';
import '../../core/kit/kit.dart';
import '../../core/theme/colecto_palette.dart';
import 'mail_controller.dart';

IconData folderIcon(FolderRole role) => switch (role) {
  FolderRole.inbox => LucideIcons.inbox,
  FolderRole.sent => LucideIcons.send,
  FolderRole.drafts => LucideIcons.fileText,
  FolderRole.archive => LucideIcons.archive,
  FolderRole.trash => LucideIcons.trash2,
  FolderRole.junk => LucideIcons.octagonAlert,
  FolderRole.other => LucideIcons.folder,
};

/// Counts cap at "999+" so badges never blow out.
String countLabel(int n) => n > 999 ? '999+' : '$n';

/// The navigation pane: folders, send approvals (only while something
/// waits) and Settings, drawn in each design's idiom:
/// - Colecto: Colecto nav items (raise when active, mint icon).
/// - Fluent: NavigationView items with the accent selection pill, under a
///   New mail button.
/// - macOS: a source list with accent-tinted icons under a section header.
/// - Material 3: a navigation drawer with pill indicators under the
///   Compose FAB.
/// - Cupertino: iPadOS sidebar rows, the selected one filled with accent.
/// In the compact tier it becomes an icon rail.
class SidebarPane extends StatelessWidget {
  const SidebarPane({
    super.key,
    required this.controller,
    this.compact = false,
    this.width,
    this.onCompose,
  });

  final MailController controller;
  final bool compact;
  final double? width;

  /// Shown at the top of the pane when the design puts Compose there.
  final VoidCallback? onCompose;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = controller;
    final style = colectoPlatformStyle(context);
    final pad = compact ? 8.0 : 10.0;
    return Container(
      width: width ?? (compact ? 56 : 220),
      color: style == ColectoPlatformStyle.fluent ? p.ground : p.sunk,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (onCompose != null)
            Padding(
              padding: EdgeInsets.fromLTRB(pad, 12, pad, 0),
              child: _ComposeButton(compact: compact, onPressed: onCompose!),
            ),
          Expanded(
            child: ListView(
              padding: EdgeInsets.fromLTRB(pad, 12, pad, 12),
              children: [
                if (!compact) _header(p, style),
                for (final folder in c.folders)
                  _NavItem(
                    icon: folderIcon(folder.role),
                    label: folder.name,
                    count: folder.unread > 0
                        ? folder.unread
                        : (folder.role == FolderRole.drafts ? folder.total : 0),
                    strongCount: folder.unread > 0,
                    selected: c.activeFolder == folder.id,
                    compact: compact,
                    onTap: () => c.selectFolder(folder.id),
                    onMenu: (pos) => showColectoContextMenu(
                      context: context,
                      globalPosition: pos,
                      title: folder.name,
                      items: [
                        ColectoMenuItem(
                          label: 'Check for new mail',
                          icon: LucideIcons.refreshCw,
                          onSelected: () async {
                            await c.selectFolder(folder.id);
                            await c.loadMailbox(refresh: true);
                          },
                        ),
                      ],
                    ),
                  ),
              ],
            ),
          ),
          Padding(
            padding: EdgeInsets.fromLTRB(pad, 8, pad, 10),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (c.pendingApprovalCount > 0)
                  _NavItem(
                    icon: LucideIcons.shieldAlert,
                    label: 'Send approvals',
                    count: c.pendingApprovalCount,
                    strongCount: true,
                    accentCount: true,
                    selected: c.activeSettings == SettingsSection.sendApprovals,
                    compact: compact,
                    onTap: () => c.openSettings(SettingsSection.sendApprovals),
                  ),
                _NavItem(
                  icon: LucideIcons.settings,
                  label: 'Settings',
                  selected: false,
                  compact: compact,
                  shortcut: '${colectoModKey(context)} ,',
                  onTap: () => c.openSettings(),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _header(ColectoPalette p, ColectoPlatformStyle style) {
    return switch (style) {
      ColectoPlatformStyle.ios => Padding(
        padding: const EdgeInsets.fromLTRB(10, 0, 10, 10),
        child: Text(
          'Mailboxes',
          style: kStyle(28, weight: 700, color: p.ink, letterSpacing: 0.36),
        ),
      ),
      ColectoPlatformStyle.macos => Padding(
        padding: const EdgeInsets.fromLTRB(10, 0, 10, 6),
        child: Text('Mailboxes', style: kStyle(11, weight: 700, color: p.ink3)),
      ),
      ColectoPlatformStyle.android => Padding(
        padding: const EdgeInsets.fromLTRB(16, 4, 16, 10),
        child: Text('Mail', style: kStyle(14, weight: 500, color: p.ink2)),
      ),
      _ => Padding(
        padding: const EdgeInsets.fromLTRB(10, 2, 10, 8),
        child: Text('Mailboxes', style: kStyle(12, weight: 600, color: p.ink3)),
      ),
    };
  }
}

class _ComposeButton extends StatelessWidget {
  const _ComposeButton({required this.compact, required this.onPressed});

  final bool compact;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    if (compact) {
      return ColectoIconButton(
        icon: LucideIcons.squarePen,
        tooltip: 'New message (C)',
        onPressed: onPressed,
      );
    }
    if (colectoPlatformStyle(context) == ColectoPlatformStyle.android) {
      return Align(
        alignment: Alignment.centerLeft,
        child: FloatingActionButton.extended(
          heroTag: null,
          elevation: 1,
          onPressed: onPressed,
          icon: const Icon(LucideIcons.squarePen),
          label: const Text('Compose'),
        ),
      );
    }
    return ColectoButton(
      label: 'New mail',
      icon: LucideIcons.squarePen,
      variant: ColectoButtonVariant.primary,
      expand: true,
      tooltip: 'New message (C)',
      onPressed: onPressed,
    );
  }
}

class _NavItem extends StatelessWidget {
  const _NavItem({
    required this.icon,
    required this.label,
    required this.selected,
    required this.compact,
    required this.onTap,
    this.count = 0,
    this.strongCount = false,
    this.accentCount = false,
    this.onMenu,
    this.shortcut,
  });

  final IconData icon;
  final String label;
  final bool selected;
  final bool compact;
  final VoidCallback onTap;
  final int count;
  final bool strongCount;
  final bool accentCount;
  final ValueChanged<Offset>? onMenu;
  final String? shortcut;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final style = colectoPlatformStyle(context);
    final fluent = style == ColectoPlatformStyle.fluent;
    final mac = style == ColectoPlatformStyle.macos;
    final material = style == ColectoPlatformStyle.android;
    final ios = style == ColectoPlatformStyle.ios;
    final radius = switch (style) {
      ColectoPlatformStyle.fluent => 4.0,
      ColectoPlatformStyle.macos => 6.0,
      ColectoPlatformStyle.android => 28.0,
      ColectoPlatformStyle.ios => 10.0,
      ColectoPlatformStyle.desktop => 8.0,
    };
    final height = compact
        ? (material ? 48.0 : 40.0)
        : switch (style) {
            ColectoPlatformStyle.fluent => 36.0,
            ColectoPlatformStyle.macos => 28.0,
            ColectoPlatformStyle.android => 48.0,
            ColectoPlatformStyle.ios => 44.0,
            ColectoPlatformStyle.desktop => 34.0,
          };
    return Padding(
      padding: const EdgeInsets.only(bottom: 2),
      child: ColectoPressable(
        onTap: onTap,
        onSecondaryTapDown: onMenu,
        selected: selected,
        semanticLabel: count > 0 ? '$label, $count' : label,
        focusRadius: radius,
        pressScale: ios || material ? 1 : ColectoMotion.pressScale,
        tooltip: compact
            ? (count > 0 ? '$label (${countLabel(count)})' : label)
            : (shortcut == null ? null : '$label  $shortcut'),
        builder: (context, s) {
          // iPadOS fills the selected row with the accent and inverts it.
          final inverted = ios && selected;
          final fg = inverted
              ? p.accentInk
              : material && selected
              ? p.accentHi
              : selected || s.hovered
              ? p.ink
              : p.ink2;
          final iconColor = inverted
              ? p.accentInk
              : mac || ios
              ? p.accent
              : fluent
              ? fg
              : material
              ? (selected ? p.accentHi : p.ink2)
              : selected
              ? p.accent
              : (s.hovered ? p.ink2 : p.ink3);
          final fill = inverted
              ? p.accent
              : material
              ? (selected
                    ? p.select
                    : s.hovered
                    ? p.ink.withValues(alpha: 0.06)
                    : const Color(0x00000000))
              : fluent || mac || ios
              ? (selected
                    ? p.ink.withValues(alpha: p.isDark ? 0.09 : 0.07)
                    : s.hovered
                    ? p.ink.withValues(alpha: 0.05)
                    : const Color(0x00000000))
              : (selected || s.hovered ? p.raise : const Color(0x00000000));
          final item = AnimatedContainer(
            duration: const Duration(milliseconds: 120),
            height: height,
            padding: EdgeInsets.symmetric(
              horizontal: compact ? 0 : (material ? 16 : 10),
            ),
            decoration: BoxDecoration(
              color: fill,
              borderRadius: BorderRadius.circular(radius),
            ),
            child: compact
                ? Stack(
                    alignment: Alignment.center,
                    children: [
                      Icon(icon, size: 18, color: iconColor),
                      if (count > 0 && strongCount)
                        Positioned(
                          top: height / 2 - 12,
                          right: 9,
                          child: Container(
                            width: 6,
                            height: 6,
                            decoration: BoxDecoration(
                              color: p.accent,
                              shape: BoxShape.circle,
                              border: Border.all(color: p.sunk),
                            ),
                          ),
                        ),
                    ],
                  )
                : Row(
                    children: [
                      Icon(icon, size: ios ? 19 : 16, color: iconColor),
                      SizedBox(width: material ? 12 : 10),
                      Expanded(
                        child: Text(
                          label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: kStyle(
                            ios ? 15 : (mac ? 13 : (material ? 14 : 13.5)),
                            weight: ios || mac
                                ? 400
                                : (material && selected ? 600 : 560),
                            color: fg,
                          ),
                        ),
                      ),
                      if (count > 0) ...[
                        const SizedBox(width: 8),
                        Text(
                          countLabel(count),
                          style: kStyle(
                            12,
                            weight: strongCount ? 640 : 500,
                            color: inverted
                                ? p.accentInk
                                : accentCount
                                ? p.accent
                                : (strongCount ? p.ink2 : p.ink3),
                            features: tabularFigures,
                          ),
                        ),
                      ],
                    ],
                  ),
          );
          if (!fluent || !selected) return item;
          return Stack(
            children: [
              item,
              Positioned(
                left: 0,
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
              ),
            ],
          );
        },
      ),
    );
  }
}
