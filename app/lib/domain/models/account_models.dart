import 'package:flutter/foundation.dart';

enum MailProtocol {
  imap,
  jmap;

  static MailProtocol fromJson(String? value) =>
      value == 'jmap' ? MailProtocol.jmap : MailProtocol.imap;

  String toJson() => name;
}

@immutable
class AccountSettings {
  const AccountSettings({
    this.name = '',
    required this.address,
    this.protocol = MailProtocol.imap,
    this.imapHost = '',
    this.imapPort = 993,
    this.smtpHost = '',
    this.smtpPort = 587,
    this.username = '',
    this.sessionUrl = '',
    this.imapStarttls = false,
    this.tlsCertificate = '',
  });

  final String name;
  final String address;
  final MailProtocol protocol;
  final String imapHost;
  final int imapPort;
  final String smtpHost;
  final int smtpPort;
  final String username;
  final String sessionUrl;
  final bool imapStarttls;
  final String tlsCertificate;

  factory AccountSettings.fromJson(Map<String, dynamic> json) =>
      AccountSettings(
        name: json['name'] as String? ?? '',
        address: json['address'] as String? ?? '',
        protocol: MailProtocol.fromJson(json['protocol'] as String?),
        imapHost: json['imap_host'] as String? ?? '',
        imapPort: (json['imap_port'] as num?)?.toInt() ?? 993,
        smtpHost: json['smtp_host'] as String? ?? '',
        smtpPort: (json['smtp_port'] as num?)?.toInt() ?? 587,
        username: json['username'] as String? ?? '',
        sessionUrl: json['session_url'] as String? ?? '',
        imapStarttls: json['imap_starttls'] as bool? ?? false,
        tlsCertificate: json['tls_certificate'] as String? ?? '',
      );

  Map<String, dynamic> toJson() => {
    'name': name,
    'address': address,
    'protocol': protocol.toJson(),
    'imap_host': imapHost,
    'imap_port': imapPort,
    'smtp_host': smtpHost,
    'smtp_port': smtpPort,
    'username': username,
    'session_url': sessionUrl,
    if (imapStarttls) 'imap_starttls': true,
    if (tlsCertificate.isNotEmpty) 'tls_certificate': tlsCertificate,
  };
}

@immutable
class AccountInput {
  const AccountInput({this.id, required this.settings, required this.secret});

  final String? id;
  final AccountSettings settings;
  final String secret;

  Map<String, dynamic> toJson() => {
    if (id != null && id!.isNotEmpty) 'id': id,
    ...settings.toJson(),
    'secret': secret,
  };
}

@immutable
class AccountInfo {
  const AccountInfo({
    required this.id,
    this.settings,
    required this.connected,
    required this.configured,
  });

  final String id;
  final AccountSettings? settings;
  final bool connected;
  final bool configured;

  String get displayName {
    if (id == 'demo') return 'Demo account';
    final name = settings?.name.trim() ?? '';
    if (name.isNotEmpty) return name;
    final addr = settings?.address.trim() ?? '';
    if (addr.isNotEmpty) return addr;
    return id;
  }

  String get address =>
      settings?.address ?? (id == 'demo' ? 'demo@colecto.local' : id);

  factory AccountInfo.fromJson(Map<String, dynamic> json) => AccountInfo(
    id: json['id'] as String? ?? '',
    settings: json['settings'] is Map<String, dynamic>
        ? AccountSettings.fromJson(json['settings'] as Map<String, dynamic>)
        : null,
    connected: json['connected'] as bool? ?? false,
    configured: json['configured'] as bool? ?? false,
  );
}

@immutable
class SecurityInfo {
  const SecurityInfo({
    required this.cleanupPending,
    required this.protected,
    required this.locked,
  });

  final bool cleanupPending;
  final bool protected;
  final bool locked;

  factory SecurityInfo.fromJson(Map<String, dynamic> json) => SecurityInfo(
    cleanupPending: json['cleanup_pending'] as bool? ?? false,
    protected: json['protected'] as bool? ?? false,
    locked: json['locked'] as bool? ?? false,
  );
}

enum DiscoverySecurity {
  tls,
  starttls;

  static DiscoverySecurity fromJson(String? value) =>
      value == 'starttls' ? DiscoverySecurity.starttls : DiscoverySecurity.tls;
}

