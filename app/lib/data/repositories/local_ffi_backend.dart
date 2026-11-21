import 'dart:convert';
import 'dart:typed_data';

import 'package:http/http.dart' as http;

import '../../domain/models/account_models.dart';
import '../../domain/models/ai_models.dart';
import '../../domain/models/automation_models.dart';
import '../../domain/models/mail_models.dart';
import '../../domain/models/preferences_models.dart';
import '../../domain/models/service_event.dart';
import 'local_ffi_bridge_stub.dart'
    if (dart.library.ffi) 'local_ffi_bridge_native.dart';
import 'mail_backend.dart';
import 'remote_http_backend.dart';

/// Custom in-process [http.BaseClient] that dispatches requests directly over
/// `dart:ffi` to `libcolecto_ffi` with **zero loopback TCP sockets**.
class _InProcessFfiClient extends http.BaseClient {
  _InProcessFfiClient(this._bridge);

  final NativeFfiBridge _bridge;

  @override
  Future<http.StreamedResponse> send(http.BaseRequest request) async {
    final bytes = await request.finalize().toBytes();
    final bodyText = bytes.isEmpty ? null : utf8.decode(bytes);
    final pathAndQuery = request.url.hasQuery
        ? '${request.url.path}?${request.url.query}'
        : request.url.path;

    try {
      final resp = await _bridge.request(
        method: request.method,
        path: pathAndQuery,
        account: request.headers['x-colecto-account'],
        body: bodyText,
      );
      final respBytes = resp.isBase64
          ? base64Decode(resp.body)
          : Uint8List.fromList(utf8.encode(resp.body));
      final headers = <String, String>{
        'content-type': resp.contentType,
        if (resp.contentDisposition != null)
          'content-disposition': resp.contentDisposition!,
      };
      return http.StreamedResponse(
        Stream<List<int>>.value(respBytes),
        resp.status,
        headers: headers,
        contentLength: respBytes.length,
        request: request,
      );
    } catch (e) {
      throw MailBackendException(e.toString(), category: 'storage_or_service');
    }
  }
}

/// In-process FFI backend adapter (`dart:ffi` -> `crates/colecto-ffi`).
///
/// Desktop and mobile standalone builds talk directly to `MailService` in Rust
/// without opening a loopback HTTP port.
class LocalFfiBackend implements MailBackend {
  LocalFfiBackend({this.dataDir, this.libraryPath})
    : _bridge = NativeFfiBridge(dataDir: dataDir, libraryPath: libraryPath) {
    _delegate = RemoteHttpBackend(
      baseUrl: 'http://colecto.local',
      client: _InProcessFfiClient(_bridge),
    );
  }

  final String? dataDir;
  final String? libraryPath;
  final NativeFfiBridge _bridge;
  late final RemoteHttpBackend _delegate;

  @override
  String get backendLabel => 'Local FFI (in-process Rust)';

  @override
  Future<VersionInfo> version() => _delegate.version();

  @override
  Future<ReleaseStatus> latestRelease() => _delegate.latestRelease();

  @override
  Stream<ServiceEvent> events({bool window = true}) => _bridge.events();

  @override
  Future<Preferences> getPreferences() => _delegate.getPreferences();

  @override
  Future<Preferences> updatePreferences(PreferencesPatch patch) =>
      _delegate.updatePreferences(patch);

  @override
  Future<List<AccountInfo>> listAccounts() => _delegate.listAccounts();

  @override
  Future<AccountInfo> upsertAccount(AccountInput input) =>
      _delegate.upsertAccount(input);

  @override
  Future<DiscoveryResult?> discoverAccount(String address) =>
      _delegate.discoverAccount(address);

  @override
  Future<bool> removeAccount(String id) => _delegate.removeAccount(id);

  @override
  Future<bool> disconnectAccount(String id) => _delegate.disconnectAccount(id);

  @override
  Future<bool> accountConnected(String account) =>
      _delegate.accountConnected(account);

  @override
  Future<SecurityInfo> security() => _delegate.security();

  @override
  Future<SecurityInfo> securityAction(String action, {String password = ''}) =>
      _delegate.securityAction(action, password: password);

  @override
  Future<List<AccountSync>> syncStatus() => _delegate.syncStatus();

  @override
  Future<List<Folder>> folders(String account) => _delegate.folders(account);

  @override
  Future<LabelMap> labels(String account) => _delegate.labels(account);

