import 'package:flutter/foundation.dart';

enum AiEffort {
  none,
  low,
  medium,
  high;

  static AiEffort? fromJson(String? value) => switch (value) {
    'none' => AiEffort.none,
    'low' => AiEffort.low,
    'medium' => AiEffort.medium,
    'high' => AiEffort.high,
    _ => null,
  };

  String toJson() => name;
}

@immutable
class TextModelConfig {
  const TextModelConfig({
    this.baseUrl = '',
    this.model = '',
    this.maxTokens = 1024,
    this.effort,
  });

  final String baseUrl;
  final String model;
  final int maxTokens;
  final AiEffort? effort;

  TextModelConfig copyWith({
    String? baseUrl,
    String? model,
    int? maxTokens,
    AiEffort? effort,
    bool clearEffort = false,
  }) => TextModelConfig(
    baseUrl: baseUrl ?? this.baseUrl,
    model: model ?? this.model,
    maxTokens: maxTokens ?? this.maxTokens,
    effort: clearEffort ? null : (effort ?? this.effort),
  );

  factory TextModelConfig.fromJson(Map<String, dynamic>? json) =>
      TextModelConfig(
        baseUrl: json?['base_url'] as String? ?? '',
        model: json?['model'] as String? ?? '',
        maxTokens: (json?['max_tokens'] as num?)?.toInt() ?? 1024,
        effort: AiEffort.fromJson(json?['effort'] as String?),
      );

  Map<String, dynamic> toJson() => {
    'base_url': baseUrl,
    'model': model,
    'max_tokens': maxTokens,
    'effort': effort?.toJson(),
  };
}

@immutable
class DecisionModelConfig {
  const DecisionModelConfig({
    this.baseUrl = 'https://openrouter.ai',
    this.model = 'typesafe/jev-1.14',
    this.threshold = 0.7,
  });

  final String baseUrl;
  final String model;
  final double threshold;

  DecisionModelConfig copyWith({
    String? baseUrl,
    String? model,
    double? threshold,
  }) => DecisionModelConfig(
    baseUrl: baseUrl ?? this.baseUrl,
    model: model ?? this.model,
    threshold: threshold ?? this.threshold,
  );

  factory DecisionModelConfig.fromJson(Map<String, dynamic>? json) =>
      DecisionModelConfig(
        baseUrl: json?['base_url'] as String? ?? 'https://openrouter.ai',
        model: json?['model'] as String? ?? 'typesafe/jev-1.14',
        threshold: (json?['threshold'] as num?)?.toDouble() ?? 0.7,
      );

  Map<String, dynamic> toJson() => {
    'base_url': baseUrl,
    'model': model,
    'threshold': threshold,
  };
}

enum RuleField {
  sender,
  subject,
  body,
  any;

  static RuleField fromJson(String? value) => switch (value) {
    'sender' => RuleField.sender,
    'subject' => RuleField.subject,
    'body' => RuleField.body,
    _ => RuleField.any,
  };

  String toJson() => name;
}

@immutable
class MatchRule {
  const MatchRule({required this.field, required this.pattern});

  final RuleField field;
  final String pattern;

  factory MatchRule.fromJson(Map<String, dynamic> json) => MatchRule(
    field: RuleField.fromJson(json['field'] as String?),
    pattern: json['pattern'] as String? ?? '',
  );

  Map<String, dynamic> toJson() => {
    'field': field.toJson(),
    'pattern': pattern,
  };
}

@immutable
class LabelRule {
  const LabelRule({
    this.enabled = false,
    this.model = false,
    this.description = '',
    this.rules = const [],
  });

  final bool enabled;
  final bool model;
  final String description;
  final List<MatchRule> rules;

  LabelRule copyWith({
    bool? enabled,
    bool? model,
    String? description,
    List<MatchRule>? rules,
  }) => LabelRule(
    enabled: enabled ?? this.enabled,
    model: model ?? this.model,
    description: description ?? this.description,
    rules: rules ?? this.rules,
  );

  factory LabelRule.fromJson(Map<String, dynamic> json) => LabelRule(
    enabled: json['enabled'] as bool? ?? false,
    model: json['model'] as bool? ?? false,
    description: json['description'] as String? ?? '',
    rules: (json['rules'] as List<dynamic>? ?? const [])
        .map((e) => MatchRule.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
  );

  Map<String, dynamic> toJson() => {
    'enabled': enabled,
    'model': model,
    'description': description,
    'rules': rules.map((r) => r.toJson()).toList(growable: false),
  };
}

@immutable
class AiSettings {
  const AiSettings({
    this.enabled = false,
    this.text = const TextModelConfig(),
    this.decision = const DecisionModelConfig(),
    this.autolabel = false,
    this.labels = const {},
    this.timeoutSeconds = 120,
  });

