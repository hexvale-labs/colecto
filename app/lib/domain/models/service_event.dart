import 'package:flutter/foundation.dart';

import 'account_models.dart';

class MailBackendException implements Exception {
  const MailBackendException(
    this.message, {
    this.category = 'service',
    this.statusCode,
  });

  final String message;
  final String category;
  final int? statusCode;

  bool get isAuthRequired => statusCode == 401 || category == 'authentication';

  @override
  String toString() => message;
}

@immutable
sealed class ServiceEvent {
  const ServiceEvent();

  /// Decodes a wire event emitted by Rust (`colecto-daemon` SSE or
  /// `colecto-ffi` JSON envelope). Returns `null` for unknown event names.
  static ServiceEvent? fromWire(String name, Map<String, dynamic> payload) {
    return switch (name) {
      'ready' => const ReadyServiceEvent(),
      'resync' => const ResyncServiceEvent(),
      'mail' => MailServiceEvent(
        account: payload['account'] as String? ?? '',
        ids: (payload['ids'] as List<dynamic>? ?? const [])
            .map((e) => e.toString())
            .toList(growable: false),
      ),
      'sync' => SyncServiceEvent(AccountSync.fromJson(payload)),
      'approval' => ApprovalServiceEvent(
        id: payload['id'] as String? ?? '',
        account: payload['account'] as String? ?? '',
        from: payload['from'] as String? ?? '',
        subject: payload['subject'] as String? ?? '',
      ),
      'navigate' => NavigateServiceEvent(
        target: payload['target'] as String? ?? '',
        account: payload['account'] as String? ?? '',
      ),
      'mcp' => McpServiceEvent(on: payload['on'] == true),
      'preferences' => const PreferencesServiceEvent(),
      'labels' => LabelsUpdatedServiceEvent(
        account: payload['account'] as String? ?? '',
      ),
      _ => null,
    };
  }
}

final class ReadyServiceEvent extends ServiceEvent {
  const ReadyServiceEvent();
}

final class ResyncServiceEvent extends ServiceEvent {
  const ResyncServiceEvent();
}

final class MailServiceEvent extends ServiceEvent {
  const MailServiceEvent({required this.account, this.ids = const []});

  final String account;
  final List<String> ids;
}

final class SyncServiceEvent extends ServiceEvent {
  const SyncServiceEvent(this.sync);

  final AccountSync sync;
}

final class ApprovalServiceEvent extends ServiceEvent {
  const ApprovalServiceEvent({
    required this.id,
    required this.account,
    required this.from,
    required this.subject,
  });

  final String id;
  final String account;
  final String from;
  final String subject;
}

final class NavigateServiceEvent extends ServiceEvent {
  const NavigateServiceEvent({required this.target, required this.account});

  final String target;
  final String account;
}

final class McpServiceEvent extends ServiceEvent {
  const McpServiceEvent({required this.on});

  final bool on;
}

final class PreferencesServiceEvent extends ServiceEvent {
  const PreferencesServiceEvent();
}

final class LabelsUpdatedServiceEvent extends ServiceEvent {
  const LabelsUpdatedServiceEvent({required this.account});

  final String account;
}