  @override
  Future<LabelMap> updateLabel(
    String account, {
    required String name,
    String? messageId,
    bool remove = false,
  }) => _delegate.updateLabel(
    account,
    name: name,
    messageId: messageId,
    remove: remove,
  );

  @override
  Future<MailSnapshot> cachedMessages(
    String account, {
    String folder = 'inbox',
    String query = '',
    int offset = 0,
    int? limit,
  }) => _delegate.cachedMessages(
    account,
    folder: folder,
    query: query,
    offset: offset,
    limit: limit,
  );

  @override
  Future<List<MailMessage>> refreshMessages(
    String account, {
    String folder = 'inbox',
    String query = '',
    int offset = 0,
    int? limit,
  }) => _delegate.refreshMessages(
    account,
    folder: folder,
    query: query,
    offset: offset,
    limit: limit,
  );

  @override
  Future<MailMessage> message(String account, String id) =>
      _delegate.message(account, id);

  @override
  Future<MessageContent> messageContent(
    String account,
    String id, {
    bool cached = false,
    bool? remoteImages,
  }) => _delegate.messageContent(
    account,
    id,
    cached: cached,
    remoteImages: remoteImages,
  );

  @override
  Future<DownloadedAttachment> downloadAttachment(
    String account,
    String messageId,
    int index,
  ) => _delegate.downloadAttachment(account, messageId, index);

  @override
  Future<List<ThreadMessage>> conversation(String account, String id) =>
      _delegate.conversation(account, id);

  @override
  Future<void> organizeMessages(
    String account,
    String action, {
    required List<String> ids,
    bool? seen,
    bool? flagged,
    String? folder,
  }) => _delegate.organizeMessages(
    account,
    action,
    ids: ids,
    seen: seen,
    flagged: flagged,
    folder: folder,
  );

  @override
  Future<List<Draft>> drafts(String account) => _delegate.drafts(account);

  @override
  Future<Draft> draft(String account, String id) =>
      _delegate.draft(account, id);

  @override
  Future<Draft> saveDraft(String account, DraftInput input) =>
      _delegate.saveDraft(account, input);

  @override
  Future<DeliveryOutcome> send(String account, DraftInput input) =>
      _delegate.send(account, input);

  @override
  Future<List<PendingSend>> listSendRequests() => _delegate.listSendRequests();

  @override
  Future<PendingSend> approveSend(String id) => _delegate.approveSend(id);

  @override
  Future<PendingSend> rejectSend(String id) => _delegate.rejectSend(id);

  @override
  Future<List<ApiKeyMetadata>> listApiKeys() => _delegate.listApiKeys();

  @override
  Future<MintedApiKey> mintApiKey(ApiKeyInput input) =>
      _delegate.mintApiKey(input);

  @override
  Future<bool> revokeApiKey(String id) => _delegate.revokeApiKey(id);

  @override
  Future<AiSettingsView> aiSettings({String? account}) =>
      _delegate.aiSettings(account: account);

  @override
  Future<AiSettingsView> aiUpdateSettings(
    AiSettings settings, {
    String? account,
  }) => _delegate.aiUpdateSettings(settings, account: account);

  @override
  Future<AiSettingsView> aiPutKey({required String kind, String? key}) =>
      _delegate.aiPutKey(kind: kind, key: key);

  @override
  Future<bool> aiTestConnection({required String kind, String? account}) =>
      _delegate.aiTestConnection(kind: kind, account: account);

  @override
  Future<PreviewResult> aiPreview(String account, LabelRule rule) =>
      _delegate.aiPreview(account, rule);

  @override
  Future<ReclassifiedResult> aiReclassify(String account, List<String> ids) =>
      _delegate.aiReclassify(account, ids);

  @override
  Future<String?> aiGetSummary(String account, String messageId) =>
      _delegate.aiGetSummary(account, messageId);

  @override
  Future<String> aiCreateSummary(String account, String messageId) =>
      _delegate.aiCreateSummary(account, messageId);

  @override
  Future<void> aiDeleteSummary(String account, String messageId) =>
      _delegate.aiDeleteSummary(account, messageId);

  @override
  Future<String> aiDraftReply(
    String account, {
    required String messageId,
    required String input,
    required String tone,
  }) => _delegate.aiDraftReply(
    account,
    messageId: messageId,
    input: input,
    tone: tone,
  );

  @override
  void dispose() {
    _delegate.dispose();
    _bridge.dispose();
  }
}
