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
