import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/account_models.dart';
import '../../../domain/models/preferences_models.dart';
import '../../core/theme/colecto_palette.dart';
import '../../core/widgets/colecto_mark.dart';
import '../../core/kit/kit.dart';
import 'mail_controller.dart';

/// Colecto's slim top bar: brand on the left, the command palette trigger
/// centred, the account switcher and Settings on the right. These are host
/// controls and do not move.
class ColectoTopBar extends StatelessWidget {
  const ColectoTopBar({
    super.key,
    required this.controller,
    required this.onOpenCommandPalette,
    required this.onCompose,
    this.sidebarWidth = 220,
  });

  final MailController controller;
  final VoidCallback onOpenCommandPalette;
  final VoidCallback onCompose;

  /// Lets the macOS toolbar line its title up over the message list.
  final double sidebarWidth;

  @override
  Widget build(BuildContext context) {
    return switch (colectoPlatformStyle(context)) {
      ColectoPlatformStyle.fluent => _fluent(context),
      ColectoPlatformStyle.macos => _macos(context),
      ColectoPlatformStyle.android => _material(context),
      ColectoPlatformStyle.ios => _cupertino(context),
      ColectoPlatformStyle.desktop => _colecto(context),
    };
  }

  Widget _settingsButton(BuildContext context) => ColectoIconButton(
    icon: LucideIcons.settings,
    tooltip: 'Settings (${colectoModKey(context)}+,)',
    onPressed: () => controller.openSettings(),
  );

  Widget _goHome(Widget child) => ColectoPressable(
    onTap: () {
      controller.closeSettings();
      controller.selectFolder('inbox');
    },
    tooltip: 'Go to Inbox',
    focusRadius: 6,
    pressScale: 0.98,
    builder: (context, s) => child,
  );

