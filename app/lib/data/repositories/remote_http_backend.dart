import 'dart:async';
import 'dart:convert';

import 'package:http/http.dart' as http;

import '../../domain/models/account_models.dart';
import '../../domain/models/ai_models.dart';
import '../../domain/models/automation_models.dart';
import '../../domain/models/mail_models.dart';
import '../../domain/models/preferences_models.dart';
import '../../domain/models/service_event.dart';
import 'mail_backend.dart';

/// HTTP + SSE implementation of [MailBackend] targeting `colecto-daemon`.
///
/// Used on Flutter Web (Wasm) and remote-daemon mode on desktop/mobile.
class RemoteHttpBackend implements MailBackend {
  RemoteHttpBackend({
    required String baseUrl,
    String token = '',
    http.Client? client,
  }) : _baseUrl = _normalizeBaseUrl(baseUrl),
       _token = token.trim(),
       _client = client ?? http.Client();

  final String _baseUrl;
  final String _token;
  final http.Client _client;
  final Set<http.Client> _activeStreamClients = {};
  bool _disposed = false;

  String get baseUrl => _baseUrl;
  String get token => _token;

  static String _normalizeBaseUrl(String value) {
    final trimmed = value.trim();
    if (trimmed.isEmpty) return '';
    return trimmed.endsWith('/')
        ? trimmed.substring(0, trimmed.length - 1)
        : trimmed;
  }

  @override
  String get backendLabel => _baseUrl.isEmpty
      ? 'Remote Daemon (same-origin)'
      : 'Remote Daemon ($_baseUrl)';

  Uri _uri(String path, [Map<String, String>? query]) {
    final base = '$_baseUrl/api/v1$path';
    final uri = Uri.parse(base);
    if (query == null || query.isEmpty) return uri;
    return uri.replace(queryParameters: query);
  }

  Map<String, String> _headers({String? account, bool jsonBody = false}) {
    return {
      if (_token.isNotEmpty) 'Authorization': 'Bearer $_token',
      if (account != null && account.isNotEmpty) 'x-colecto-account': account,
      if (jsonBody) 'content-type': 'application/json',
    };
  }

  Future<dynamic> _request(
    String method,
    String path, {
    String? account,
    Map<String, String>? query,
    Object? body,
    bool delivery = false,
  }) async {
    const deliveryWarning =
        'Delivery status is unknown. Check Sent and delivery requests before sending again.';
    final uri = _uri(path, query);
    final headers = _headers(account: account, jsonBody: body != null);
    final encoded = body != null ? jsonEncode(body) : null;

    http.Response response;
    try {
      switch (method) {
        case 'GET':
          response = await _client.get(uri, headers: headers);
        case 'POST':
          response = await _client.post(uri, headers: headers, body: encoded);
        case 'DELETE':
          response = await _client.delete(uri, headers: headers, body: encoded);
        default:
          throw MailBackendException('Unsupported HTTP method $method');
      }
    } catch (e) {
      if (e is MailBackendException) rethrow;
      throw MailBackendException(
        delivery
            ? deliveryWarning
            : 'Cannot reach Colecto daemon at $_baseUrl. Check that it is running.',
        category: delivery ? 'delivery_uncertain' : 'network',
      );
    }

    dynamic decoded;
    if (response.body.isNotEmpty) {
      try {
        decoded = jsonDecode(response.body);
      } catch (_) {
        decoded = null;
      }
    }

    if (response.statusCode < 200 || response.statusCode >= 300) {
      final errMsg =
          decoded is Map<String, dynamic> && decoded['error'] is String
          ? decoded['error'] as String
          : 'Mail daemon returned HTTP ${response.statusCode}.';
      final category =
          decoded is Map<String, dynamic> && decoded['category'] is String
          ? decoded['category'] as String
          : (response.statusCode == 401 ? 'authentication' : 'server');
      throw MailBackendException(
        errMsg,
        category: category,
        statusCode: response.statusCode,
      );
    }

    return decoded;
  }

  Map<String, dynamic> _asMap(dynamic data, String context) {
    if (data is Map<String, dynamic>) return data;
    throw MailBackendException(
      'Unexpected response for $context.',
      category: 'server',
    );
  }

