import '../../domain/models/account_models.dart';
import '../../domain/models/ai_models.dart';
import '../../domain/models/automation_models.dart';
import '../../domain/models/mail_models.dart';
import '../../domain/models/preferences_models.dart';
import '../../domain/models/service_event.dart';

typedef LabelMap = Map<String, List<String>>;

/// Single backend contract for all Flutter UI code.
///
/// Production implementations:
/// - `LocalFfiBackend` (`flutter_rust_bridge`, in-process on desktop & mobile)
/// - `RemoteHttpBackend` (HTTP + SSE to `colecto-daemon` on Web & remote mode)
///
/// `InMemoryMailBackend` provides a zero-dependency demo & test backend.
abstract class MailBackend {
  String get backendLabel;

  // System & metadata
  Future<VersionInfo> version();
  Future<ReleaseStatus> latestRelease();
  Stream<ServiceEvent> events({bool window = true});

  // Preferences
  Future<Preferences> getPreferences();
  Future<Preferences> updatePreferences(PreferencesPatch patch);

  // Accounts, discovery, security, sync
  Future<List<AccountInfo>> listAccounts();
  Future<AccountInfo> upsertAccount(AccountInput input);
  Future<DiscoveryResult?> discoverAccount(String address);
  Future<bool> removeAccount(String id);
  Future<bool> disconnectAccount(String id);
  Future<bool> accountConnected(String account);
  Future<SecurityInfo> security();
  Future<SecurityInfo> securityAction(String action, {String password = ''});
  Future<List<AccountSync>> syncStatus();

  // Folders & labels
  Future<List<Folder>> folders(String account);
  Future<LabelMap> labels(String account);
  Future<LabelMap> updateLabel(
    String account, {
    required String name,
    String? messageId,
    bool remove = false,
  });

  // Messages, conversation, content, attachments, organize
  Future<MailSnapshot> cachedMessages(
    String account, {
    String folder = 'inbox',
    String query = '',
    int offset = 0,
  });
  Future<List<MailMessage>> refreshMessages(
    String account, {
    String folder = 'inbox',
    String query = '',
    int offset = 0,
  });
  Future<MailMessage> message(String account, String id);
  Future<MessageContent> messageContent(
    String account,
    String id, {
    bool cached = false,
    bool? remoteImages,
  });
  Future<DownloadedAttachment> downloadAttachment(
    String account,
    String messageId,
    int index,
  );
  Future<List<ThreadMessage>> conversation(String account, String id);
  Future<void> organizeMessages(
    String account,
    String action, {
    required List<String> ids,
    bool? seen,
    bool? flagged,
    String? folder,
  });

  // Drafts & send
  Future<List<Draft>> drafts(String account);
  Future<Draft> draft(String account, String id);
  Future<Draft> saveDraft(String account, DraftInput input);
  Future<DeliveryOutcome> send(String account, DraftInput input);

  // Send approvals
  Future<List<PendingSend>> listSendRequests();
  Future<PendingSend> approveSend(String id);
  Future<PendingSend> rejectSend(String id);

  // API keys
  Future<List<ApiKeyMetadata>> listApiKeys();
  Future<MintedApiKey> mintApiKey(ApiKeyInput input);
  Future<bool> revokeApiKey(String id);

  // AI
  Future<AiSettingsView> aiSettings({String? account});
  Future<AiSettingsView> aiUpdateSettings(
    AiSettings settings, {
    String? account,
  });
  Future<AiSettingsView> aiPutKey({required String kind, String? key});
  Future<bool> aiTestConnection({required String kind, String? account});
  Future<PreviewResult> aiPreview(String account, LabelRule rule);
  Future<ReclassifiedResult> aiReclassify(String account, List<String> ids);
  Future<String?> aiGetSummary(String account, String messageId);
  Future<String> aiCreateSummary(String account, String messageId);
  Future<void> aiDeleteSummary(String account, String messageId);
  Future<String> aiDraftReply(
    String account, {
    required String messageId,
    required String input,
    required String tone,
  });

  void dispose();
}
