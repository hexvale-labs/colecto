import '../../../domain/models/mail_models.dart';
import '../../core/format/when.dart';
import '../shell/mail_controller.dart';

/// Builds reply, reply-all and forward drafts from a message, so the reader
/// toolbar, keyboard shortcuts and the command palette agree.
ComposeRequest replyRequest(
  MailMessage msg,
  MessageContent? content, {
  required bool all,
  String self = '',
  String body = '',
}) {
  final to = content?.replyTo.isNotEmpty == true
      ? content!.replyTo
      : msg.address;
  final subject = RegExp(r'^\s*re:', caseSensitive: false).hasMatch(msg.subject)
      ? msg.subject
      : 'Re: ${msg.subject}';
  final refs = [
    if (content?.references.isNotEmpty == true) content!.references,
    if (content?.messageId.isNotEmpty == true) content!.messageId,
  ].join(' ').trim();
  var cc = '';
  if (all) {
    final me = self.trim().toLowerCase();
    final skip = {me, to.trim().toLowerCase()};
    cc = [...?content?.to.split(','), ...?content?.cc.split(',')]
        .map((a) => a.trim())
        .where((a) {
          if (a.isEmpty) return false;
          final lower = a.toLowerCase();
          return !skip.any((s) => s.isNotEmpty && lower.contains(s));
        })
        .toSet()
        .join(', ');
  }
  final quoted = _quote(msg, content);
  return ComposeRequest(
    heading: 'Reply',
    to: to,
    cc: cc,
    subject: subject,
    body: body.isEmpty ? '\n\n$quoted' : '$body\n\n$quoted',
    inReplyTo: content?.messageId ?? '',
    references: refs,
    replyMessageId: msg.id,
  );
}

ComposeRequest forwardRequest(MailMessage msg, MessageContent? content) {
  final subject =
      RegExp(r'^\s*fwd?:', caseSensitive: false).hasMatch(msg.subject)
      ? msg.subject
      : 'Fwd: ${msg.subject}';
  final body =
      '\n\n---------- Forwarded message ----------\n'
      'From: ${msg.displaySender} <${msg.address}>\n'
      'Date: ${when(msg.date).full}\n'
      'Subject: ${msg.subject}\n'
      '${content?.to.isNotEmpty == true ? 'To: ${content!.to}\n' : ''}'
      '\n${content?.text.isNotEmpty == true ? content!.text : msg.body}';
  return ComposeRequest(
    heading: 'Forward',
    subject: subject,
    body: body,
    forwardOf: msg.id,
  );
}

String _quote(MailMessage msg, MessageContent? content) {
  final text = content?.text.isNotEmpty == true ? content!.text : msg.body;
  final lines = text.trimRight().split('\n').map((l) => '> $l').join('\n');
  return 'On ${when(msg.date).full}, ${msg.displaySender} wrote:\n$lines';
}