@immutable
class DiscoveryServer {
  const DiscoveryServer({
    required this.host,
    required this.port,
    required this.security,
  });

  final String host;
  final int port;
  final DiscoverySecurity security;

  factory DiscoveryServer.fromJson(Map<String, dynamic> json) =>
      DiscoveryServer(
        host: json['host'] as String? ?? '',
        port: (json['port'] as num?)?.toInt() ?? 993,
        security: DiscoverySecurity.fromJson(json['security'] as String?),
      );
}

@immutable
class DiscoveryResult {
  const DiscoveryResult({
    this.sources = const [],
    this.provider,
    this.imap,
    this.smtp,
    this.jmap,
    this.username,
    this.appPassword = false,
    this.limitation,
  });

  final List<String> sources;
  final String? provider;
  final DiscoveryServer? imap;
  final DiscoveryServer? smtp;
  final String? jmap;
  final String? username;
  final bool appPassword;
  final String? limitation;

  factory DiscoveryResult.fromJson(Map<String, dynamic> json) =>
      DiscoveryResult(
        sources: (json['sources'] as List<dynamic>? ?? const [])
            .map((e) => e.toString())
            .toList(growable: false),
        provider: json['provider'] as String?,
        imap: json['imap'] is Map<String, dynamic>
            ? DiscoveryServer.fromJson(json['imap'] as Map<String, dynamic>)
            : null,
        smtp: json['smtp'] is Map<String, dynamic>
            ? DiscoveryServer.fromJson(json['smtp'] as Map<String, dynamic>)
            : null,
        jmap: json['jmap'] as String?,
        username: json['username'] as String?,
        appPassword: json['app_password'] as bool? ?? false,
        limitation: json['limitation'] as String?,
      );
}

enum SyncMode {
  local,
  jmapPush,
  imapIdle,
  polling,
  offline;

  static SyncMode fromJson(String? value) {
    return switch (value) {
      'local' => SyncMode.local,
      'jmap_push' => SyncMode.jmapPush,
      'imap_idle' => SyncMode.imapIdle,
      'polling' => SyncMode.polling,
      _ => SyncMode.offline,
    };
  }

  String get label => switch (this) {
    SyncMode.local => 'Local',
    SyncMode.jmapPush => 'JMAP Push',
    SyncMode.imapIdle => 'IMAP IDLE',
    SyncMode.polling => 'Polling',
    SyncMode.offline => 'Offline',
  };
}

enum SyncActivity {
  idle,
  syncing,
  prefetching;

  static SyncActivity fromJson(String? value) {
    return switch (value) {
      'syncing' => SyncActivity.syncing,
      'prefetching' => SyncActivity.prefetching,
      _ => SyncActivity.idle,
    };
  }
}

@immutable
class AccountSync {
  const AccountSync({
    required this.account,
    required this.protocol,
    required this.mode,
    required this.interval,
    required this.activity,
    this.progressDone,
    this.progressTotal,
    this.error,
    this.updatedAt,
  });

  final String account;
  final String protocol;
  final SyncMode mode;
  final int interval;
  final SyncActivity activity;
  final int? progressDone;
  final int? progressTotal;
  final String? error;
  final int? updatedAt;

  bool get isOnline =>
      mode == SyncMode.jmapPush ||
      mode == SyncMode.imapIdle ||
      mode == SyncMode.polling ||
      mode == SyncMode.local;

  factory AccountSync.fromJson(Map<String, dynamic> json) {
    int? done;
    int? total;
    final progress = json['progress'];
    if (progress is List && progress.length == 2) {
      done = (progress[0] as num?)?.toInt();
      total = (progress[1] as num?)?.toInt();
    }
    return AccountSync(
      account: json['account'] as String? ?? '',
      protocol: json['protocol'] as String? ?? 'demo',
      mode: SyncMode.fromJson(json['mode'] as String?),
      interval: (json['interval'] as num?)?.toInt() ?? 60,
      activity: SyncActivity.fromJson(json['activity'] as String?),
      progressDone: done,
      progressTotal: total,
      error: json['error'] as String?,
      updatedAt: (json['updated_at'] as num?)?.toInt(),
    );
  }
}
