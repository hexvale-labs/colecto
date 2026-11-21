import 'package:flutter/foundation.dart';

enum FolderRole {
  inbox,
  sent,
  drafts,
  archive,
  trash,
  junk,
  other;

  static FolderRole fromJson(String? value) {
    return switch (value?.toLowerCase()) {
      'inbox' => FolderRole.inbox,
      'sent' => FolderRole.sent,
      'drafts' => FolderRole.drafts,
      'archive' => FolderRole.archive,
      'trash' => FolderRole.trash,
      'junk' => FolderRole.junk,
      _ => FolderRole.other,
    };
  }

  String toJson() => name;
}

@immutable
class Folder {
  const Folder({
    required this.id,
    required this.name,
    required this.role,
    required this.unread,
    required this.total,
  });

  final String id;
  final String name;
  final FolderRole role;
  final int unread;
  final int total;

  factory Folder.fromJson(Map<String, dynamic> json) => Folder(
    id: json['id'] as String? ?? '',
    name: json['name'] as String? ?? '',
    role: FolderRole.fromJson(json['role'] as String?),
    unread: (json['unread'] as num?)?.toInt() ?? 0,
    total: (json['total'] as num?)?.toInt() ?? 0,
  );

  Map<String, dynamic> toJson() => {
    'id': id,
    'name': name,
    'role': role.toJson(),
    'unread': unread,
    'total': total,
  };
}

@immutable
class MailMessage {
  const MailMessage({
    required this.id,
    required this.sender,
    required this.address,
    required this.subject,
    required this.body,
    required this.date,
    this.seen = false,
    this.flagged = false,
    this.thread = '',
  });

  final String id;
  final String sender;
  final String address;
  final String subject;
  final String body;
  final String date;
  final bool seen;
  final bool flagged;
  final String thread;

  String get displaySender =>
      sender.trim().isNotEmpty ? sender.trim() : address;

  String get preview => body.replaceAll(RegExp(r'\s+'), ' ').trim();

  MailMessage copyWith({
    bool? seen,
    bool? flagged,
    String? subject,
    String? body,
  }) {
    return MailMessage(
      id: id,
      sender: sender,
      address: address,
      subject: subject ?? this.subject,
      body: body ?? this.body,
      date: date,
      seen: seen ?? this.seen,
      flagged: flagged ?? this.flagged,
      thread: thread,
    );
  }

  factory MailMessage.fromJson(Map<String, dynamic> json) => MailMessage(
    id: json['id'] as String? ?? '',
    sender: json['sender'] as String? ?? '',
    address: json['address'] as String? ?? '',
    subject: json['subject'] as String? ?? '',
    body: json['body'] as String? ?? '',
    date: json['date'] as String? ?? '',
    seen: json['seen'] as bool? ?? false,
    flagged: json['flagged'] as bool? ?? false,
    thread: json['thread'] as String? ?? '',
  );

  Map<String, dynamic> toJson() => {
    'id': id,
    'sender': sender,
    'address': address,
    'subject': subject,
    'body': body,
    'date': date,
    'seen': seen,
    'flagged': flagged,
    if (thread.isNotEmpty) 'thread': thread,
  };
}

@immutable
class ThreadMessage {
  const ThreadMessage({required this.folder, required this.message});

  final String folder;
  final MailMessage message;

  factory ThreadMessage.fromJson(Map<String, dynamic> json) => ThreadMessage(
    folder: json['folder'] as String? ?? 'inbox',
    message: MailMessage.fromJson(
      json['message'] as Map<String, dynamic>? ?? const {},
    ),
  );
}

@immutable
class ConversationThread {
  const ConversationThread({
    required this.id,
    required this.subject,
    required this.messages,
    required this.unreadCount,
    required this.flagged,
    required this.latestDate,
    required this.participants,
  });

  final String id;
  final String subject;
  final List<MailMessage> messages;
  final int unreadCount;
  final bool flagged;
  final String latestDate;
  final List<String> participants;

  MailMessage get latest => messages.last;

  bool get hasUnread => unreadCount > 0;
}

