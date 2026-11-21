import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/ai_models.dart';
import '../../core/theme/colecto_palette.dart';
import '../../core/kit/kit.dart';
import '../shell/mail_controller.dart';

class AiSettingsTab extends StatefulWidget {
  const AiSettingsTab({super.key, required this.controller});

  final MailController controller;

  @override
  State<AiSettingsTab> createState() => _AiSettingsTabState();
}

class _AiSettingsTabState extends State<AiSettingsTab> {
  late final TextEditingController _textUrlCtrl;
  late final TextEditingController _textModelCtrl;
  late final TextEditingController _textMaxTokensCtrl;
  late final TextEditingController _textKeyCtrl;
  late final TextEditingController _decUrlCtrl;
  late final TextEditingController _decModelCtrl;
  late final TextEditingController _decKeyCtrl;
  late final TextEditingController _timeoutCtrl;

  // Auto-label rule editor state
  String? _selectedLabel;
  final _rulePatternCtrl = TextEditingController();
  final _ruleDescCtrl = TextEditingController();
  RuleField _ruleField = RuleField.any;
  PreviewResult? _previewResult;

  AiSettings? _seeded;

  @override
  void initState() {
    super.initState();
    final s = widget.controller.aiSettings.settings;
    _seeded = s;
    _textUrlCtrl = TextEditingController(text: s.text.baseUrl);
    _textModelCtrl = TextEditingController(text: s.text.model);
    _textMaxTokensCtrl = TextEditingController(text: '${s.text.maxTokens}');
    _textKeyCtrl = TextEditingController();
    _decUrlCtrl = TextEditingController(text: s.decision.baseUrl);
    _decModelCtrl = TextEditingController(text: s.decision.model);
    _decKeyCtrl = TextEditingController();
    _timeoutCtrl = TextEditingController(text: '${s.timeoutSeconds}');
    widget.controller.addListener(_syncFromController);
  }

  void _syncFromController() {
    final s = widget.controller.aiSettings.settings;
    final prev = _seeded;
    if (identical(s, prev)) return;
    _seeded = s;
    void sync(TextEditingController ctrl, String oldVal, String newVal) {
      if (ctrl.text == oldVal && ctrl.text != newVal) {
        ctrl.text = newVal;
      }
    }

    sync(_textUrlCtrl, prev?.text.baseUrl ?? '', s.text.baseUrl);
    sync(_textModelCtrl, prev?.text.model ?? '', s.text.model);
    sync(
      _textMaxTokensCtrl,
      '${prev?.text.maxTokens ?? 1024}',
      '${s.text.maxTokens}',
    );
    sync(_decUrlCtrl, prev?.decision.baseUrl ?? '', s.decision.baseUrl);
    sync(_decModelCtrl, prev?.decision.model ?? '', s.decision.model);
    sync(_timeoutCtrl, '${prev?.timeoutSeconds ?? 120}', '${s.timeoutSeconds}');
  }

  @override
  void dispose() {
    widget.controller.removeListener(_syncFromController);
    _textUrlCtrl.dispose();
    _textModelCtrl.dispose();
    _textMaxTokensCtrl.dispose();
    _textKeyCtrl.dispose();
    _decUrlCtrl.dispose();
    _decModelCtrl.dispose();
    _decKeyCtrl.dispose();
    _timeoutCtrl.dispose();
    _rulePatternCtrl.dispose();
    _ruleDescCtrl.dispose();
    super.dispose();
  }

