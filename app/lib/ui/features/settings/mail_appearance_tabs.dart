import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/preferences_models.dart';
import '../../core/kit/kit.dart';
import '../../core/theme/colecto_palette.dart';
import '../../core/widgets/corner_picker.dart';
import '../shell/mail_controller.dart';

class MailSettingsTab extends StatelessWidget {
  const MailSettingsTab({super.key, required this.controller});

  final MailController controller;

  @override
  Widget build(BuildContext context) {
    final c = controller;
    final prefs = c.preferences;
    const prefetchSteps = [25, 50, 100];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const ColectoPageHead(
          title: 'Mail',
          description: 'How the message list looks and what Colecto keeps ready offline.',
        ),
        ColectoSection(
          title: 'List density',
          description:
              'Full shows a preview line; Compact fits one message per line.',
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoSegmented<ListDensity>(
                value: prefs.listDensity,
                options: [for (final d in ListDensity.values) (d, d.label)],
                onChanged: (d) =>
                    c.updatePreferences(PreferencesPatch(listDensity: d)),
              ),
            ),
          ],
        ),
        ColectoSection(
          children: [
            ColectoSettingRow(
              title: 'Group by date',
              description:
                  'Today, Yesterday, Previous 7 days, then one heading per '
                  'month.',
              trailing: ColectoSwitch(
                value: prefs.dateGroups,
                onChanged: (v) =>
                    c.updatePreferences(PreferencesPatch(dateGroups: v)),
              ),
            ),
            const SizedBox(height: 16),
            ColectoSettingRow(
              title: 'Load older messages on scroll',
              description:
                  'Fetch the next batch automatically when you reach the '
                  'bottom of the list. When off, a "Load more" link appears '
                  'instead.',
              trailing: ColectoSwitch(
                value: prefs.autoLoadMore,
                onChanged: (v) =>
                    c.updatePreferences(PreferencesPatch(autoLoadMore: v)),
              ),
            ),
          ],
        ),
        ColectoSection(
          children: [
            ColectoSettingRow(
              title: 'Download messages for offline reading',
              description:
                  'After each Inbox check, fetch the newest message bodies so '
                  'they open instantly and work without a connection.',
              trailing: ColectoSwitch(
                value: prefs.prefetch.enabled,
                onChanged: (v) => c.updatePreferences(
                  PreferencesPatch(
                    prefetch: prefs.prefetch.copyWith(enabled: v),
                  ),
                ),
              ),
            ),
            if (prefs.prefetch.enabled) ...[
              const SizedBox(height: 14),
              const ColectoLabel('How many messages'),
              Align(
                alignment: Alignment.centerLeft,
                child: ColectoSegmented<int>(
                  value: prefetchSteps.contains(prefs.prefetch.count)
                      ? prefs.prefetch.count
                      : 50,
                  options: [for (final n in prefetchSteps) (n, '$n')],
                  onChanged: (n) => c.updatePreferences(
                    PreferencesPatch(
                      prefetch: prefs.prefetch.copyWith(count: n),
                    ),
                  ),
                ),
              ),
            ],
          ],
        ),
      ],
    );
  }
}

class AppearanceSettingsTab extends StatefulWidget {
  const AppearanceSettingsTab({super.key, required this.controller});

  final MailController controller;

  @override
  State<AppearanceSettingsTab> createState() => _AppearanceSettingsTabState();
}

class _AppearanceSettingsTabState extends State<AppearanceSettingsTab> {
  final _hexCtrl = TextEditingController();

  // Light, low-chroma tones that read both as text and as a fill.
  static const _presets = <(String, String)>[
    ('#a2d6b1', 'Mint'),
    ('#9cc4f0', 'Sky'),
    ('#c4b0f5', 'Lilac'),
    ('#f2a9b8', 'Rose'),
    ('#f0c58e', 'Amber'),
    ('#8fd9cf', 'Teal'),
  ];

  static const _fonts = <(String, String)>[
    ('', 'Platform default'),
    ('mona', 'Mona Sans'),
    ('inter', 'Inter'),
    ('geist', 'Geist'),
    ('plex', 'IBM Plex Sans'),
    ('atkinson', 'Atkinson Hyperlegible Next'),
  ];

  @override
  void initState() {
    super.initState();
    _hexCtrl.text = widget.controller.preferences.accent ?? '';
  }