  List<dynamic> _asList(dynamic data, String context) {
    if (data is List<dynamic>) return data;
    throw MailBackendException(
      'Unexpected response for $context.',
      category: 'server',
    );
  }

  @override
  Future<VersionInfo> version() async {
    final data = await _request('GET', '/version');
    return VersionInfo.fromJson(_asMap(data, 'version'));
  }

  @override
  Future<ReleaseStatus> latestRelease() async {
    final data = await _request('GET', '/releases/latest');
    return ReleaseStatus.fromJson(_asMap(data, 'latest release'));
  }

  @override
  Stream<ServiceEvent> events({bool window = true}) {
    late StreamController<ServiceEvent> controller;
    var cancelled = false;
    http.Client? liveClient;

    Future<void> pump() async {
      var backoffMs = 1000;
      while (!cancelled && !_disposed) {
        final streamClient = http.Client();
        liveClient = streamClient;
        _activeStreamClients.add(streamClient);
        var stopReconnecting = false;
        try {
          final req = http.Request(
            'GET',
            _uri('/events', window ? {'window': 'true'} : null),
          );
          req.headers.addAll(_headers());
          req.headers['Accept'] = 'text/event-stream';
          final response = await streamClient.send(req);
          if (response.statusCode == 401 || response.statusCode == 403) {
            stopReconnecting = true;
            throw MailBackendException(
              'SSE stream unauthorized (HTTP ${response.statusCode})',
              category: 'authentication',
              statusCode: response.statusCode,
            );
          }
          if (response.statusCode != 200) {
            throw MailBackendException(
              'SSE stream refused (HTTP ${response.statusCode})',
              statusCode: response.statusCode,
            );
          }
          backoffMs = 1000;
          var buffer = '';
          await for (final chunk in response.stream.transform(utf8.decoder)) {
            if (cancelled || _disposed) break;
            buffer += chunk;
            if (buffer.length > 262144) {
              throw const MailBackendException('SSE event buffer overflow');
            }
            while (true) {
              final idx = buffer.indexOf(RegExp(r'\r?\n\r?\n'));
              if (idx < 0) break;
              final block = buffer.substring(0, idx);
              buffer = buffer
                  .substring(idx)
                  .replaceFirst(RegExp(r'^\r?\n\r?\n'), '');
              final event = _parseSseBlock(block);
              if (event != null && !controller.isClosed) {
                controller.add(event);
              }
            }
          }
        } catch (_) {
          // Reconnect with exponential backoff unless auth failed.
        } finally {
          _activeStreamClients.remove(streamClient);
          if (identical(liveClient, streamClient)) liveClient = null;
          streamClient.close();
        }
        if (cancelled || _disposed || stopReconnecting) break;
        await Future<void>.delayed(Duration(milliseconds: backoffMs));
        backoffMs = (backoffMs * 2).clamp(1000, 30000);
      }
    }

    controller = StreamController<ServiceEvent>.broadcast(
      onListen: () {
        cancelled = false;
        unawaited(pump());
      },
      onCancel: () {
        cancelled = true;
        liveClient?.close();
        liveClient = null;
      },
    );
    return controller.stream;
  }

  ServiceEvent? _parseSseBlock(String block) {
    var name = 'message';
    final dataLines = <String>[];
    for (final line in block.split(RegExp(r'\r?\n'))) {
      if (line.startsWith('event:')) {
        name = line.substring(6).trim();
      } else if (line.startsWith('data:')) {
        dataLines.add(line.substring(5).trimLeft());
      }
    }
    if (dataLines.isEmpty && name == 'message') return null;
    Map<String, dynamic> payload = const {};
    try {
      final parsed = jsonDecode(dataLines.join('\n'));
      if (parsed is Map<String, dynamic>) payload = parsed;
    } catch (_) {
      return null;
    }
    return ServiceEvent.fromWire(name, payload);
  }

  @override
  Future<Preferences> getPreferences() async {
    final data = await _request('GET', '/preferences');
    return Preferences.fromJson(_asMap(data, 'preferences'));
  }