  Future<void> _saveModelSettings() async {
    final cur = widget.controller.aiSettings.settings;
    await widget.controller.saveAiSettings(
      cur.copyWith(
        text: cur.text.copyWith(
          baseUrl: _textUrlCtrl.text.trim(),
          model: _textModelCtrl.text.trim(),
          maxTokens: int.tryParse(_textMaxTokensCtrl.text.trim()) ?? 1024,
        ),
        decision: cur.decision.copyWith(
          baseUrl: _decUrlCtrl.text.trim(),
          model: _decModelCtrl.text.trim(),
        ),
        timeoutSeconds: int.tryParse(_timeoutCtrl.text.trim()) ?? 120,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = widget.controller;
    final view = c.aiSettings;
    final s = view.settings;
    final accountLabels = c.labels.keys.toList();
    final activeLabel =
        _selectedLabel != null && accountLabels.contains(_selectedLabel)
        ? _selectedLabel
        : (accountLabels.isNotEmpty ? accountLabels.first : null);
    final currentRule = activeLabel != null
        ? (s.labels[c.activeAccount]?[activeLabel] ?? const LabelRule())
        : null;

    Widget keyRow(String kind, TextEditingController ctrl, bool saved) => Row(
      crossAxisAlignment: CrossAxisAlignment.end,
      children: [
        Expanded(
          child: ColectoTextField(
            controller: ctrl,
            labelText: 'API key',
            hintText: saved ? 'Saved. Enter a new key to replace it.' : null,
            obscureText: true,
          ),
        ),
        const SizedBox(width: 8),
        ColectoButton(
          label: 'Save key',
          onPressed: () async {
            await c.saveAiKey(kind, ctrl.text);
            ctrl.clear();
          },
        ),
        if (saved) ...[
          const SizedBox(width: 6),
          ColectoButton(
            label: 'Remove',
            variant: ColectoButtonVariant.ghost,
            onPressed: () => c.saveAiKey(kind, null),
          ),
        ],
      ],
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const ColectoPageHead(
          title: 'AI',
          description:
              'Summaries, reply drafts and automatic labels. Requests only go '
              'to the endpoints you set here.',
        ),
        ColectoSection(
          children: [
            ColectoSettingRow(
              title: 'AI features',
              description: 'Summarize and Draft a reply in the reader.',
              trailing: ColectoSwitch(
                value: s.enabled,
                onChanged: (v) => c.saveAiSettings(s.copyWith(enabled: v)),
              ),
            ),
            const SizedBox(height: 10),
            ColectoSettingRow(
              title: 'Label new mail automatically',
              description: 'Uses the rules below as messages arrive.',
              trailing: ColectoSwitch(
                value: s.autolabel,
                onChanged: (v) => c.saveAiSettings(s.copyWith(autolabel: v)),
              ),
            ),
          ],
        ),
        ColectoSection(
          title: 'Writing model',
          description:
              'Any OpenAI-compatible endpoint, for summaries and replies.',
          trailing: ColectoBadge(
            text: view.keys.text ? 'Key saved' : 'No key',
            accent: view.keys.text,
          ),
          children: [
            ColectoTextField(
              controller: _textUrlCtrl,
              labelText: 'Base URL',
              hintText: 'https://api.openai.com/v1',
              keyboardType: TextInputType.url,
            ),
            const SizedBox(height: 14),
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  flex: 2,
                  child: ColectoTextField(
                    controller: _textModelCtrl,
                    labelText: 'Model',
                    hintText: 'gpt-4o-mini',
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: ColectoTextField(
                    controller: _textMaxTokensCtrl,
                    labelText: 'Max tokens',
                    keyboardType: TextInputType.number,
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: ColectoTextField(
                    controller: _timeoutCtrl,
                    labelText: 'Timeout (s)',
                    keyboardType: TextInputType.number,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 14),
            keyRow('text', _textKeyCtrl, view.keys.text),
            const SizedBox(height: 16),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                ColectoButton(
                  label: 'Save',
                  variant: ColectoButtonVariant.primary,
                  onPressed: _saveModelSettings,
                ),
                ColectoButton(
                  label: 'Test connection',
                  icon: LucideIcons.plugZap,
                  onPressed: () => c.testAiConnection('text'),
                ),
              ],
            ),
          ],
        ),
        ColectoSection(
          title: 'Labelling model',
          description: 'A small, fast model that decides which label fits.',
          trailing: ColectoBadge(
            text: view.keys.decision ? 'Key saved' : 'No key',
            accent: view.keys.decision,
          ),
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  flex: 2,
                  child: ColectoTextField(
                    controller: _decUrlCtrl,
                    labelText: 'Base URL',
                    keyboardType: TextInputType.url,
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: ColectoTextField(
                    controller: _decModelCtrl,
                    labelText: 'Model',
                  ),
                ),
              ],
            ),
            const SizedBox(height: 14),
            const ColectoLabel(
              'Only label when the model is at least this sure',
            ),
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoSegmented<double>(
                value: [0.6, 0.75, 0.9].reduce(
                  (a, b) =>
                      (a - s.decision.threshold).abs() <
                          (b - s.decision.threshold).abs()
                      ? a
                      : b,
                ),
                options: const [(0.6, '60%'), (0.75, '75%'), (0.9, '90%')],
                onChanged: (v) => c.saveAiSettings(
                  s.copyWith(decision: s.decision.copyWith(threshold: v)),
                ),
              ),
            ),
            const SizedBox(height: 14),
            keyRow('decision', _decKeyCtrl, view.keys.decision),
            const SizedBox(height: 16),
            Wrap(
              spacing: 8,
              children: [
                ColectoButton(
                  label: 'Save',
                  variant: ColectoButtonVariant.primary,
                  onPressed: _saveModelSettings,
                ),
                ColectoButton(
                  label: 'Test connection',
                  icon: LucideIcons.plugZap,
                  onPressed: () => c.testAiConnection('decision'),
                ),
              ],
            ),
          ],
        ),
        ColectoSection(
          title: 'Label rules',
          description: accountLabels.isEmpty
              ? 'Create a label from the message list first, then teach Colecto '
                    'what belongs in it.'
              : 'Describe each label, add patterns, and preview the result '
                    'against your Inbox.',
          children: [
            if (activeLabel != null && currentRule != null) ...[
              Wrap(
                spacing: 6,
                runSpacing: 6,
                children: [
                  for (final label in accountLabels)
                    ColectoChip(
                      label: label,
                      selected: activeLabel == label,
                      onTap: () {
                        final rule =
                            s.labels[c.activeAccount]?[label] ??
                            const LabelRule();
                        setState(() {
                          _selectedLabel = label;
                          _ruleDescCtrl.text = rule.description;
                          _previewResult = null;
                        });
                      },
                    ),
                ],
              ),
              const SizedBox(height: 16),
              ColectoSettingRow(
                title: 'Apply this rule',
                trailing: ColectoSwitch(
                  value: currentRule.enabled,
                  onChanged: (v) => _saveLabelRule(
                    activeLabel,
                    currentRule.copyWith(enabled: v),
                  ),
                ),
              ),
              ColectoSettingRow(
                title: 'Ask the labelling model',
                description: 'Otherwise only the patterns below are used.',
                trailing: ColectoSwitch(
                  value: currentRule.model,
                  onChanged: (v) => _saveLabelRule(
                    activeLabel,
                    currentRule.copyWith(model: v),
                  ),
                ),
              ),
              const SizedBox(height: 10),
              ColectoTextField(
                controller: _ruleDescCtrl,
                labelText: 'What belongs in “$activeLabel”',
                hintText: 'Receipts and invoices from shops and services',
                helperText: 'Press Enter to save.',
                onSubmitted: (v) => _saveLabelRule(
                  activeLabel,
                  currentRule.copyWith(description: v),
                ),
              ),
              const SizedBox(height: 16),
              const ColectoLabel('Patterns'),
              for (var i = 0; i < currentRule.rules.length; i++)
                Padding(
                  padding: const EdgeInsets.only(bottom: 6),
                  child: Row(
                    children: [
                      ColectoBadge(text: currentRule.rules[i].field.name),
                      const SizedBox(width: 10),
                      Expanded(
                        child: Text(
                          currentRule.rules[i].pattern,
                          style: TextStyle(
                            fontFamily: 'monospace',
                            fontSize: 12.5,
                            color: p.ink,
                          ),
                        ),
                      ),
                      ColectoIconButton(
                        icon: LucideIcons.x,
                        tooltip: 'Remove pattern',
                        size: 14,
                        onPressed: () {
                          final next = List<MatchRule>.from(currentRule.rules)
                            ..removeAt(i);
                          _saveLabelRule(
                            activeLabel,
                            currentRule.copyWith(rules: next),
                          );
                        },
                      ),
                    ],
                  ),
                ),
              LayoutBuilder(
                builder: (context, box) {
                  final parts = <Widget>[
                    ColectoSelect<RuleField>(
                      value: _ruleField,
                      minWidth: 120,
                      options: [for (final f in RuleField.values) (f, f.name)],
                      onChanged: (f) => setState(() => _ruleField = f),
                    ),
                    ColectoTextField(
                      controller: _rulePatternCtrl,
                      hintText: 'invoice|receipt',
                      monospace: true,
                    ),
                    ColectoButton(
                      label: 'Add',
                      icon: LucideIcons.plus,
                      onPressed: () {
                        final pat = _rulePatternCtrl.text.trim();
                        if (pat.isEmpty) return;
                        _rulePatternCtrl.clear();
                        _saveLabelRule(
                          activeLabel,
                          currentRule.copyWith(
                            rules: [
                              ...currentRule.rules,
                              MatchRule(field: _ruleField, pattern: pat),
                            ],
                          ),
                        );
                      },
                    ),
                  ];
                  // Narrow pages stack the pattern controls.
                  if (box.maxWidth < 460) {
                    return Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: [
                        parts[0],
                        SizedBox(width: box.maxWidth, child: parts[1]),
                        parts[2],
                      ],
                    );
                  }
                  return Row(
                    children: [
                      parts[0],
                      const SizedBox(width: 8),
                      Expanded(child: parts[1]),
                      const SizedBox(width: 8),
                      parts[2],
                    ],
                  );
                },
              ),
              const SizedBox(height: 16),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  ColectoButton(
                    label: 'Preview on Inbox',
                    icon: LucideIcons.eye,
                    onPressed: () async {
                      final res = await c.previewAiRule(currentRule);
                      if (mounted) setState(() => _previewResult = res);
                    },
                  ),
                  ColectoButton(
                    label: 'Relabel Inbox now',
                    icon: LucideIcons.sparkles,
                    variant: ColectoButtonVariant.ghost,
                    onPressed: () => c.reclassifyMessages(
                      c.messages.map((m) => m.id).toList(),
                    ),
                  ),
                ],
              ),
              if (_previewResult != null) ...[
                const SizedBox(height: 14),
                ColectoNotice(
                  icon: LucideIcons.listFilter,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Matches ${_previewResult!.count} of '
                        '${_previewResult!.checked} messages.',
                      ),
                      for (final m in _previewResult!.matches.take(5))
                        Text(
                          '${m.sender} — ${m.subject}',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(color: p.ink2),
                        ),
                    ],
                  ),
                ),
              ],
            ],
          ],
        ),
      ],
    );
  }

  Future<void> _saveLabelRule(String label, LabelRule rule) async {
    final s = widget.controller.aiSettings.settings;
    final nextLabels = <String, Map<String, LabelRule>>{
      for (final e in s.labels.entries)
        e.key: Map<String, LabelRule>.from(e.value),
    };
    nextLabels.putIfAbsent(widget.controller.activeAccount, () => {})[label] =
        rule;
    await widget.controller.saveAiSettings(s.copyWith(labels: nextLabels));
  }
}