  @override
  void dispose() {
    _hexCtrl.dispose();
    super.dispose();
  }

  void _setAccent(String hex) {
    _hexCtrl.text = hex;
    widget.controller.updatePreferences(PreferencesPatch(accent: hex));
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = widget.controller;
    final prefs = c.preferences;
    final spacing = prefs.textSpacing;
    final accent = prefs.accent ?? '';
    final materialYou = c.design == ColectoPlatformStyle.android;
    final systemColors = materialYou && accent.isEmpty;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const ColectoPageHead(
          title: 'Appearance',
          description: 'Theme, colour and type. Changes apply right away.',
        ),
        ColectoSection(
          title: 'Theme',
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoSegmented<ThemeMode>(
                value: c.themeMode,
                options: const [
                  (ThemeMode.system, 'System'),
                  (ThemeMode.dark, 'Dark'),
                  (ThemeMode.light, 'Light'),
                ],
                onChanged: c.setThemeMode,
              ),
            ),
          ],
        ),
        ColectoSection(
          title: colectoTouch(context) ? 'Interface size' : 'Zoom',
          description: colectoTouch(context)
              ? 'Make everything on screen bigger or smaller.'
              : 'Make everything bigger or smaller. ${colectoModKey(context)} +, '
                    '${colectoModKey(context)} − and ${colectoModKey(context)} 0 '
                    'work anywhere.',
          children: [
            Row(
              children: [
                ColectoIconButton(
                  icon: LucideIcons.minus,
                  tooltip: 'Zoom out',
                  onPressed: c.zoom <= 0.5 ? null : c.zoomOut,
                ),
                SizedBox(
                  width: 64,
                  child: ColectoText(
                    '${(c.zoom * 100).round()}%',
                    role: ColectoTextRole.bodyStrong,
                    align: TextAlign.center,
                  ),
                ),
                ColectoIconButton(
                  icon: LucideIcons.plus,
                  tooltip: 'Zoom in',
                  onPressed: c.zoom >= 2 ? null : c.zoomIn,
                ),
                const SizedBox(width: 8),
                ColectoButton(
                  label: 'Reset',
                  variant: ColectoButtonVariant.ghost,
                  onPressed: c.zoom == 1 ? null : c.resetZoom,
                ),
              ],
            ),
          ],
        ),
        ColectoSection(
          title: 'Design',
          description: c.designForcedByLaunch
              ? 'Set by COLECTO_DESIGN for this launch. Your choice here is '
                    'saved and used next time.'
              : 'Each platform gets its own controls by default, but any '
                    'design works anywhere: pick the one you like.',
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoSelect<ColectoPlatformStyle>(
                value: c.design,
                minWidth: 260,
                options: [
                  for (final d in ColectoPlatformStyle.values)
                    (
                      d,
                      d == ColectoPlatformStyle.native()
                          ? '${d.label} · native'
                          : d.label,
                    ),
                ],
                onChanged: c.setDesign,
              ),
            ),
          ],
        ),
        ColectoSection(
          title: 'Accent colour',
          description: systemColors
              ? 'Following your wallpaper colours (Material You).'
              : 'Used for selection, focus and the main action.',
          children: [
            if (materialYou) ...[
              ColectoSettingRow(
                title: 'Use system colours',
                description:
                    'Match Android’s Material You palette. Turn off to pick '
                    'your own accent.',
                trailing: ColectoSwitch(
                  value: systemColors,
                  onChanged: (on) =>
                      _setAccent(on ? '' : ColectoPalette.toHex(p.accent)),
                ),
              ),
              const SizedBox(height: 14),
            ],
            if (!systemColors) ...[
              Wrap(
                spacing: 10,
                runSpacing: 10,
                children: [
                  for (final (hex, name) in _presets)
                    _Swatch(
                      color: ColectoPalette.parseHexColor(hex)!,
                      name: name,
                      selected:
                          accent.toLowerCase() == hex ||
                          (accent.isEmpty && hex == '#a2d6b1'),
                      onTap: () => _setAccent(hex),
                    ),
                ],
              ),
              const SizedBox(height: 16),
              Row(
                children: [
                  SizedBox(
                    width: 170,
                    child: ColectoTextField(
                      controller: _hexCtrl,
                      hintText: '#a2d6b1',
                      onSubmitted: (v) {
                        final hex = v.startsWith('#') ? v : '#$v';
                        if (ColectoPalette.parseHexColor(hex) != null) {
                          _setAccent(hex.toLowerCase());
                        } else {
                          c.showToast(
                            'That isn’t a colour',
                            detail: 'Use six hex digits, like #a2d6b1.',
                            isError: true,
                          );
                        }
                      },
                    ),
                  ),
                  const SizedBox(width: 8),
                  ColectoButton(
                    label: 'Reset',
                    variant: ColectoButtonVariant.ghost,
                    onPressed: accent.isEmpty ? null : () => _setAccent(''),
                  ),
                ],
              ),
            ],
          ],
        ),
        ColectoSection(
          title: 'Typeface',
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoSelect<String>(
                value: prefs.font ?? '',
                minWidth: 260,
                options: _fonts,
                onChanged: (f) =>
                    c.updatePreferences(PreferencesPatch(font: f)),
              ),
            ),
            const SizedBox(height: 16),
            const ColectoLabel('Text size'),
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoSegmented<int>(
                value: (prefs.fontSize ?? 14).clamp(13, 16),
                options: const [(13, '13'), (14, '14'), (15, '15'), (16, '16')],
                onChanged: (v) =>
                    c.updatePreferences(PreferencesPatch(fontSize: v)),
              ),
            ),
          ],
        ),
        ColectoSection(
          title: 'Text spacing',
          description:
              'Extra room between letters, words, lines and paragraphs '
              '(WCAG 1.4.12).',
          children: [
            _spacing(
              'Letters',
              spacing.letter,
              (v) => spacing.copyWith(letter: v),
            ),
            _spacing('Words', spacing.word, (v) => spacing.copyWith(word: v)),
            _spacing('Lines', spacing.line, (v) => spacing.copyWith(line: v)),
            _spacing(
              'Paragraphs',
              spacing.paragraph,
              (v) => spacing.copyWith(paragraph: v),
            ),
          ],
        ),
      ],
    );
  }

  Widget _spacing(String label, int value, TextSpacing Function(int) next) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Wrap(
        alignment: WrapAlignment.spaceBetween,
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: 16,
        runSpacing: 6,
        children: [
          SizedBox(width: 120, child: ColectoText(label)),
          ColectoSegmented<int>(
            value: value,
            options: const [(0, 'Normal'), (1, 'More'), (2, 'Most')],
            onChanged: (v) => widget.controller.updatePreferences(
              PreferencesPatch(textSpacing: next(v)),
            ),
          ),
        ],
      ),
    );
  }
}