  @override
  Future<Preferences> updatePreferences(PreferencesPatch patch) async {
    final data = await _request('POST', '/preferences', body: patch.toJson());
    return Preferences.fromJson(_asMap(data, 'preferences'));
  }

  @override
  Future<List<AccountInfo>> listAccounts() async {
    final data = _asList(await _request('GET', '/accounts'), 'accounts');
    return data
        .map((e) => AccountInfo.fromJson(_asMap(e, 'account')))
        .toList(growable: false);
  }

  @override
  Future<AccountInfo> upsertAccount(AccountInput input) async {
    final data = await _request('POST', '/accounts', body: input.toJson());
    return AccountInfo.fromJson(_asMap(data, 'account'));
  }

  @override
  Future<DiscoveryResult?> discoverAccount(String address) async {
    final data = await _request(
      'POST',
      '/accounts/discover',
      body: {'address': address},
    );
    if (data == null) return null;
    return DiscoveryResult.fromJson(_asMap(data, 'discovery'));
  }

  @override
  Future<bool> removeAccount(String id) async {
    final data = await _request(
      'DELETE',
      '/accounts/${Uri.encodeComponent(id)}',
    );
    return data == true;
  }

  @override
  Future<bool> disconnectAccount(String id) async {
    final data = await _request(
      'POST',
      '/accounts/${Uri.encodeComponent(id)}/disconnect',
      body: const <String, dynamic>{},
    );
    return data == true;
  }

  @override
  Future<bool> accountConnected(String account) async {
    final data = await _request('GET', '/account', account: account);
    return data == true;
  }

