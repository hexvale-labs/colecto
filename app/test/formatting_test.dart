import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:colecto/domain/models/mail_models.dart';
import 'package:colecto/ui/core/format/when.dart';
import 'package:colecto/ui/core/theme/colecto_palette.dart';
import 'package:colecto/ui/core/widgets/html_message_view.dart';
import 'package:colecto/ui/features/composer/compose_requests.dart';

void main() {
  group('Conversation grouping', () {
    test(
      'groups messages by thread chronologically and normalizes subject',
      () {
        final threads = groupConversations(const [
          MailMessage(
            id: '2',
            sender: 'Bob',
            address: 'bob@example.com',
            subject: 'Re: Fwd: Hello',
            body: 'Second',
            date: '2026-01-02T10:00:00Z',
            seen: false,
            thread: 't1',
          ),
          MailMessage(
            id: '1',
            sender: 'Alice',
            address: 'alice@example.com',
            subject: 'Hello',
            body: 'First',
            date: '2026-01-01T10:00:00Z',
            seen: true,
            thread: 't1',
          ),
        ]);

        expect(threads.length, 1);
        expect(threads.first.subject, 'Hello');
        expect(threads.first.messages.map((m) => m.id).toList(), ['1', '2']);
        expect(threads.first.unreadCount, 1);
        expect(threads.first.participants, ['Alice', 'Bob']);
      },
    );
  });

  group('Mail formatting', () {
    test('dates read like a mail client across time buckets', () {
      final now = DateTime(2026, 11, 21, 12);
      expect(when('2026-11-21T08:05:00', now: now).label, '08:05');
      expect(when('2026-11-20T08:05:00', now: now).group, 'Yesterday');
      expect(when('2026-11-17T08:05:00', now: now).label, 'Tue');
      expect(when('2026-03-02T08:05:00', now: now).label, 'Mar 2');
      expect(when('2025-03-02T08:05:00', now: now).label, 'Mar 2, 2025');
      expect(when('2026-03-02T08:05:00', now: now).group, 'March');
    });

    test('reply all omits self and sender, forward quotes original', () {
      const msg = MailMessage(
        id: '1',
        sender: 'Ana',
        address: 'ana@example.com',
        subject: 'Plan',
        body: 'Hi',
        date: '2026-11-20T08:00:00Z',
        seen: true,
        thread: 't',
      );
      const content = MessageContent(
        text: 'Hi',
        to: 'me@example.com, bo@example.com',
        cc: 'cy@example.com',
      );
      final reply = replyRequest(
        msg,
        content,
        all: true,
        self: 'me@example.com',
      );
      expect(reply.to, 'ana@example.com');
      expect(reply.cc, 'bo@example.com, cy@example.com');
      expect(reply.subject, 'Re: Plan');
      expect(reply.body, contains('> Hi'));

      final fwd = forwardRequest(msg, content);
      expect(fwd.to, isEmpty);
      expect(fwd.subject, 'Fwd: Plan');
      expect(fwd.forwardOf, '1');
      expect(fwd.body, contains('Hi'));
    });

    test('HTML helpers detect styled mail accurately', () {
      expect(
        HtmlMessageView.isDesigned('<table bgcolor="#ffffff">Hi</table>'),
        isTrue,
      );
      expect(HtmlMessageView.isDesigned('<p>Plain paragraph</p>'), isFalse);
    });
  });

  group('Design system resolution', () {
    test('COLECTO_DESIGN values and Material You scheme map accurately', () {
      expect(ColectoPlatformStyle.parse('fluent'), ColectoPlatformStyle.fluent);
      expect(
        ColectoPlatformStyle.parse('Windows'),
        ColectoPlatformStyle.fluent,
      );
      expect(ColectoPlatformStyle.parse('macos'), ColectoPlatformStyle.macos);
      expect(
        ColectoPlatformStyle.parse('colecto'),
        ColectoPlatformStyle.desktop,
      );
      expect(
        ColectoPlatformStyle.parse('material'),
        ColectoPlatformStyle.android,
      );
      expect(ColectoPlatformStyle.parse('cupertino'), ColectoPlatformStyle.ios);
      expect(ColectoPlatformStyle.parse('native'), isNull);

      final scheme = ColorScheme.fromSeed(seedColor: const Color(0xFF3366FF));
      final palette = ColectoPalette.fromScheme(scheme);
      expect(palette.accent, scheme.primary);
      expect(palette.bg, scheme.surface);
    });
  });
}