class _Swatch extends StatelessWidget {
  const _Swatch({
    required this.color,
    required this.name,
    required this.selected,
    required this.onTap,
  });

  final Color color;
  final String name;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return ColectoPressable(
      onTap: onTap,
      tooltip: name,
      semanticLabel: name,
      selected: selected,
      pressScale: 0.92,
      focusRadius: 999,
      builder: (context, s) => AnimatedContainer(
        duration: ColectoMotion.hover,
        width: 34,
        height: 34,
        decoration: BoxDecoration(
          color: color,
          shape: BoxShape.circle,
          boxShadow: selected
              ? [
                  BoxShadow(color: p.bg, spreadRadius: 2),
                  BoxShadow(color: color, spreadRadius: 4),
                ]
              : null,
        ),
        child: AnimatedOpacity(
          opacity: selected ? 1 : 0,
          duration: ColectoMotion.hover,
          child: Icon(
            LucideIcons.check,
            size: 16,
            color: Color.lerp(const Color(0xFF000000), color, 0.14),
          ),
        ),
      ),
    );
  }
}

class NotificationsSettingsTab extends StatelessWidget {
  const NotificationsSettingsTab({super.key, required this.controller});

  final MailController controller;

  @override
  Widget build(BuildContext context) {
    final c = controller;
    final n = c.preferences.notifications;
    void set(NotificationPreferences v) =>
        c.updatePreferences(PreferencesPatch(notifications: v));
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const ColectoPageHead(
          title: 'Notifications',
          description: 'New mail and send approvals, inside Colecto and on your desktop.',
        ),
        ColectoSection(
          children: [
            ColectoSettingRow(
              title: 'Show notifications in Colecto',
              description:
                  'They leave after six seconds unless you point at them. '
                  'Errors always show.',
              trailing: ColectoSwitch(
                value: n.enabled,
                onChanged: (v) => set(n.copyWith(enabled: v)),
              ),
            ),
            const SizedBox(height: 10),
            ColectoSettingRow(
              title: 'Desktop notifications',
              description: 'Only while Colecto is in the background.',
              trailing: ColectoSwitch(
                value: n.system,
                onChanged: (v) => set(n.copyWith(system: v)),
              ),
            ),
          ],
        ),
        ColectoSection(
          title: 'Position',
          description:
              'Where notifications appear in the window: '
              '${n.position.label.toLowerCase()}.',
          children: [
            Wrap(
              crossAxisAlignment: WrapCrossAlignment.end,
              spacing: 14,
              runSpacing: 12,
              children: [
                CornerPicker(
                  value: n.position,
                  onChanged: (corner) {
                    set(n.copyWith(position: corner));
                    c.showToast('Notifications will appear here');
                  },
                ),
                ColectoButton(
                  label: 'Send a test',
                  icon: LucideIcons.bell,
                  onPressed: () => c.showToast(
                    'This is a notification',
                    detail: 'Point at it to keep it on screen.',
                  ),
                ),
              ],
            ),
          ],
        ),
      ],
    );
  }
}

