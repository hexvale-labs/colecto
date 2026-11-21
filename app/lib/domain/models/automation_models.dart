import 'package:flutter/foundation.dart';

import 'mail_models.dart';

enum MailCapability {
  accountRead('account.read', 'Read account status'),
  messagesList('messages.list', 'List & search messages'),
  messagesRead('messages.read', 'Read message content'),
  messagesOrganize('messages.organize', 'Archive, move, flag & delete'),
  messagesLabel('messages.label', 'Manage local labels'),
  messagesSend('messages.send', 'Request to send mail'),
  draftsList('drafts.list', 'List drafts'),
  draftsRead('drafts.read', 'Read drafts'),
  draftsWrite('drafts.write', 'Save drafts'),
  aiText('ai.text', 'AI summaries & reply drafts'),
  aiDecide('ai.decide', 'AI auto-label decisions'),
  aiConfigure('ai.configure', 'Configure AI settings'),
  releasesRead('releases.read', 'Check latest release'),
  appNavigate('app.navigate', 'Navigate open windows'),
  settingsAppearance('settings.appearance', 'Appearance settings'),
  settingsNotifications('settings.notifications', 'Notification settings'),
  settingsMail('settings.mail', 'Mail & prefetch settings');

  const MailCapability(this.wire, this.label);
  final String wire;
  final String label;

  static MailCapability? fromJson(String? value) {
    for (final c in MailCapability.values) {
      if (c.wire == value) return c;
    }
    return null;
  }
}

@immutable
class ApiKeyMetadata {
  const ApiKeyMetadata({
    required this.id,
    required this.name,
    required this.accounts,
    required this.capabilities,
    required this.createdAt,
    this.expiresAt,
    this.lastUsedAt,
    this.revokedAt,
  });

  final String id;
  final String name;
  final List<String> accounts;
  final List<MailCapability> capabilities;
  final int createdAt;
  final int? expiresAt;
  final int? lastUsedAt;
  final int? revokedAt;

  bool get isRevoked => revokedAt != null;

  factory ApiKeyMetadata.fromJson(Map<String, dynamic> json) => ApiKeyMetadata(
    id: json['id'] as String? ?? '',
    name: json['name'] as String? ?? '',
    accounts: (json['accounts'] as List<dynamic>? ?? const [])
        .map((e) => e.toString())
        .toList(growable: false),
    capabilities: (json['capabilities'] as List<dynamic>? ?? const [])
        .map((e) => MailCapability.fromJson(e as String?))
        .whereType<MailCapability>()
        .toList(growable: false),
    createdAt: (json['created_at'] as num?)?.toInt() ?? 0,
    expiresAt: (json['expires_at'] as num?)?.toInt(),
    lastUsedAt: (json['last_used_at'] as num?)?.toInt(),
    revokedAt: (json['revoked_at'] as num?)?.toInt(),
  );
}

@immutable
class ApiKeyInput {
  const ApiKeyInput({
    required this.name,
    required this.accounts,
    required this.capabilities,
    this.expiresAt,
  });

  final String name;
  final List<String> accounts;
  final List<MailCapability> capabilities;
  final int? expiresAt;

  Map<String, dynamic> toJson() => {
    'name': name,
    'accounts': accounts,
    'capabilities': capabilities.map((c) => c.wire).toList(growable: false),
    'expires_at': expiresAt,
  };
}

@immutable
class MintedApiKey {
  const MintedApiKey({required this.key, required this.secret});

  final ApiKeyMetadata key;
  final String secret;

  factory MintedApiKey.fromJson(Map<String, dynamic> json) => MintedApiKey(
    key: ApiKeyMetadata.fromJson(json['key'] as Map<String, dynamic>),
    secret: json['secret'] as String? ?? '',
  );
}

enum ApprovalStatus {
  pending,
  rejected,
  expired,
  submitting,
  sent,
  uncertain;

  static ApprovalStatus fromJson(String? value) => switch (value) {
    'pending' => ApprovalStatus.pending,
    'rejected' => ApprovalStatus.rejected,
    'expired' => ApprovalStatus.expired,
    'submitting' => ApprovalStatus.submitting,
    'sent' => ApprovalStatus.sent,
    _ => ApprovalStatus.uncertain,
  };
}

@immutable
class PendingSend {
  const PendingSend({
    required this.id,
    required this.requester,
    required this.account,
    required this.from,
    this.snapshot,
    required this.createdAt,
    required this.expiresAt,
    required this.status,
  });

  final String id;
  final String requester;
  final String account;
  final String from;
  final DraftInput? snapshot;
  final int createdAt;
  final int expiresAt;
  final ApprovalStatus status;

  factory PendingSend.fromJson(Map<String, dynamic> json) => PendingSend(
    id: json['id'] as String? ?? '',
    requester: json['requester'] as String? ?? '',
    account: json['account'] as String? ?? '',
    from: json['from'] as String? ?? '',
    snapshot: json['snapshot'] is Map<String, dynamic>
        ? DraftInput.fromJson(json['snapshot'] as Map<String, dynamic>)
        : null,
    createdAt: (json['created_at'] as num?)?.toInt() ?? 0,
    expiresAt: (json['expires_at'] as num?)?.toInt() ?? 0,
    status: ApprovalStatus.fromJson(json['status'] as String?),
  );
}
