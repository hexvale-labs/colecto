import 'package:flutter/foundation.dart';

enum ListDensity {
  full,
  medium,
  compact;

  static ListDensity fromJson(String? value) => switch (value) {
    'medium' => ListDensity.medium,
    'compact' => ListDensity.compact,
    _ => ListDensity.full,
  };

  String toJson() => name;

  String get label => switch (this) {
    ListDensity.full => 'Full',
    ListDensity.medium => 'Medium',
    ListDensity.compact => 'Compact',
  };
}

enum NotificationCorner {
  topLeft('top-left', 'Top left'),
  top('top', 'Top'),
  topRight('top-right', 'Top right'),
  right('right', 'Right'),
  bottomRight('bottom-right', 'Bottom right'),
  bottom('bottom', 'Bottom'),
  bottomLeft('bottom-left', 'Bottom left'),
  left('left', 'Left');

  const NotificationCorner(this.wire, this.label);
  final String wire;
  final String label;

  static NotificationCorner fromJson(String? value) {
    for (final c in NotificationCorner.values) {
      if (c.wire == value) return c;
    }
    return NotificationCorner.top;
  }
}

@immutable
class TextSpacing {
  const TextSpacing({
    this.letter = 0,
    this.word = 0,
    this.line = 0,
    this.paragraph = 0,
  });

  final int letter;
  final int word;
  final int line;
  final int paragraph;

  TextSpacing copyWith({int? letter, int? word, int? line, int? paragraph}) =>
      TextSpacing(
        letter: letter ?? this.letter,
        word: word ?? this.word,
        line: line ?? this.line,
        paragraph: paragraph ?? this.paragraph,
      );

  factory TextSpacing.fromJson(Map<String, dynamic>? json) => TextSpacing(
    letter: (json?['letter'] as num?)?.toInt() ?? 0,
    word: (json?['word'] as num?)?.toInt() ?? 0,
    line: (json?['line'] as num?)?.toInt() ?? 0,
    paragraph: (json?['paragraph'] as num?)?.toInt() ?? 0,
  );

  Map<String, dynamic> toJson() => {
    'letter': letter,
    'word': word,
    'line': line,
    'paragraph': paragraph,
  };
}

@immutable
class NotificationPreferences {
  const NotificationPreferences({
    this.enabled = true,
    this.system = false,
    this.position = NotificationCorner.top,
  });

  final bool enabled;
  final bool system;
  final NotificationCorner position;

  NotificationPreferences copyWith({
    bool? enabled,
    bool? system,
    NotificationCorner? position,
  }) => NotificationPreferences(
    enabled: enabled ?? this.enabled,
    system: system ?? this.system,
    position: position ?? this.position,
  );

  factory NotificationPreferences.fromJson(Map<String, dynamic>? json) =>
      NotificationPreferences(
        enabled: json?['enabled'] as bool? ?? true,
        system: json?['system'] as bool? ?? false,
        position: NotificationCorner.fromJson(json?['position'] as String?),
      );

  Map<String, dynamic> toJson() => {
    'enabled': enabled,
    'system': system,
    'position': position.wire,
  };
}

@immutable
class PrefetchPreferences {
  const PrefetchPreferences({this.enabled = true, this.count = 50});

  final bool enabled;
  final int count;

  PrefetchPreferences copyWith({bool? enabled, int? count}) =>
      PrefetchPreferences(
        enabled: enabled ?? this.enabled,
        count: count ?? this.count,
      );

  factory PrefetchPreferences.fromJson(Map<String, dynamic>? json) =>
      PrefetchPreferences(
        enabled: json?['enabled'] as bool? ?? true,
        count: (json?['count'] as num?)?.toInt() ?? 50,
      );

  Map<String, dynamic> toJson() => {'enabled': enabled, 'count': count};
}

@immutable
class Preferences {
  const Preferences({
    this.mcp = false,
    this.account,
    this.workspace,
    this.accent,
    this.font,
    this.fontSize,
    this.design,
    this.zoom,
    this.textSpacing = const TextSpacing(),
    this.notifications = const NotificationPreferences(),
    this.remoteImages = false,
    this.accountsEnabled = const {},
    this.onboarded = false,
    this.listDensity = ListDensity.full,
    this.dateGroups = true,
    this.notifiedRelease,
    this.prefetch = const PrefetchPreferences(),
    this.changed = const {},
  });

  final bool mcp;
  final String? account;
  final Map<String, dynamic>? workspace;
  final String? accent;
  final String? font;
  final int? fontSize;

  /// The UI design system id (`colecto`, `fluent`, `macos`, `material`,
  /// `cupertino`); null means the platform's own. An empty string in a
  /// patch restores that.
  final String? design;

  /// Whole-interface zoom in percent (50–200); null means 100. In a patch,
  /// 100 restores the default.
  final int? zoom;
  final TextSpacing textSpacing;
  final NotificationPreferences notifications;
  final bool remoteImages;
  final Map<String, bool> accountsEnabled;
  final bool onboarded;
  final ListDensity listDensity;
  final bool dateGroups;
  final String? notifiedRelease;
  final PrefetchPreferences prefetch;
  final Set<String> changed;

  bool accountEnabled(String id) => accountsEnabled[id] ?? (id != 'demo');

