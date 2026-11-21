import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../core/theme/colecto_palette.dart';
import '../../core/kit/kit.dart';
import '../shell/mail_controller.dart';
import 'accounts_settings_tab.dart';
import 'ai_settings_tab.dart';
import 'automation_tabs.dart';
import 'mail_appearance_tabs.dart';

/// Colecto's Settings card: a section list on the left, a breadcrumb and a
/// close button over the page. It floats over the workspace (which stays
/// mounted behind it); on phones it fills the screen and the sections
/// scroll sideways above the page.
class SettingsView extends StatefulWidget {
  const SettingsView({super.key, required this.controller, this.phone = false});

  final MailController controller;
  final bool phone;

  @override
  State<SettingsView> createState() => _SettingsViewState();
}

class _SettingsViewState extends State<SettingsView> {
  final _scroll = ScrollController();
  SettingsSection? _shown;

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  Widget _page(SettingsSection section) {
    final c = widget.controller;
    return switch (section) {
      SettingsSection.accounts => AccountsSettingsTab(controller: c),
      SettingsSection.mail => MailSettingsTab(controller: c),
      SettingsSection.appearance => AppearanceSettingsTab(controller: c),
      SettingsSection.notifications => NotificationsSettingsTab(controller: c),
      SettingsSection.privacy => PrivacySettingsTab(controller: c),
      SettingsSection.servers => ServersSettingsTab(controller: c),
      SettingsSection.ai => AiSettingsTab(controller: c),
      SettingsSection.apiKeys => ApiKeysSettingsTab(controller: c),
      SettingsSection.sendApprovals => SendApprovalsSettingsTab(controller: c),
    };
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = widget.controller;
    final section = c.activeSettings ?? SettingsSection.accounts;
    if (_shown != section) {
      _shown = section;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && _scroll.hasClients) _scroll.jumpTo(0);
      });
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final wide = !widget.phone && constraints.maxWidth >= 720;
        final narrowPage = constraints.maxWidth < 600;
        final page = Scrollbar(
          controller: _scroll,
          child: SingleChildScrollView(
            controller: _scroll,
            padding: EdgeInsets.fromLTRB(
              narrowPage ? 16 : 48,
              narrowPage ? 20 : 32,
              narrowPage ? 16 : 48,
              48 + MediaQuery.paddingOf(context).bottom,
            ),
            child: Align(
              alignment: Alignment.topLeft,
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 720),
                child: KeyedSubtree(
                  key: ValueKey(section),
                  child: _page(section),
                ),
              ),
            ),
          ),
        );

        final head = Container(
          height: widget.phone ? 54 : 52,
          padding: EdgeInsets.only(left: wide ? 24 : 16, right: 10),
          decoration: BoxDecoration(
            border: Border(bottom: BorderSide(color: p.lineSoft)),
          ),
          child: Row(
            children: [
              Expanded(
                child: Text.rich(
                  TextSpan(
                    children: [
                      TextSpan(
                        text: 'Settings',
                        style: kStyle(13.5, color: p.ink3),
                      ),
                      TextSpan(
                        text: '  ›  ',
                        style: kStyle(13.5, color: p.ink3),
                      ),
                      TextSpan(
                        text: section.label,
                        style: kStyle(13.5, weight: 600, color: p.ink),
                      ),
                    ],
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              ColectoIconButton(
                icon: LucideIcons.x,
                tooltip: 'Close settings (Esc)',
                size: 18,
                onPressed: c.closeSettings,
              ),
            ],
          ),
        );

        if (!wide) {
          return ColoredBox(
            color: p.bg,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                head,
                Container(
                  height: 52,
                  decoration: BoxDecoration(
                    border: Border(bottom: BorderSide(color: p.lineSoft)),
                  ),
                  child: ListView(
                    scrollDirection: Axis.horizontal,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 8,
                      vertical: 6,
                    ),
                    children: [
                      for (final s in c.visibleSettingsSections)
                        _NavItem(
                          section: s,
                          selected: s == section,
                          badge: _badge(s),
                          dense: true,
                          onTap: () => c.openSettings(s),
                        ),
                    ],
                  ),
                ),
                Expanded(child: page),
              ],
            ),
          );
        }

        if (colectoPlatformStyle(context) == ColectoPlatformStyle.macos) {
          // A macOS Settings window: icon tabs across a toolbar.
          return ColoredBox(
            color: p.bg,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Container(
                  decoration: BoxDecoration(
                    color: p.float,
                    border: Border(bottom: BorderSide(color: p.lineSoft)),
                  ),
                  padding: const EdgeInsets.fromLTRB(12, 8, 8, 8),
                  child: Row(
                    children: [
                      Expanded(
                        child: SingleChildScrollView(
                          scrollDirection: Axis.horizontal,
                          child: Row(
                            children: [
                              for (final s in c.visibleSettingsSections)
                                _MacTab(
                                  section: s,
                                  selected: s == section,
                                  badge: _badge(s),
                                  onTap: () => c.openSettings(s),
                                ),
                            ],
                          ),
                        ),
                      ),
                      ColectoIconButton(
                        icon: LucideIcons.x,
                        tooltip: 'Close settings (Esc)',
                        onPressed: c.closeSettings,
                      ),
                    ],
                  ),
                ),
                Expanded(child: page),
              ],
            ),
          );
        }

        return Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Container(
              width: 228,
              decoration: BoxDecoration(
                color: p.sunk,
                border: Border(right: BorderSide(color: p.lineSoft)),
              ),
              child: ListView(
                padding: const EdgeInsets.fromLTRB(10, 12, 10, 12),
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(10, 10, 10, 12),
                    child: Text(
                      'Settings',
                      style: kStyle(
                        18,
                        weight: 640,
                        width: 108,
                        color: p.ink,
                        letterSpacing: -0.27,
                      ),
                    ),
                  ),
                  for (final s in c.visibleSettingsSections)
                    _NavItem(
                      section: s,
                      selected: s == section,
                      badge: _badge(s),
                      onTap: () => c.openSettings(s),
                    ),
                ],
              ),
            ),
            Expanded(
              child: ColoredBox(
                color: p.bg,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    head,
                    Expanded(child: page),
                  ],
                ),
              ),
            ),
          ],
        );
      },
    );
  }

  int _badge(SettingsSection s) => s == SettingsSection.sendApprovals
      ? widget.controller.pendingApprovalCount
      : 0;
}

