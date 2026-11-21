import 'dart:convert';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:colecto/app.dart';
import 'package:colecto/data/repositories/backend_config.dart';
import 'package:colecto/data/repositories/in_memory_mail_backend.dart';
import 'package:colecto/data/repositories/remote_http_backend.dart';
import 'package:colecto/domain/models/automation_models.dart';
import 'package:colecto/domain/models/mail_models.dart';
import 'package:colecto/ui/core/format/when.dart';
import 'package:colecto/ui/core/theme/colecto_palette.dart';
import 'package:colecto/ui/features/composer/compose_requests.dart';
import 'package:colecto/ui/features/shell/mail_controller.dart';

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

  group('Backend bootstrap config', () {
    test('defaults to same-origin remote daemon on Web', () {
      final config = resolveBackendConfig(
        envBackend: '',
        envDaemonUrl: '',
        envToken: '',
        isWeb: true,
        baseUri: Uri.parse('http://127.0.0.1:4173/'),
        storedToken: 'saved-tab-token',
      );
      expect(config.kind, BackendKind.remoteHttp);
      expect(config.daemonUrl, 'http://127.0.0.1:4173');
      expect(config.token, 'saved-tab-token');
    });

    test('honors explicit COLECTO_BACKEND=demo on Web', () {
      final config = resolveBackendConfig(
        envBackend: 'demo',
        envDaemonUrl: '',
        envToken: '',
        isWeb: true,
        baseUri: Uri.parse('http://127.0.0.1:4173/'),
        storedToken: '',
      );
      expect(config.kind, BackendKind.demo);
    });

    test('defaults to demo on native unless daemon URL is provided', () {
      final demoConfig = resolveBackendConfig(
        envBackend: '',
        envDaemonUrl: '',
        envToken: '',
        isWeb: false,
        baseUri: Uri.file('/tmp/app'),
        storedToken: '',
      );
      expect(demoConfig.kind, BackendKind.demo);

      final remoteConfig = resolveBackendConfig(
        envBackend: '',
        envDaemonUrl: 'http://127.0.0.1:4173',
        envToken: 'tok',
        isWeb: false,
        baseUri: Uri.file('/tmp/app'),
        storedToken: '',
      );
      expect(remoteConfig.kind, BackendKind.remoteHttp);
      expect(remoteConfig.daemonUrl, 'http://127.0.0.1:4173');
      expect(remoteConfig.token, 'tok');
    });

    test('MailController constructs RemoteHttpBackend for initialKind', () {
      final controller = MailController(
        initialKind: BackendKind.remoteHttp,
        initialDaemonUrl: 'http://127.0.0.1:4173',
        initialToken: 'tok',
      );
      addTearDown(controller.dispose);
      expect(controller.backend, isA<RemoteHttpBackend>());
      expect(controller.daemonUrl, 'http://127.0.0.1:4173');
      expect(controller.daemonToken, 'tok');
    });
  });

  group('RemoteHttpBackend wire protocol', () {
    test('sends Authorization bearer and x-colecto-account headers', () async {
      late http.Request captured;
      final client = MockClient((request) async {
        captured = request;
        return http.Response(
          jsonEncode([
            {
              'id': 'inbox',
              'name': 'Inbox',
              'role': 'inbox',
              'unread': 2,
              'total': 10,
            },
          ]),
          200,
          headers: {'content-type': 'application/json'},
        );
      });

      final backend = RemoteHttpBackend(
        baseUrl: 'http://127.0.0.1:4173',
        token: 'secret-token',
        client: client,
      );

      final folders = await backend.folders('work');
      expect(captured.url.toString(), 'http://127.0.0.1:4173/api/v1/folders');
      expect(captured.headers['Authorization'], 'Bearer secret-token');
      expect(captured.headers['x-colecto-account'], 'work');
      expect(folders.single.name, 'Inbox');
      expect(folders.single.unread, 2);
      backend.dispose();
    });
  });

  group('Design system', () {
    test('COLECTO_DESIGN values resolve to designs', () {
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
    });

    test('Material You palette takes its accent from the system scheme', () {
      final scheme = ColorScheme.fromSeed(seedColor: const Color(0xFF3366FF));
      final palette = ColectoPalette.fromScheme(scheme);
      expect(palette.accent, scheme.primary);
      expect(palette.bg, scheme.surface);
    });
  });

  group('Mail formatting', () {
    test('dates read like a mail client', () {
      final now = DateTime(2026, 11, 21, 12);
      expect(when('2026-11-21T08:05:00', now: now).label, '08:05');
      expect(when('2026-11-20T08:05:00', now: now).group, 'Yesterday');
      expect(when('2026-11-17T08:05:00', now: now).label, 'Tue');
      expect(when('2026-03-02T08:05:00', now: now).label, 'Mar 2');
      expect(when('2025-03-02T08:05:00', now: now).label, 'Mar 2, 2025');
      expect(when('2026-03-02T08:05:00', now: now).group, 'March');
    });

    test('reply all leaves out yourself and the sender', () {
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
      final r = replyRequest(msg, content, all: true, self: 'me@example.com');
      expect(r.to, 'ana@example.com');
      expect(r.cc, 'bo@example.com, cy@example.com');
      expect(r.subject, 'Re: Plan');
      expect(r.body, contains('> Hi'));
    });
  });

  group('ColectoApp widget tests', () {
    Future<MailController> pumpApp(
      WidgetTester tester,
      Size size,
      ColectoPlatformStyle design,
    ) async {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.reset);
      final controller = MailController()..previewDesign(design);
      await controller.initialize();
      addTearDown(controller.dispose);
      await tester.pumpWidget(ColectoApp(controller: controller));
      await tester.pumpAndSettle();
      return controller;
    }

    testWidgets('desktop layout reads mail and approves a send', (
      tester,
    ) async {
      final c = await pumpApp(
        tester,
        const Size(1280, 800),
        ColectoPlatformStyle.desktop,
      );

      expect(find.text('Colecto'), findsOneWidget);
      expect(find.text('Welcome to Colecto'), findsOneWidget);
      expect(find.text('Welcome to Colecto.'), findsOneWidget);

      await tester.tap(find.text('Welcome to Colecto'));
      await tester.pumpAndSettle();
      expect(find.textContaining('remote images blocked'), findsOneWidget);
      expect(find.text('Reply'), findsOneWidget);

      await tester.tap(find.text('Send approvals'));
      await tester.pumpAndSettle();
      expect(find.text('Confirming Friday architecture sync'), findsOneWidget);

      await tester.tap(find.text('Approve and send'));
      await tester.pumpAndSettle();
      expect(find.text('Approve and send'), findsNothing);
      expect(c.sendRequests.first.status, ApprovalStatus.sent);
    });

    testWidgets('composer opens in the reader and keeps writing as a draft', (
      tester,
    ) async {
      final c = await pumpApp(
        tester,
        const Size(1280, 800),
        ColectoPlatformStyle.desktop,
      );
      c.startCompose();
      await tester.pumpAndSettle();
      expect(find.text('New message'), findsOneWidget);

      final to = tester.widget<TextField>(
        find.byWidgetPredicate(
          (w) => w is TextField && w.decoration?.hintText == 'name@example.com',
        ),
      );
      to.controller!.text = 'ana@example.com';
      await tester.pump();
      expect(find.text('Unsaved changes'), findsOneWidget);

      c.closeCompose();
      await tester.pumpAndSettle();
      expect(c.drafts.any((d) => d.recipient == 'ana@example.com'), isTrue);
    });

    // Any design must work on any platform and at any window size.
    for (final design in ColectoPlatformStyle.values) {
      for (final size in const [
        Size(1280, 800),
        Size(820, 700),
        Size(390, 844),
      ]) {
        testWidgets('renders ${design.id} at ${size.width.toInt()} px', (
          tester,
        ) async {
          await pumpApp(tester, size, design);
          expect(tester.takeException(), isNull);
          expect(find.text('Welcome to Colecto'), findsOneWidget);
        });
      }
    }

    // Worst realistic data: long names, unbreakable addresses, empty
    // subjects, non-Latin text, emoji, 999+ counts, long label names.
    for (final design in ColectoPlatformStyle.values) {
      for (final size in const [
        Size(1280, 800),
        Size(820, 700),
        Size(360, 740),
      ]) {
        testWidgets('survives worst-case data: ${design.id} at '
            '${size.width.toInt()} px', (tester) async {
          tester.view.physicalSize = size;
          tester.view.devicePixelRatio = 1.0;
          addTearDown(tester.view.reset);
          final c = MailController(
            backend: InMemoryMailBackend(worstCase: true),
          )..previewDesign(design);
          await c.initialize();
          addTearDown(c.dispose);
          await tester.pumpWidget(ColectoApp(controller: c));
          await tester.pumpAndSettle();

          // The longest message: attachments, recipients, long subject.
          await c.selectThread(c.threads.first);
          await tester.pumpAndSettle();
          // A message with no sender name and no subject.
          await c.selectThread(c.threads[1]);
          await tester.pumpAndSettle();
          // The 27-message conversation.
          await c.selectThread(
            c.threads.firstWhere((t) => t.messages.length > 20),
          );
          await tester.pumpAndSettle();

          c.startCompose();
          await tester.pumpAndSettle();
          c.closeCompose();
          await tester.pumpAndSettle();

          for (final section in SettingsSection.values) {
            c.openSettings(section);
            await tester.pumpAndSettle();
          }
          c.closeSettings();
          await tester.pumpAndSettle();
          c.clearSelection();
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
        });
      }
    }

    testWidgets('phones get a native list with a bottom folder bar', (
      tester,
    ) async {
      await pumpApp(tester, const Size(400, 820), ColectoPlatformStyle.android);
      expect(find.text('Welcome to Colecto'), findsOneWidget);
      expect(find.byType(NavigationBar), findsOneWidget);
      expect(find.text('Pick a message and it will open here.'), findsNothing);
    });
  });
}