  factory Preferences.fromJson(Map<String, dynamic> json) {
    final rawEnabled = json['accounts_enabled'] as Map<String, dynamic>? ?? {};
    return Preferences(
      mcp: json['mcp'] as bool? ?? false,
      account: json['account'] as String?,
      workspace: json['workspace'] is Map<String, dynamic>
          ? Map<String, dynamic>.from(json['workspace'] as Map<String, dynamic>)
          : null,
      accent: json['accent'] as String?,
      font: json['font'] as String?,
      fontSize: (json['font_size'] as num?)?.toInt(),
      design: json['design'] as String?,
      zoom: (json['zoom'] as num?)?.toInt(),
      textSpacing: TextSpacing.fromJson(
        json['text_spacing'] as Map<String, dynamic>?,
      ),
      notifications: NotificationPreferences.fromJson(
        json['notifications'] as Map<String, dynamic>?,
      ),
      remoteImages: json['remote_images'] as bool? ?? false,
      accountsEnabled: rawEnabled.map((k, v) => MapEntry(k, v == true)),
      onboarded: json['onboarded'] as bool? ?? false,
      listDensity: ListDensity.fromJson(json['list_density'] as String?),
      dateGroups: json['date_groups'] as bool? ?? true,
      notifiedRelease: json['notified_release'] as String?,
      prefetch: PrefetchPreferences.fromJson(
        json['prefetch'] as Map<String, dynamic>?,
      ),
      changed: (json['changed'] as List<dynamic>? ?? const [])
          .map((e) => e.toString())
          .toSet(),
    );
  }
}

@immutable
class PreferencesPatch {
  const PreferencesPatch({
    this.mcp,
    this.account,
    this.workspace,
    this.accent,
    this.font,
    this.fontSize,
    this.design,
    this.zoom,
    this.textSpacing,
    this.notifications,
    this.remoteImages,
    this.accountsEnabled,
    this.onboarded,
    this.listDensity,
    this.dateGroups,
    this.notifiedRelease,
    this.prefetch,
    this.reset = const [],
  });

  final bool? mcp;
  final String? account;
  final Map<String, dynamic>? workspace;
  final String? accent;
  final String? font;
  final int? fontSize;

  /// The UI design system id (`colecto`, `fluent`, `macos`, `material`,
  /// `cupertino`); null means the platform's own. An empty string in a
  /// patch restores that.
  final String? design;

  /// Whole-interface zoom in percent (50–200); null means 100. In a patch,
  /// 100 restores the default.
  final int? zoom;
  final TextSpacing? textSpacing;
  final NotificationPreferences? notifications;
  final bool? remoteImages;
  final Map<String, bool>? accountsEnabled;
  final bool? onboarded;
  final ListDensity? listDensity;
  final bool? dateGroups;
  final String? notifiedRelease;
  final PrefetchPreferences? prefetch;
  final List<String> reset;

  Map<String, dynamic> toJson() => {
    if (mcp != null) 'mcp': mcp,
    if (account != null) 'account': account,
    if (workspace != null) 'workspace': workspace,
    if (accent != null) 'accent': accent,
    if (font != null) 'font': font,
    if (fontSize != null) 'font_size': fontSize,
    if (design != null) 'design': design,
    if (zoom != null) 'zoom': zoom,
    if (textSpacing != null) 'text_spacing': textSpacing!.toJson(),
    if (notifications != null) 'notifications': notifications!.toJson(),
    if (remoteImages != null) 'remote_images': remoteImages,
    if (accountsEnabled != null) 'accounts_enabled': accountsEnabled,
    if (onboarded != null) 'onboarded': onboarded,
    if (listDensity != null) 'list_density': listDensity!.toJson(),
    if (dateGroups != null) 'date_groups': dateGroups,
    if (notifiedRelease != null) 'notified_release': notifiedRelease,
    if (prefetch != null) 'prefetch': prefetch!.toJson(),
    if (reset.isNotEmpty) 'reset': reset,
  };
}

@immutable
class VersionInfo {
  const VersionInfo({
    required this.name,
    required this.version,
    this.commit,
    required this.api,
  });

  final String name;
  final String version;
  final String? commit;
  final String api;

  factory VersionInfo.fromJson(Map<String, dynamic> json) => VersionInfo(
    name: json['name'] as String? ?? 'colecto',
    version: json['version'] as String? ?? '0.1.0',
    commit: json['commit'] as String?,
    api: json['api'] as String? ?? 'v1',
  );
}

@immutable
class ReleaseInfo {
  const ReleaseInfo({required this.version, required this.url});

  final String version;
  final String url;

  factory ReleaseInfo.fromJson(Map<String, dynamic> json) => ReleaseInfo(
    version: json['version'] as String? ?? '',
    url: json['url'] as String? ?? '',
  );
}

@immutable
class ReleaseStatus {
  const ReleaseStatus({required this.current, this.latest});

  final String current;
  final ReleaseInfo? latest;

  factory ReleaseStatus.fromJson(Map<String, dynamic> json) => ReleaseStatus(
    current: json['current'] as String? ?? '0.1.0',
    latest: json['latest'] is Map<String, dynamic>
        ? ReleaseInfo.fromJson(json['latest'] as Map<String, dynamic>)
        : null,
  );
}
