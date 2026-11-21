import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:colecto/data/repositories/backend_config.dart';
import 'package:colecto/data/repositories/remote_http_backend.dart';
import 'package:colecto/domain/models/mail_models.dart';
import 'package:colecto/domain/models/service_event.dart';
import 'package:colecto/ui/features/shell/mail_controller.dart';

void main() {
  group('Backend bootstrap config', () {
    test('resolves Web, explicit demo, and native daemon defaults', () {
      final webDefault = resolveBackendConfig(
        envBackend: '',
        envDaemonUrl: '',
        envToken: '',
        isWeb: true,
        baseUri: Uri.parse('http://127.0.0.1:4173/'),
        storedToken: 'saved-tab-token',
      );
      expect(webDefault.kind, BackendKind.remoteHttp);
      expect(webDefault.daemonUrl, 'http://127.0.0.1:4173');
      expect(webDefault.token, 'saved-tab-token');

      final webDemo = resolveBackendConfig(
        envBackend: 'demo',
        envDaemonUrl: '',
        envToken: '',
        isWeb: true,
        baseUri: Uri.parse('http://127.0.0.1:4173/'),
        storedToken: '',
      );
      expect(webDemo.kind, BackendKind.demo);

      final nativeDemo = resolveBackendConfig(
        envBackend: '',
        envDaemonUrl: '',
        envToken: '',
        isWeb: false,
        baseUri: Uri.file('/tmp/app'),
        storedToken: '',
      );
      expect(nativeDemo.kind, BackendKind.demo);

      final nativeRemote = resolveBackendConfig(
        envBackend: '',
        envDaemonUrl: 'http://127.0.0.1:4173',
        envToken: 'tok',
        isWeb: false,
        baseUri: Uri.file('/tmp/app'),
        storedToken: '',
      );
      expect(nativeRemote.kind, BackendKind.remoteHttp);
      expect(nativeRemote.daemonUrl, 'http://127.0.0.1:4173');
      expect(nativeRemote.token, 'tok');

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

  group('ServiceEvent.fromWire', () {
    test('decodes every supported event type consistently', () {
      expect(
        ServiceEvent.fromWire('ready', const {}),
        isA<ReadyServiceEvent>(),
      );
      expect(
        ServiceEvent.fromWire('resync', const {}),
        isA<ResyncServiceEvent>(),
      );

      final mail = ServiceEvent.fromWire('mail', {
        'account': 'work',
        'ids': ['m1', 'm2'],
      }) as MailServiceEvent;
      expect(mail.account, 'work');
      expect(mail.ids, ['m1', 'm2']);

      final sync = ServiceEvent.fromWire('sync', {
        'account': 'work',
        'state': 'idle',
        'phase': 'done',
      }) as SyncServiceEvent;
      expect(sync.sync.account, 'work');

      final approval = ServiceEvent.fromWire('approval', {
        'id': 'req-1',
        'account': 'work',
        'from': 'bot@example.test',
        'subject': 'Quarterly update',
      }) as ApprovalServiceEvent;
      expect(approval.id, 'req-1');
      expect(approval.subject, 'Quarterly update');

      final nav = ServiceEvent.fromWire('navigate', {
        'target': 'settings/ai',
        'account': 'work',
      }) as NavigateServiceEvent;
      expect(nav.target, 'settings/ai');
      expect(nav.account, 'work');

      final mcp = ServiceEvent.fromWire('mcp', {'on': true}) as McpServiceEvent;
      expect(mcp.on, isTrue);

      expect(
        ServiceEvent.fromWire('preferences', const {}),
        isA<PreferencesServiceEvent>(),
      );

      final labels = ServiceEvent.fromWire('labels', {
        'account': 'demo',
      }) as LabelsUpdatedServiceEvent;
      expect(labels.account, 'demo');

      expect(ServiceEvent.fromWire('unknown_event', const {}), isNull);
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
        baseUrl: 'http://127.0.0.1:4173/',
        token: 'secret-token',
        client: client,
      );
      addTearDown(backend.dispose);

      final folders = await backend.folders('work');
      expect(captured.url.toString(), 'http://127.0.0.1:4173/api/v1/folders');
      expect(captured.headers['Authorization'], 'Bearer secret-token');
      expect(captured.headers['x-colecto-account'], 'work');
      expect(folders.single.name, 'Inbox');
      expect(folders.single.unread, 2);
    });

    test('maps HTTP 401 to isAuthRequired and send network drop to delivery_uncertain', () async {
      final authClient = MockClient((_) async {
        return http.Response(
          jsonEncode({'error': 'Invalid token', 'category': 'authentication'}),
          401,
        );
      });
      final authBackend = RemoteHttpBackend(
        baseUrl: 'http://127.0.0.1:4173',
        token: 'bad',
        client: authClient,
      );
      addTearDown(authBackend.dispose);

      await expectLater(
        authBackend.listAccounts(),
        throwsA(
          isA<MailBackendException>()
              .having((e) => e.isAuthRequired, 'isAuthRequired', isTrue)
              .having((e) => e.statusCode, 'statusCode', 401),
        ),
      );

      final flakyClient = MockClient((_) async {
        throw const SocketException('Connection reset by peer');
      });
      final sendBackend = RemoteHttpBackend(
        baseUrl: 'http://127.0.0.1:4173',
        token: 'tok',
        client: flakyClient,
      );
      addTearDown(sendBackend.dispose);

      await expectLater(
        sendBackend.send(
          'work',
          const DraftInput(
            recipient: 'ana@example.com',
            subject: 'Hello',
            body: 'Body',
          ),
        ),
        throwsA(
          isA<MailBackendException>().having(
            (e) => e.category,
            'category',
            'delivery_uncertain',
          ),
        ),
      );
    });

    test('parses live SSE frames over HTTP into typed ServiceEvents', () async {
      final prevOverrides = HttpOverrides.current;
      HttpOverrides.global = null;
      addTearDown(() => HttpOverrides.global = prevOverrides);

      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      addTearDown(() => server.close(force: true));

      server.listen((req) async {
        expect(req.uri.path, '/api/v1/events');
        expect(req.headers.value('authorization'), 'Bearer sse-tok');
        req.response.statusCode = HttpStatus.ok;
        req.response.bufferOutput = false;
        req.response.headers.contentType = ContentType('text', 'event-stream');
        req.response.write('event: ready\ndata: {}\n\n');
        req.response.write(
          'event: mail\ndata: {"account":"demo","ids":["m1"]}\n\n',
        );
        req.response.write('event: unknown\ndata: {"ignored":true}\n\n');
        req.response.write('event: labels\ndata: {"account":"demo"}\n\n');
        await req.response.flush();
        await req.response.close();
      });

      final backend = RemoteHttpBackend(
        baseUrl: 'http://127.0.0.1:${server.port}',
        token: 'sse-tok',
      );
      addTearDown(backend.dispose);

      final received = await backend
          .events()
          .take(3)
          .toList()
          .timeout(const Duration(seconds: 5));

      expect(received[0], isA<ReadyServiceEvent>());
      expect(
        received[1],
        isA<MailServiceEvent>()
            .having((e) => e.account, 'account', 'demo')
            .having((e) => e.ids, 'ids', ['m1']),
      );
      expect(
        received[2],
        isA<LabelsUpdatedServiceEvent>().having(
          (e) => e.account,
          'account',
          'demo',
        ),
      );
    });
  });
}