  @override
  Future<SecurityInfo> security() async {
    final data = await _request('GET', '/security');
    return SecurityInfo.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<SecurityInfo> securityAction(
    String action, {
    String password = '',
  }) async {
    final data = await _request(
      'POST',
      '/security',
      body: {'action': action, 'password': password},
    );
    return SecurityInfo.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<List<AccountSync>> syncStatus() async {
    final data = _asList(await _request('GET', '/sync'), 'sync status');
    return data
        .map((e) => AccountSync.fromJson(_asMap(e, 'sync status')))
        .toList(growable: false);
  }

  @override
  Future<List<Folder>> folders(String account) async {
    final data = _asList(
      await _request('GET', '/folders', account: account),
      'folders',
    );
    return data
        .map((e) => Folder.fromJson(_asMap(e, 'folder')))
        .toList(growable: false);
  }

  @override
  Future<LabelMap> labels(String account) async {
    final data = _asMap(
      await _request('GET', '/labels', account: account),
      'labels',
    );
    return data.map(
      (k, v) => MapEntry(
        k,
        _asList(v, 'label $k').map((e) => e.toString()).toList(growable: false),
      ),
    );
  }

  @override
  Future<LabelMap> updateLabel(
    String account, {
    required String name,
    String? messageId,
    bool remove = false,
  }) async {
    final data = _asMap(
      await _request(
        'POST',
        '/labels',
        account: account,
        body: {'name': name, 'message_id': ?messageId, 'remove': remove},
      ),
      'labels',
    );
    return data.map(
      (k, v) => MapEntry(
        k,
        _asList(v, 'label $k').map((e) => e.toString()).toList(growable: false),
      ),
    );
  }

  @override
  Future<MailSnapshot> cachedMessages(
    String account, {
    String folder = 'inbox',
    String query = '',
    int offset = 0,
  }) async {
    final data = await _request(
      'GET',
      '/messages/cache',
      account: account,
      query: {
        'folder': folder,
        if (query.isNotEmpty) 'q': query,
        if (offset > 0) 'offset': '$offset',
      },
    );
    return MailSnapshot.fromJson(_asMap(data, 'cached messages'));
  }

  @override
  Future<List<MailMessage>> refreshMessages(
    String account, {
    String folder = 'inbox',
    String query = '',
    int offset = 0,
  }) async {
    final data = _asList(
      await _request(
        'POST',
        '/messages/refresh',
        account: account,
        query: {
          'folder': folder,
          if (query.isNotEmpty) 'q': query,
          if (offset > 0) 'offset': '$offset',
        },
        body: const <String, dynamic>{},
      ),
      'messages',
    );
    return data
        .map((e) => MailMessage.fromJson(_asMap(e, 'message')))
        .toList(growable: false);
  }

  @override
  Future<MailMessage> message(String account, String id) async {
    final data = await _request(
      'GET',
      '/messages/${Uri.encodeComponent(id)}',
      account: account,
    );
    return MailMessage.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<MessageContent> messageContent(
    String account,
    String id, {
    bool cached = false,
    bool? remoteImages,
  }) async {
    final data = await _request(
      'GET',
      '/messages/${Uri.encodeComponent(id)}/content',
      account: account,
      query: {
        if (cached) 'cached': 'true',
        if (remoteImages != null) 'remote_images': remoteImages.toString(),
      },
    );
    return MessageContent.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<DownloadedAttachment> downloadAttachment(
    String account,
    String messageId,
    int index,
  ) async {
    final uri = _uri(
      '/messages/${Uri.encodeComponent(messageId)}/attachments/$index',
    );
    final http.Response response;
    try {
      response = await _client.get(uri, headers: _headers(account: account));
    } catch (_) {
      throw MailBackendException(
        'Cannot reach Colecto daemon at $_baseUrl to download attachment.',
        category: 'network',
      );
    }
    if (response.statusCode < 200 || response.statusCode >= 300) {
      throw MailBackendException(
        'Attachment download failed (HTTP ${response.statusCode})',
        statusCode: response.statusCode,
      );
    }
    final disposition = response.headers['content-disposition'] ?? '';
    final name = _parseFilename(disposition, 'attachment-$index');
    return DownloadedAttachment(
      name: name,
      contentType:
          response.headers['content-type'] ?? 'application/octet-stream',
      bytes: response.bodyBytes,
    );
  }

  static String _parseFilename(String disposition, String fallback) {
    final ext = RegExp(
      r"filename\*=UTF-8''([^;]+)",
      caseSensitive: false,
    ).firstMatch(disposition);
    if (ext != null) {
      try {
        final decoded = Uri.decodeComponent(ext.group(1)!.trim());
        if (decoded.isNotEmpty) return decoded;
      } catch (_) {}
    }
    final basic = RegExp(
      r'filename\s*=\s*"([^"]+)"|filename\s*=\s*([^;\s]+)',
      caseSensitive: false,
    ).firstMatch(disposition);
    final raw = (basic?.group(1) ?? basic?.group(2))?.trim();
    return raw != null && raw.isNotEmpty ? raw : fallback;
  }

  @override
  Future<List<ThreadMessage>> conversation(String account, String id) async {
    final data = await _request(
      'GET',
      '/messages/${Uri.encodeComponent(id)}/conversation',
      account: account,
    ) as List<dynamic>;
    return data
        .map((e) => ThreadMessage.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  @override
  Future<void> organizeMessages(
    String account,
    String action, {
    required List<String> ids,
    bool? seen,
    bool? flagged,
    String? folder,
  }) async {
    await _request(
      'POST',
      '/messages/${Uri.encodeComponent(action)}',
      account: account,
      body: {'ids': ids, 'seen': ?seen, 'flagged': ?flagged, 'folder': ?folder},
    );
  }

  @override
  Future<List<Draft>> drafts(String account) async {
    final data =
        await _request('GET', '/drafts', account: account) as List<dynamic>;
    return data
        .map((e) => Draft.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  @override
  Future<Draft> draft(String account, String id) async {
    final data = await _request(
      'GET',
      '/drafts/${Uri.encodeComponent(id)}',
      account: account,
    );
    return Draft.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<Draft> saveDraft(String account, DraftInput input) async {
    final data = await _request(
      'POST',
      '/drafts',
      account: account,
      body: input.toJson(),
    );
    return Draft.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<DeliveryOutcome> send(String account, DraftInput input) async {
    final data = await _request(
      'POST',
      '/send',
      account: account,
      body: input.toJson(),
      delivery: true,
    );
    return DeliveryOutcome.fromJson(data);
  }

  @override
  Future<List<PendingSend>> listSendRequests() async {
    final data = await _request('GET', '/send-requests') as List<dynamic>;
    return data
        .map((e) => PendingSend.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  @override
  Future<PendingSend> approveSend(String id) async {
    final data = await _request(
      'POST',
      '/send-requests/${Uri.encodeComponent(id)}/approve',
      body: const <String, dynamic>{},
      delivery: true,
    );
    return PendingSend.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<PendingSend> rejectSend(String id) async {
    final data = await _request(
      'POST',
      '/send-requests/${Uri.encodeComponent(id)}/reject',
      body: const <String, dynamic>{},
    );
    return PendingSend.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<List<ApiKeyMetadata>> listApiKeys() async {
    final data = await _request('GET', '/api-keys') as List<dynamic>;
    return data
        .map((e) => ApiKeyMetadata.fromJson(e as Map<String, dynamic>))
        .toList(growable: false);
  }

  @override
  Future<MintedApiKey> mintApiKey(ApiKeyInput input) async {
    final data = await _request('POST', '/api-keys', body: input.toJson());
    return MintedApiKey.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<bool> revokeApiKey(String id) async {
    final data = await _request(
      'DELETE',
      '/api-keys/${Uri.encodeComponent(id)}',
    );
    return data == true;
  }

  @override
  Future<AiSettingsView> aiSettings({String? account}) async {
    final data = await _request('GET', '/ai/settings', account: account);
    return AiSettingsView.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<AiSettingsView> aiUpdateSettings(
    AiSettings settings, {
    String? account,
  }) async {
    final data = await _request(
      'POST',
      '/ai/settings',
      account: account,
      body: settings.toJson(),
    );
    return AiSettingsView.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<AiSettingsView> aiPutKey({required String kind, String? key}) async {
    final data = await _request(
      'POST',
      '/ai/keys',
      body: {'kind': kind, 'key': key},
    );
    return AiSettingsView.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<bool> aiTestConnection({required String kind, String? account}) async {
    final data = await _request(
      'POST',
      '/ai/test',
      account: account,
      body: {'kind': kind},
    ) as Map<String, dynamic>;
    return data['ok'] == true;
  }

  @override
  Future<PreviewResult> aiPreview(String account, LabelRule rule) async {
    final data = await _request(
      'POST',
      '/ai/labels/preview',
      account: account,
      body: {'rule': rule.toJson()},
    );
    return PreviewResult.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<ReclassifiedResult> aiReclassify(
    String account,
    List<String> ids,
  ) async {
    final data = await _request(
      'POST',
      '/ai/reclassify',
      account: account,
      body: {'ids': ids},
    );
    return ReclassifiedResult.fromJson(data as Map<String, dynamic>);
  }

  @override
  Future<String?> aiGetSummary(String account, String messageId) async {
    final data = await _request(
      'GET',
      '/ai/summaries/${Uri.encodeComponent(messageId)}',
      account: account,
    ) as Map<String, dynamic>;
    return data['summary'] as String?;
  }

  @override
  Future<String> aiCreateSummary(String account, String messageId) async {
    final data = await _request(
      'POST',
      '/ai/summaries/${Uri.encodeComponent(messageId)}',
      account: account,
      body: const <String, dynamic>{},
    ) as Map<String, dynamic>;
    return data['summary'] as String? ?? '';
  }

  @override
  Future<void> aiDeleteSummary(String account, String messageId) async {
    await _request(
      'DELETE',
      '/ai/summaries/${Uri.encodeComponent(messageId)}',
      account: account,
    );
  }

  @override
  Future<String> aiDraftReply(
    String account, {
    required String messageId,
    required String input,
    required String tone,
  }) async {
    final data = await _request(
      'POST',
      '/ai/reply',
      account: account,
      body: {'message_id': messageId, 'input': input, 'tone': tone},
    ) as Map<String, dynamic>;
    return data['text'] as String? ?? '';
  }

  @override
  void dispose() {
    _disposed = true;
    for (final c in _activeStreamClients.toList()) {
      c.close();
    }
    _activeStreamClients.clear();
    _client.close();
  }
}