class _NavItem extends StatelessWidget {
  const _NavItem({
    required this.section,
    required this.selected,
    required this.onTap,
    this.badge = 0,
    this.dense = false,
  });

  final SettingsSection section;
  final bool selected;
  final VoidCallback onTap;
  final int badge;
  final bool dense;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final style = colectoPlatformStyle(context);
    final material = style == ColectoPlatformStyle.android;
    final ios = style == ColectoPlatformStyle.ios;
    final fluent = style == ColectoPlatformStyle.fluent;
    final radius = switch (style) {
      ColectoPlatformStyle.android => 28.0,
      ColectoPlatformStyle.ios => 10.0,
      ColectoPlatformStyle.fluent => 4.0,
      _ => 8.0,
    };
    return Padding(
      padding: dense
          ? const EdgeInsets.only(right: 4)
          : const EdgeInsets.only(bottom: 2),
      child: ColectoPressable(
        onTap: onTap,
        selected: selected,
        semanticLabel: section.label,
        focusRadius: radius,
        builder: (context, s) => AnimatedContainer(
          duration: const Duration(milliseconds: 120),
          height: dense ? 40 : (material || ios ? 44 : (fluent ? 36 : 34)),
          padding: EdgeInsets.symmetric(horizontal: material ? 16 : 10),
          decoration: BoxDecoration(
            color: selected
                ? (material
                      ? p.select
                      : ios
                      ? p.accent
                      : fluent
                      ? p.ink.withValues(alpha: 0.08)
                      : p.raise)
                : s.hovered
                ? (fluent || ios || material
                      ? p.ink.withValues(alpha: 0.05)
                      : p.raise)
                : const Color(0x00000000),
            borderRadius: BorderRadius.circular(radius),
            border: fluent && selected && !dense
                ? Border(left: BorderSide(color: p.accent, width: 3))
                : null,
          ),
          child: Row(
            mainAxisSize: dense ? MainAxisSize.min : MainAxisSize.max,
            children: [
              Icon(
                section.icon,
                size: 16,
                color: ios && selected
                    ? p.accentInk
                    : ios || selected
                    ? (material ? p.accentHi : p.accent)
                    : (s.hovered ? p.ink2 : p.ink3),
              ),
              const SizedBox(width: 10),
              if (dense)
                Text(
                  section.label,
                  style: kStyle(
                    13.5,
                    weight: 560,
                    color: ios && selected
                        ? p.accentInk
                        : material && selected
                        ? p.accentHi
                        : selected || s.hovered
                        ? p.ink
                        : p.ink2,
                  ),
                )
              else
                Expanded(
                  child: Text(
                    section.label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: kStyle(
                      ios ? 15 : (material ? 14 : 13.5),
                      weight: ios ? 400 : 560,
                      color: ios && selected
                          ? p.accentInk
                          : material && selected
                          ? p.accentHi
                          : selected || s.hovered
                          ? p.ink
                          : p.ink2,
                    ),
                  ),
                ),
              if (badge > 0) ...[
                const SizedBox(width: 8),
                ColectoBadge(text: '$badge', accent: true),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

/// A macOS Settings toolbar tab: an icon over its name.
class _MacTab extends StatelessWidget {
  const _MacTab({
    required this.section,
    required this.selected,
    required this.onTap,
    this.badge = 0,
  });

  final SettingsSection section;
  final bool selected;
  final VoidCallback onTap;
  final int badge;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return ColectoPressable(
      onTap: onTap,
      selected: selected,
      semanticLabel: section.label,
      pressScale: 1,
      focusRadius: 6,
      builder: (context, s) => Container(
        constraints: const BoxConstraints(minWidth: 72),
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
        margin: const EdgeInsets.only(right: 2),
        decoration: BoxDecoration(
          color: selected
              ? p.ink.withValues(alpha: 0.1)
              : s.hovered
              ? p.ink.withValues(alpha: 0.05)
              : const Color(0x00000000),
          borderRadius: BorderRadius.circular(6),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Badge(
              isLabelVisible: badge > 0,
              backgroundColor: p.accent,
              smallSize: 7,
              child: Icon(
                section.icon,
                size: 20,
                color: selected ? p.accent : p.ink2,
              ),
            ),
            const SizedBox(height: 3),
            Text(
              section.label,
              maxLines: 1,
              style: kStyle(11, color: selected ? p.ink : p.ink2),
            ),
          ],
        ),
      ),
    );
  }
}