/// Groups newest-first folder messages into chronological conversations,
/// mirroring `colecto_core::threading::group_conversations`.
List<ConversationThread> groupConversations(List<MailMessage> messages) {
  final order = <String>[];
  final grouped = <String, List<MailMessage>>{};

  for (final message in messages) {
    final key = message.thread.isEmpty
        ? 'msg:${message.id}'
        : 'thread:${message.thread}';
    if (!grouped.containsKey(key)) {
      order.add(key);
      grouped[key] = <MailMessage>[];
    }
    grouped[key]!.add(message);
  }

  return order
      .map((key) {
        final items = grouped[key]!.reversed.toList(growable: false);
        final latest = items.last;
        final unreadCount = items.where((m) => !m.seen).length;
        final flagged = items.any((m) => m.flagged);
        final seenNames = <String>{};
        final participants = <String>[];
        for (final m in items) {
          final name = m.displaySender;
          if (name.isNotEmpty && seenNames.add(name.toLowerCase())) {
            participants.add(name);
          }
        }
        return ConversationThread(
          id: key,
          subject: normalizeSubject(latest.subject),
          messages: items,
          unreadCount: unreadCount,
          flagged: flagged,
          latestDate: latest.date,
          participants: participants,
        );
      })
      .toList(growable: false);
}

String normalizeSubject(String subject) {
  var rest = subject.trim();
  while (true) {
    final lower = rest.toLowerCase();
    if (lower.startsWith('re:')) {
      rest = rest.substring(3).trimLeft();
    } else if (lower.startsWith('fwd:')) {
      rest = rest.substring(4).trimLeft();
    } else if (lower.startsWith('fw:')) {
      rest = rest.substring(3).trimLeft();
    } else {
      break;
    }
  }
  return rest.isEmpty ? '(No subject)' : rest;
}

@immutable
class MailSnapshot {
  const MailSnapshot({
    required this.folder,
    required this.messages,
    this.fetchedAt,
  });

  final String folder;
  final List<MailMessage> messages;
  final int? fetchedAt;

  factory MailSnapshot.fromJson(Map<String, dynamic> json) => MailSnapshot(
    folder: json['folder'] as String? ?? 'inbox',
    messages: (json['messages'] as List<dynamic>? ?? const [])
        .map((e) => MailMessage.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    fetchedAt: (json['fetched_at'] as num?)?.toInt(),
  );
}

@immutable
class AttachmentInfo {
  const AttachmentInfo({
    required this.index,
    required this.name,
    required this.contentType,
    required this.size,
  });

  final int index;
  final String name;
  final String contentType;
  final int size;