  final bool enabled;
  final TextModelConfig text;
  final DecisionModelConfig decision;
  final bool autolabel;
  final Map<String, Map<String, LabelRule>> labels;
  final int timeoutSeconds;

  AiSettings copyWith({
    bool? enabled,
    TextModelConfig? text,
    DecisionModelConfig? decision,
    bool? autolabel,
    Map<String, Map<String, LabelRule>>? labels,
    int? timeoutSeconds,
  }) => AiSettings(
    enabled: enabled ?? this.enabled,
    text: text ?? this.text,
    decision: decision ?? this.decision,
    autolabel: autolabel ?? this.autolabel,
    labels: labels ?? this.labels,
    timeoutSeconds: timeoutSeconds ?? this.timeoutSeconds,
  );

  factory AiSettings.fromJson(Map<String, dynamic> json) {
    final rawLabels = json['labels'] as Map<String, dynamic>? ?? const {};
    final parsedLabels = <String, Map<String, LabelRule>>{};
    for (final entry in rawLabels.entries) {
      if (entry.value is Map<String, dynamic>) {
        final inner = entry.value as Map<String, dynamic>;
        parsedLabels[entry.key] = inner.map(
          (k, v) => MapEntry(k, LabelRule.fromJson(v as Map<String, dynamic>)),
        );
      }
    }
    return AiSettings(
      enabled: json['enabled'] as bool? ?? false,
      text: TextModelConfig.fromJson(json['text'] as Map<String, dynamic>?),
      decision: DecisionModelConfig.fromJson(
        json['decision'] as Map<String, dynamic>?,
      ),
      autolabel: json['autolabel'] as bool? ?? false,
      labels: parsedLabels,
      timeoutSeconds: (json['timeout_seconds'] as num?)?.toInt() ?? 120,
    );
  }

  Map<String, dynamic> toJson() => {
    'enabled': enabled,
    'text': text.toJson(),
    'decision': decision.toJson(),
    'autolabel': autolabel,
    'labels': labels.map(
      (account, rules) => MapEntry(
        account,
        rules.map((label, rule) => MapEntry(label, rule.toJson())),
      ),
    ),
    'timeout_seconds': timeoutSeconds,
  };
}

@immutable
class AiKeysView {
  const AiKeysView({this.text = false, this.decision = false});

  final bool text;
  final bool decision;

  factory AiKeysView.fromJson(Map<String, dynamic>? json) => AiKeysView(
    text: json?['text'] as bool? ?? false,
    decision: json?['decision'] as bool? ?? false,
  );
}

@immutable
class AiSettingsView {
  const AiSettingsView({
    this.settings = const AiSettings(),
    this.keys = const AiKeysView(),
  });

  final AiSettings settings;
  final AiKeysView keys;

  factory AiSettingsView.fromJson(Map<String, dynamic> json) => AiSettingsView(
    settings: AiSettings.fromJson(
      json['settings'] as Map<String, dynamic>? ?? const {},
    ),
    keys: AiKeysView.fromJson(json['keys'] as Map<String, dynamic>?),
  );
}

@immutable
class PreviewMatch {
  const PreviewMatch({required this.subject, required this.sender});

  final String subject;
  final String sender;

  factory PreviewMatch.fromJson(Map<String, dynamic> json) => PreviewMatch(
    subject: json['subject'] as String? ?? '',
    sender: json['sender'] as String? ?? '',
  );
}

@immutable
class PreviewResult {
  const PreviewResult({
    this.errors = const [],
    this.checked = 0,
    this.count = 0,
    this.matches = const [],
  });

  final List<String?> errors;
  final int checked;
  final int count;
  final List<PreviewMatch> matches;

  factory PreviewResult.fromJson(Map<String, dynamic> json) => PreviewResult(
    errors: (json['errors'] as List<dynamic>? ?? const [])
        .map((e) => e as String?)
        .toList(growable: false),
    checked: (json['checked'] as num?)?.toInt() ?? 0,
    count: (json['count'] as num?)?.toInt() ?? 0,
    matches: (json['matches'] as List<dynamic>? ?? const [])
        .map((e) => PreviewMatch.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
  );
}

@immutable
class ReclassifiedResult {
  const ReclassifiedResult({
    this.checked = 0,
    this.changed = 0,
    this.unanswered = 0,
  });

  final int checked;
  final int changed;
  final int unanswered;

  factory ReclassifiedResult.fromJson(Map<String, dynamic> json) =>
      ReclassifiedResult(
        checked: (json['checked'] as num?)?.toInt() ?? 0,
        changed: (json['changed'] as num?)?.toInt() ?? 0,
        unanswered: (json['unanswered'] as num?)?.toInt() ?? 0,
      );
}