class PrivacySettingsTab extends StatelessWidget {
  const PrivacySettingsTab({super.key, required this.controller});

  final MailController controller;

  @override
  Widget build(BuildContext context) {
    final prefs = controller.preferences;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const ColectoPageHead(
          title: 'Privacy',
          description:
              'Colecto sanitizes every HTML message before showing it.',
        ),
        ColectoSection(
          children: [
            ColectoSettingRow(
              title: 'Load remote images automatically',
              description:
                  'Off by default. Senders can use remote images to learn '
                  'when and where you opened a message. You can still load '
                  'them per message.',
              trailing: ColectoSwitch(
                value: prefs.remoteImages,
                onChanged: (v) => controller.updatePreferences(
                  PreferencesPatch(remoteImages: v),
                ),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class ServersSettingsTab extends StatefulWidget {
  const ServersSettingsTab({super.key, required this.controller});

  final MailController controller;

  @override
  State<ServersSettingsTab> createState() => _ServersSettingsTabState();
}

class _ServersSettingsTabState extends State<ServersSettingsTab> {
  late final _url = TextEditingController(text: widget.controller.daemonUrl);
  late final _token = TextEditingController(
    text: widget.controller.daemonToken,
  );

  @override
  void dispose() {
    _url.dispose();
    _token.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = widget.controller;
    final ver = c.version;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ColectoPageHead(
          title: 'Servers',
          description:
              'Where your mail is handled. Now: ${c.backend.backendLabel}.',
        ),
        ColectoSection(
          title: 'Backend',
          description: ver == null
              ? null
              : '${ver.name} ${ver.version} · API ${ver.api}'
                    '${ver.commit != null ? ' · ${ver.commit}' : ''}',
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoSegmented<BackendKind>(
                value: c.backendKind,
                options: const [
                  (BackendKind.demo, 'Demo'),
                  (BackendKind.remoteHttp, 'Daemon'),
                  (BackendKind.localFfi, 'On this device'),
                ],
                onChanged: (k) => k == BackendKind.remoteHttp
                    ? c.authenticateDaemon(_url.text, _token.text)
                    : c.switchBackend(kind: k),
              ),
            ),
          ],
        ),
        ColectoSection(
          title: 'colecto-daemon',
          description: 'Connect to a daemon on this machine or one you host.',
          children: [
            ColectoTextField(
              controller: _url,
              labelText: 'Address',
              hintText: 'http://127.0.0.1:4173',
              keyboardType: TextInputType.url,
            ),
            const SizedBox(height: 14),
            ColectoTextField(
              controller: _token,
              labelText: 'Access token',
              obscureText: true,
              onSubmitted: (_) => c.authenticateDaemon(_url.text, _token.text),
            ),
            const SizedBox(height: 16),
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoButton(
                label: 'Connect',
                variant: ColectoButtonVariant.primary,
                onPressed: () => c.authenticateDaemon(_url.text, _token.text),
              ),
            ),
          ],
        ),
      ],
    );
  }
}