  /// Colecto: brand left, the command trigger centred, account and Settings
  /// right.
  Widget _colecto(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = controller;
    return Container(
      height: 46,
      padding: const EdgeInsets.only(left: 14, right: 8),
      decoration: BoxDecoration(
        color: p.ground,
        border: Border(bottom: BorderSide(color: p.lineSoft)),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final roomy = constraints.maxWidth > 820;
          return Row(
            children: [
              Expanded(
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: _goHome(
                    roomy ? const ColectoBrand() : const ColectoMark(size: 24),
                  ),
                ),
              ),
              // `grid-template-columns: 1fr minmax(0, 520px) 1fr`.
              SizedBox(
                width: (constraints.maxWidth * 0.42).clamp(180.0, 520.0),
                child: _CommandTrigger(onTap: onOpenCommandPalette),
              ),
              Expanded(
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    if (c.hasUpdateBanner && roomy) ...[
                      Flexible(child: _UpdateChip(controller: c)),
                      const SizedBox(width: 6),
                    ],
                    Flexible(
                      child: ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 240),
                        child: AccountSwitcher(controller: c, showName: roomy),
                      ),
                    ),
                    const SizedBox(width: 2),
                    _settingsButton(context),
                  ],
                ),
              ),
            ],
          );
        },
      ),
    );
  }

  /// Windows 11 title bar: a small app icon and name on the left, the
  /// search box centred, the account picture and Settings on the right.
  Widget _fluent(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Container(
      height: 48,
      padding: const EdgeInsets.only(left: 16, right: 8),
      color: p.ground,
      child: LayoutBuilder(
        builder: (context, constraints) => Row(
          children: [
            Expanded(
              child: Align(
                alignment: Alignment.centerLeft,
                child: _goHome(
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      const ColectoMark(size: 18),
                      const SizedBox(width: 12),
                      Text('Colecto', style: kStyle(12, color: p.ink2)),
                    ],
                  ),
                ),
              ),
            ),
            SizedBox(
              width: (constraints.maxWidth * 0.4).clamp(180.0, 480.0),
              child: _CommandTrigger(onTap: onOpenCommandPalette),
            ),
            Expanded(
              child: Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  AccountSwitcher(controller: controller, showName: false),
                  _settingsButton(context),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// macOS unified toolbar: room for the traffic lights over the sidebar,
  /// the mailbox title and count over the list, actions and search on the
  /// right.
  Widget _macos(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = controller;
    final unread =
        c.folders.where((f) => f.id == c.activeFolder).firstOrNull?.unread ?? 0;
    final count = c.threads.length;
    final subtitle = [
      count == 1 ? '1 message' : '$count messages',
      if (unread > 0) '$unread unread',
    ].join(', ');
    return Container(
      height: 52,
      padding: const EdgeInsets.only(right: 10),
      decoration: BoxDecoration(
        color: p.float,
        border: Border(bottom: BorderSide(color: p.lineSoft)),
      ),
      child: Row(
        children: [
          SizedBox(width: sidebarWidth + 1),
          const SizedBox(width: 16),
          Expanded(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  c.currentFolderName,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: kStyle(13, weight: 700, color: p.ink),
                ),
                Text(
                  subtitle,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: kStyle(11, color: p.ink3),
                ),
              ],
            ),
          ),
          ColectoIconButton(
            icon: LucideIcons.squarePen,
            tooltip: 'New message (C)',
            size: 17,
            onPressed: onCompose,
          ),
          const SizedBox(width: 4),
          SizedBox(
            width: 220,
            child: _CommandTrigger(onTap: onOpenCommandPalette),
          ),
          const SizedBox(width: 6),
          AccountSwitcher(controller: c, showName: false),
          _settingsButton(context),
        ],
      ),
    );
  }

  /// Material 3 (Gmail on a large screen): the name, a pill search bar and
  /// the account picture.
  Widget _material(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Container(
      height: 64,
      padding: const EdgeInsets.only(left: 20, right: 12),
      color: p.sunk,
      child: Row(
        children: [
          _goHome(
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                const ColectoMark(size: 28),
                const SizedBox(width: 12),
                Text('Colecto', style: kStyle(20, color: p.ink)),
              ],
            ),
          ),
          const SizedBox(width: 48),
          Flexible(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 720),
              child: _CommandTrigger(onTap: onOpenCommandPalette),
            ),
          ),
          const Spacer(),
          _settingsButton(context),
          const SizedBox(width: 4),
          AccountSwitcher(controller: controller, showName: false),
        ],
      ),
    );
  }

  /// iPadOS: a quiet bar with the name and the search field.
  Widget _cupertino(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Container(
      height: 52,
      padding: const EdgeInsets.only(left: 20, right: 8),
      decoration: BoxDecoration(
        color: p.bg,
        border: Border(bottom: BorderSide(color: p.lineSoft, width: 0.5)),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) => Row(
          children: [
            Expanded(
              child: Align(
                alignment: Alignment.centerLeft,
                child: _goHome(
                  Text('Colecto', style: kStyle(17, weight: 600, color: p.ink)),
                ),
              ),
            ),
            SizedBox(
              width: (constraints.maxWidth * 0.4).clamp(200.0, 480.0),
              child: _CommandTrigger(onTap: onOpenCommandPalette),
            ),
            Expanded(
              child: Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  AccountSwitcher(controller: controller, showName: false),
                  _settingsButton(context),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The search / command trigger, drawn as each design's search box.
class _CommandTrigger extends StatelessWidget {
  const _CommandTrigger({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final style = colectoPlatformStyle(context);
    final mod = colectoModKey(context);
    final (height, radius, fill, hint) = switch (style) {
      ColectoPlatformStyle.fluent => (
        32.0,
        4.0,
        p.ink.withValues(alpha: p.isDark ? 0.06 : 0.04),
        'Search',
      ),
      ColectoPlatformStyle.macos => (
        28.0,
        6.0,
        p.ink.withValues(alpha: 0.07),
        'Search',
      ),
      ColectoPlatformStyle.android => (48.0, 28.0, p.raise, 'Search in mail'),
      ColectoPlatformStyle.ios => (36.0, 10.0, p.control, 'Search'),
      ColectoPlatformStyle.desktop => (
        30.0,
        7.0,
        p.sunk,
        'Search mail or run a command',
      ),
    };
    final iconLeft = style != ColectoPlatformStyle.fluent;
    return ColectoPressable(
      onTap: onTap,
      pressScale: 0.99,
      semanticLabel: 'Search and commands',
      tooltip: '$mod+K',
      focusRadius: radius,
      builder: (context, s) {
        final icon = Icon(
          LucideIcons.search,
          size: style == ColectoPlatformStyle.android ? 20 : 15,
          color: s.hovered ? p.ink2 : p.ink3,
        );
        return AnimatedContainer(
          duration: ColectoMotion.hover,
          height: height,
          padding: EdgeInsets.symmetric(
            horizontal: style == ColectoPlatformStyle.android ? 16 : 10,
          ),
          decoration: BoxDecoration(
            color: s.hovered && style != ColectoPlatformStyle.desktop
                ? Color.lerp(fill, p.ink, 0.04)
                : fill,
            borderRadius: BorderRadius.circular(radius),
            border: switch (style) {
              ColectoPlatformStyle.desktop => Border.all(
                color: s.hovered ? p.line : p.lineSoft,
              ),
              // Fluent text boxes carry a stronger bottom edge.
              ColectoPlatformStyle.fluent => Border(
                bottom: BorderSide(color: p.ink3.withValues(alpha: 0.5)),
              ),
              _ => null,
            },
          ),
          child: Row(
            children: [
              if (iconLeft) ...[
                icon,
                SizedBox(width: style == ColectoPlatformStyle.android ? 14 : 8),
              ],
              Expanded(
                child: Text(
                  hint,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: kStyle(
                    style == ColectoPlatformStyle.android ? 16 : 13,
                    color: s.hovered ? p.ink2 : p.ink3,
                  ),
                ),
              ),
              if (style == ColectoPlatformStyle.desktop) ...[
                ColectoKbd(mod, raised: true),
                const SizedBox(width: 3),
                const ColectoKbd('K', raised: true),
              ],
              if (!iconLeft) icon,
            ],
          ),
        );
      },
    );
  }
}

class _UpdateChip extends StatelessWidget {
  const _UpdateChip({required this.controller});

  final MailController controller;

  @override
  Widget build(BuildContext context) {
    final version = controller.releaseStatus!.latest!.version;
    return ColectoChip(
      label: 'Colecto $version is out',
      icon: LucideIcons.sparkles,
      selected: true,
      onTap: () => controller.updatePreferences(
        PreferencesPatch(notifiedRelease: version),
      ),
    );
  }
}

/// The account switcher: avatar, name and live sync status; it opens a
/// listbox with every enabled account.
class AccountSwitcher extends StatelessWidget {
  const AccountSwitcher({
    super.key,
    required this.controller,
    this.showName = true,
  });

  final MailController controller;
  final bool showName;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = controller;
    final current = c.currentAccountInfo;
    final sync = c.currentAccountSync;
    final online = sync?.isOnline ?? (current?.connected ?? false);
    final status = sync == null
        ? (online ? 'Connected' : 'Offline')
        : sync.error != null
        ? 'Sync paused'
        : sync.activity == SyncActivity.syncing
        ? 'Syncing…'
        : sync.mode.label;

    return Builder(
      builder: (ctx) => ColectoPressable(
        onTap: () => showAccountMenu(ctx, c),
        pressScale: 0.98,
        tooltip: showName ? null : current?.displayName,
        semanticLabel: 'Current account: ${current?.displayName ?? 'none'}',
        builder: (context, s) => AnimatedContainer(
          duration: ColectoMotion.hover,
          height: 40,
          padding: EdgeInsets.only(left: 6, right: showName ? 8 : 6),
          decoration: BoxDecoration(
            color: s.hovered ? p.raise : const Color(0x00000000),
            borderRadius: BorderRadius.circular(7),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              ColectoAvatar(
                name: current?.displayName ?? '?',
                hueKey: current?.address,
                size: 28,
              ),
              if (showName) ...[
                const SizedBox(width: 10),
                Flexible(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        current?.displayName ?? 'No account',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: kStyle(
                          13,
                          weight: 600,
                          color: p.ink,
                          height: 1.25,
                        ),
                      ),
                      Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          ColectoStatusDot(
                            on: online,
                            error: sync?.error != null,
                          ),
                          const SizedBox(width: 6),
                          Flexible(
                            child: Text(
                              status,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: kStyle(11.5, color: p.ink3, height: 1.25),
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 8),
                Icon(LucideIcons.chevronsUpDown, size: 14, color: p.ink3),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

void showAccountMenu(BuildContext anchorContext, MailController c) {
  final items = [
    for (final a in c.enabledAccounts)
      ColectoMenuItem(
        label: a.displayName,
        subtitle: a.address,
        checked: a.id == c.activeAccount,
        onSelected: () => c.selectAccount(a.id),
      ),
    const ColectoMenuItem.divider(),
    ColectoMenuItem(
      label: 'Add or manage accounts…',
      icon: LucideIcons.userRoundPlus,
      onSelected: () => c.openSettings(SettingsSection.accounts),
    ),
  ];
  if (colectoTouch(anchorContext)) {
    showColectoContextMenu(
      context: anchorContext,
      globalPosition: colectoRectOf(anchorContext).center,
      title: 'Accounts',
      items: items,
    );
  } else {
    showColectoMenu(
      context: anchorContext,
      anchor: colectoRectOf(anchorContext),
      items: items,
      width: 260,
      alignRight: true,
    );
  }
}