  String get formattedSize {
    if (size < 1024) return '$size B';
    if (size < 1024 * 1024) return '${(size / 1024).toStringAsFixed(1)} KB';
    if (size < 1024 * 1024 * 1024) {
      return '${(size / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(size / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }

  factory AttachmentInfo.fromJson(Map<String, dynamic> json) => AttachmentInfo(
    index: (json['index'] as num?)?.toInt() ?? 0,
    name: json['name'] as String? ?? 'attachment',
    contentType: json['content_type'] as String? ?? 'application/octet-stream',
    size: (json['size'] as num?)?.toInt() ?? 0,
  );
}

@immutable
class MessageContent {
  const MessageContent({
    required this.text,
    this.html,
    this.blockedRemoteImages = 0,
    this.attachments = const [],
    this.messageId = '',
    this.references = '',
    this.replyTo = '',
    this.to = '',
    this.cc = '',
  });

  final String text;
  final String? html;
  final int blockedRemoteImages;
  final List<AttachmentInfo> attachments;
  final String messageId;
  final String references;
  final String replyTo;
  final String to;
  final String cc;

  factory MessageContent.fromJson(Map<String, dynamic> json) => MessageContent(
    text: json['text'] as String? ?? '',
    html: json['html'] as String?,
    blockedRemoteImages: (json['blocked_remote_images'] as num?)?.toInt() ?? 0,
    attachments: (json['attachments'] as List<dynamic>? ?? const [])
        .map((e) => AttachmentInfo.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
    messageId: json['message_id'] as String? ?? '',
    references: json['references'] as String? ?? '',
    replyTo: json['reply_to'] as String? ?? '',
    to: json['to'] as String? ?? '',
    cc: json['cc'] as String? ?? '',
  );
}

@immutable
class OutgoingAttachment {
  const OutgoingAttachment({
    required this.name,
    required this.contentType,
    required this.data,
  });

  final String name;
  final String contentType;
  final String data;

  factory OutgoingAttachment.fromJson(Map<String, dynamic> json) =>
      OutgoingAttachment(
        name: json['name'] as String? ?? 'file',
        contentType:
            json['content_type'] as String? ?? 'application/octet-stream',
        data: json['data'] as String? ?? '',
      );

  Map<String, dynamic> toJson() => {
    'name': name,
    'content_type': contentType,
    'data': data,
  };
}

@immutable
class Draft {
  const Draft({
    required this.id,
    required this.recipient,
    this.cc = '',
    required this.subject,
    required this.body,
    this.inReplyTo = '',
    this.references = '',
    this.forwardOf,
  });

  final String id;
  final String recipient;
  final String cc;
  final String subject;
  final String body;
  final String inReplyTo;
  final String references;
  final String? forwardOf;

  factory Draft.fromJson(Map<String, dynamic> json) => Draft(
    id: json['id'] as String? ?? '',
    recipient: json['recipient'] as String? ?? '',
    cc: json['cc'] as String? ?? '',
    subject: json['subject'] as String? ?? '',
    body: json['body'] as String? ?? '',
    inReplyTo: json['in_reply_to'] as String? ?? '',
    references: json['references'] as String? ?? '',
    forwardOf: json['forward_of'] as String?,
  );
}

@immutable
class DraftInput {
  const DraftInput({
    this.id,
    required this.recipient,
    this.cc = '',
    required this.subject,
    required this.body,
    this.inReplyTo = '',
    this.references = '',
    this.forwardOf,
    this.attachments = const [],
  });

  final String? id;
  final String recipient;
  final String cc;
  final String subject;
  final String body;
  final String inReplyTo;
  final String references;
  final String? forwardOf;
  final List<OutgoingAttachment> attachments;

  factory DraftInput.fromJson(Map<String, dynamic> json) => DraftInput(
    id: json['id'] as String?,
    recipient: json['recipient'] as String? ?? '',
    cc: json['cc'] as String? ?? '',
    subject: json['subject'] as String? ?? '',
    body: json['body'] as String? ?? '',
    inReplyTo: json['in_reply_to'] as String? ?? '',
    references: json['references'] as String? ?? '',
    forwardOf: json['forward_of'] as String?,
    attachments: (json['attachments'] as List<dynamic>? ?? const [])
        .map((e) => OutgoingAttachment.fromJson(e as Map<String, dynamic>))
        .toList(growable: false),
  );

  Map<String, dynamic> toJson() => {
    if (id != null && id!.isNotEmpty) 'id': id,
    'recipient': recipient,
    if (cc.isNotEmpty) 'cc': cc,
    'subject': subject,
    'body': body,
    if (inReplyTo.isNotEmpty) 'in_reply_to': inReplyTo,
    if (references.isNotEmpty) 'references': references,
    if (forwardOf != null && forwardOf!.isNotEmpty) 'forward_of': forwardOf,
    if (attachments.isNotEmpty)
      'attachments': attachments.map((a) => a.toJson()).toList(growable: false),
  };
}

@immutable
class DeliveryOutcome {
  const DeliveryOutcome({
    required this.sent,
    this.approvalId,
    this.approvalStatus,
    this.expiresAt,
  });

  final bool sent;
  final String? approvalId;
  final String? approvalStatus;
  final int? expiresAt;

  bool get requiresApproval => approvalId != null;

  factory DeliveryOutcome.fromJson(dynamic json) {
    if (json is bool) {
      return DeliveryOutcome(sent: json);
    }
    if (json is Map<String, dynamic>) {
      return DeliveryOutcome(
        sent: false,
        approvalId: json['id'] as String?,
        approvalStatus: json['status'] as String?,
        expiresAt: (json['expires_at'] as num?)?.toInt(),
      );
    }
    return const DeliveryOutcome(sent: true);
  }
}

@immutable
class DownloadedAttachment {
  const DownloadedAttachment({
    required this.name,
    required this.contentType,
    required this.bytes,
  });

  final String name;
  final String contentType;
  final Uint8List bytes;
}
