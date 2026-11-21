import 'dart:async';

import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../data/repositories/in_memory_mail_backend.dart';
import '../../../data/repositories/local_ffi_backend.dart';
import '../../../data/repositories/mail_backend.dart';
import '../../../data/repositories/remote_http_backend.dart';
import '../../../data/repositories/session_token_store_stub.dart'
    if (dart.library.js_interop) '../../../data/repositories/session_token_store_web.dart';
import '../../../domain/models/account_models.dart';
import '../../../domain/models/ai_models.dart';
import '../../../domain/models/automation_models.dart';
import '../../../domain/models/mail_models.dart';
import '../../../domain/models/preferences_models.dart';
import '../../../domain/models/service_event.dart';
import '../../core/theme/colecto_palette.dart';
import '../../core/widgets/toast_overlay.dart';

enum SettingsSection {
  accounts('accounts', 'Accounts', LucideIcons.userRound),
  mail('mail', 'Mail', LucideIcons.mail),
  appearance('appearance', 'Appearance', LucideIcons.palette),
  notifications('notifications', 'Notifications', LucideIcons.bell),
  privacy('privacy', 'Privacy', LucideIcons.shieldCheck),
  servers('servers', 'Servers', LucideIcons.server),
  ai('ai', 'AI', LucideIcons.sparkles),
  apiKeys('api-keys', 'API keys & MCP', LucideIcons.keyRound),
  sendApprovals('send-approvals', 'Send approvals', LucideIcons.shieldAlert);

  const SettingsSection(this.routeSegment, this.label, this.icon);
  final String routeSegment;
  final String label;
  final IconData icon;

  static SettingsSection? fromSegment(String segment) {
    for (final s in SettingsSection.values) {
      if (s.routeSegment == segment) return s;
    }
    return null;
  }
}

enum BackendKind { demo, remoteHttp, localFfi }

class ComposeRequest {
  const ComposeRequest({
    this.draftId,
    this.to = '',
    this.cc = '',
    this.subject = '',
    this.body = '',
    this.inReplyTo = '',
    this.references = '',
    this.forwardOf,
    this.replyMessageId,
    this.heading = 'New message',
  });

  final String? draftId;
  final String to;
  final String cc;
  final String subject;
  final String body;
  final String inReplyTo;
  final String references;
  final String? forwardOf;
  final String? replyMessageId;

  /// `New message`, `Reply`, `Forward` or `Your draft`.
  final String heading;
}

String _plural(int n, String noun) => n == 1 ? '1 $noun' : '$n ${noun}s';

class MailController extends ChangeNotifier {
  MailController({
    MailBackend? backend,
    BackendKind initialKind = BackendKind.demo,
    String initialDaemonUrl = 'http://127.0.0.1:4173',
    String initialToken = '',
  }) : _backend =
           backend ??
           _buildBackend(initialKind, initialDaemonUrl, initialToken),
       _backendKind = initialKind,
       _daemonUrl = initialDaemonUrl,
       _daemonToken = initialToken;

  static MailBackend _buildBackend(
    BackendKind kind,
    String daemonUrl,
    String token,
  ) {
    return switch (kind) {
      BackendKind.demo => InMemoryMailBackend(
        worstCase: InMemoryMailBackend.worstCaseDefine,
      ),
      BackendKind.remoteHttp => RemoteHttpBackend(
        baseUrl: daemonUrl.trim(),
        token: token.trim(),
      ),
      BackendKind.localFfi => LocalFfiBackend(),
    };
  }

  MailBackend _backend;
  BackendKind _backendKind;
  String _daemonUrl;
  String _daemonToken;

  StreamSubscription<ServiceEvent>? _eventSub;
  int _toastSeq = 0;
  final Map<int, Timer> _toastTimers = {};

  // Global UI state
  ThemeMode _themeMode = ThemeMode.system;
  // A session preview (Appearance, or a test) wins over the saved choice.
  ColectoPlatformStyle? _sessionDesign;
  bool _loading = true;
  bool _refreshing = false;
  bool _authRequired = false;
  String? _error;
  SettingsSection? _activeSettings;
  final List<ColectoToast> _toasts = [];
  final Set<int> _heldToasts = {};
  DateTime? _syncedAt;

  // Composer hosted by the shell (reader stage on desktop, a full-screen
  // sheet on phones). The key changes for every new compose so the editor
  // starts fresh.
  ComposeRequest? _compose;
  int _composeKey = 0;

  // System metadata
  VersionInfo? _version;
  ReleaseStatus? _releaseStatus;
  SecurityInfo _security = const SecurityInfo(
    cleanupPending: false,
    protected: false,
    locked: false,
  );
  Preferences _preferences = const Preferences();

  // Accounts & sync
  List<AccountInfo> _accounts = const [];
  String _activeAccount = 'demo';
  Map<String, AccountSync> _syncByAccount = const {};

  // Mailbox state
  List<Folder> _folders = const [];
  LabelMap _labels = const {};
  String _activeFolder = 'inbox';
  String? _activeLabel;
  String _searchQuery = '';
  List<MailMessage> _messages = const [];
  List<ConversationThread> _threads = const [];
  List<Draft> _drafts = const [];
  final Set<String> _bulkSelectedIds = {};

  // Reader state
  String? _selectedMessageId;
  List<ThreadMessage> _conversation = const [];
  final Map<String, MessageContent> _contentCache = {};
  final Set<String> _loadingContentIds = {};

  // AI & Automation state
  AiSettingsView _aiSettings = const AiSettingsView();
  final Map<String, String> _summaries = {};
  final Set<String> _loadingSummaryIds = {};
  List<ApiKeyMetadata> _apiKeys = const [];
  List<PendingSend> _sendRequests = const [];

  // Getters
  MailBackend get backend => _backend;
  BackendKind get backendKind => _backendKind;
  String get daemonUrl => _daemonUrl;
  String get daemonToken => _daemonToken;
  ThemeMode get themeMode => _themeMode;

  /// The design system in use. `COLECTO_DESIGN` (a launch flag for
  /// previews) wins, then the owner's saved choice, then the platform's own.
  /// Any design works on any platform.
  ColectoPlatformStyle get design =>
      _sessionDesign ??
      ColectoPlatformStyle.parse(colectoDesignDefine) ??
      ColectoPlatformStyle.parse(_preferences.design) ??
      ColectoPlatformStyle.native();

  /// Whether a launch flag is overriding the saved design.
  bool get designForcedByLaunch =>
      ColectoPlatformStyle.parse(colectoDesignDefine) != null;

  /// Saves [design] as the owner's choice; the platform's own design is
  /// stored as no choice, so it keeps following the platform.
  Future<void> setDesign(ColectoPlatformStyle design) async {
    _sessionDesign = null;
    await updatePreferences(
      PreferencesPatch(
        design: design == ColectoPlatformStyle.native() ? '' : design.id,
      ),
    );
  }

  /// Browser-like zoom steps, in percent.
  static const zoomSteps = [50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200];

  /// Whole-interface zoom as a factor (1.0 = 100 %).
  double get zoom => (_preferences.zoom ?? 100) / 100;

  int? _zoomToast;

  Future<void> _setZoom(int percent) async {
    await updatePreferences(PreferencesPatch(zoom: percent));
    // One zoom notice that updates, not a stack of them.
    if (_zoomToast != null) dismissToast(_zoomToast!);
    showToast('Zoom $percent%');
    _zoomToast = _toastSeq;
  }

  Future<void> zoomIn() {
    final now = _preferences.zoom ?? 100;
    return _setZoom(zoomSteps.firstWhere((s) => s > now, orElse: () => 200));
  }

  Future<void> zoomOut() {
    final now = _preferences.zoom ?? 100;
    return _setZoom(
      zoomSteps.lastWhere((s) => s < now, orElse: () => zoomSteps.first),
    );
  }

  Future<void> resetZoom() => _setZoom(100);

  /// Uses [design] until restart without saving it (tests, previews).
  void previewDesign(ColectoPlatformStyle design) {
    _sessionDesign = design;
    notifyListeners();
  }

  bool get loading => _loading;
  bool get refreshing => _refreshing;
  bool get authRequired => _authRequired;
  String? get error => _error;
  SettingsSection? get activeSettings => _activeSettings;
  List<ColectoToast> get toasts => List.unmodifiable(_toasts);
  DateTime? get syncedAt => _syncedAt;
  ComposeRequest? get compose => _compose;
  int get composeKey => _composeKey;

  String get currentFolderName {
    if (_activeLabel != null) return _activeLabel!;
    for (final f in _folders) {
      if (f.id == _activeFolder) return f.name;
    }
    return _activeFolder.isEmpty
        ? 'Inbox'
        : '${_activeFolder[0].toUpperCase()}${_activeFolder.substring(1)}';
  }

  void startCompose([ComposeRequest request = const ComposeRequest()]) {
    _compose = request;
    _composeKey++;
    notifyListeners();
  }

  void closeCompose() {
    if (_compose == null) return;
    _compose = null;
    notifyListeners();
  }

  VersionInfo? get version => _version;
  ReleaseStatus? get releaseStatus => _releaseStatus;
  SecurityInfo get security => _security;
  Preferences get preferences => _preferences;

  List<AccountInfo> get accounts => _accounts;
  List<AccountInfo> get enabledAccounts {
    final enabled = _accounts
        .where((a) => _preferences.accountEnabled(a.id))
        .toList(growable: false);
    return enabled.isNotEmpty ? enabled : _accounts;
  }

  String get activeAccount => _activeAccount;
  AccountInfo? get currentAccountInfo {
    for (final a in _accounts) {
      if (a.id == _activeAccount) return a;
    }
    return _accounts.isNotEmpty ? _accounts.first : null;
  }

  AccountSync? get currentAccountSync => _syncByAccount[_activeAccount];
  Map<String, AccountSync> get syncByAccount => _syncByAccount;

  List<Folder> get folders => _folders;
  LabelMap get labels => _labels;
  String get activeFolder => _activeFolder;
  String? get activeLabel => _activeLabel;
  String get searchQuery => _searchQuery;
  List<MailMessage> get messages => _messages;
  List<ConversationThread> get threads => _threads;
  List<Draft> get drafts => _drafts;
  Set<String> get bulkSelectedIds => Set.unmodifiable(_bulkSelectedIds);

  String? get selectedMessageId => _selectedMessageId;
  List<ThreadMessage> get conversation => _conversation;
  MessageContent? contentFor(String id) => _contentCache[id];
  bool isLoadingContent(String id) => _loadingContentIds.contains(id);

  AiSettingsView get aiSettings => _aiSettings;
  String? summaryFor(String id) => _summaries[id];
  bool isLoadingSummary(String id) => _loadingSummaryIds.contains(id);
  List<ApiKeyMetadata> get apiKeys => _apiKeys;
  List<PendingSend> get sendRequests => _sendRequests;
  int get pendingApprovalCount =>
      _sendRequests.where((r) => r.status == ApprovalStatus.pending).length;

  /// Settings sections worth showing. Send approvals appears only while
  /// something waits (or while it is open, so approving the last request
  /// doesn't pull the page away).
  List<SettingsSection> get visibleSettingsSections => [
    for (final s in SettingsSection.values)
      if (s != SettingsSection.sendApprovals ||
          pendingApprovalCount > 0 ||
          _activeSettings == SettingsSection.sendApprovals)
        s,
  ];

  bool get hasUpdateBanner {
    final latest = _releaseStatus?.latest;
    if (latest == null) return false;
    return _preferences.notifiedRelease != latest.version;
  }

  void setThemeMode(ThemeMode mode) {
    _themeMode = mode;
    notifyListeners();
  }

  void openSettings([SettingsSection section = SettingsSection.accounts]) {
    _activeSettings = section;
    notifyListeners();
    if (section == SettingsSection.ai) unawaited(loadAiSettings());
    if (section == SettingsSection.apiKeys) unawaited(loadApiKeys());
    if (section == SettingsSection.sendApprovals) unawaited(loadSendRequests());
    if (section == SettingsSection.accounts) unawaited(loadSecurity());
  }

  void closeSettings() {
    _activeSettings = null;
    notifyListeners();
  }

  void showToast(String message, {bool isError = false, String? detail}) {
    if (!_preferences.notifications.enabled && !isError) return;
    final id = ++_toastSeq;
    _toasts.add(
      ColectoToast(id: id, message: message, detail: detail, isError: isError),
    );
    if (_toasts.length > 4) {
      final evicted = _toasts.removeAt(0);
      _toastTimers.remove(evicted.id)?.cancel();
    }
    notifyListeners();
    _armToast(id);
  }

  // Notifications leave after six seconds unless hovered.
  void _armToast(int id) {
    _toastTimers.remove(id)?.cancel();
    _toastTimers[id] = Timer(
      const Duration(seconds: 6),
      () => dismissToast(id),
    );
  }

  void holdToast(int id, bool hold) {
    if (hold) {
      _heldToasts.add(id);
      _toastTimers.remove(id)?.cancel();
    } else if (_heldToasts.remove(id) && _toasts.any((t) => t.id == id)) {
      _armToast(id);
    }
  }

  void dismissToast(int id) {
    _heldToasts.remove(id);
    _toastTimers.remove(id)?.cancel();
    final before = _toasts.length;
    _toasts.removeWhere((t) => t.id == id);
    if (_toasts.length != before) notifyListeners();
  }

  Future<void> switchBackend({
    required BackendKind kind,
    String? daemonUrl,
    String? token,
  }) async {
    await _eventSub?.cancel();
    _backend.dispose();
    _backendKind = kind;
    if (daemonUrl != null) _daemonUrl = daemonUrl.trim();
    if (token != null) _daemonToken = token.trim();

    _backend = switch (kind) {
      BackendKind.demo => InMemoryMailBackend(
        worstCase: InMemoryMailBackend.worstCaseDefine,
      ),
      BackendKind.remoteHttp => RemoteHttpBackend(
        baseUrl: _daemonUrl,
        token: _daemonToken,
      ),
      BackendKind.localFfi => LocalFfiBackend(),
    };
    _contentCache.clear();
    _summaries.clear();
    _selectedMessageId = null;
    _conversation = const [];
    _authRequired = false;
    await initialize();
  }

  Future<void> authenticateDaemon(String url, String token) async {
    await switchBackend(
      kind: BackendKind.remoteHttp,
      daemonUrl: url,
      token: token,
    );
  }

  Future<void> initialize() async {
    _loading = true;
    _error = null;
    notifyListeners();
    try {
      _version = await _backend.version();
      _preferences = await _backend.getPreferences();
      _accounts = await _backend.listAccounts();
      _security = await _backend.security();
      await _loadSyncStatus();

      final enabled = enabledAccounts;
      if (_preferences.account != null &&
          enabled.any((a) => a.id == _preferences.account)) {
        _activeAccount = _preferences.account!;
      } else if (enabled.isNotEmpty) {
        _activeAccount = enabled.first.id;
      }

      _authRequired = false;
      if (_backendKind == BackendKind.remoteHttp && _daemonToken.isNotEmpty) {
        writeSessionToken(_daemonToken);
      }
      _subscribeEvents();
      await loadMailbox(refresh: false);
      unawaited(_loadBackgroundMetadata());
    } on MailBackendException catch (e) {
      if (e.isAuthRequired) {
        _authRequired = true;
        clearSessionToken();
      }
      _error = e.message;
    } catch (e) {
      _error = e.toString();
    } finally {
      _loading = false;
      notifyListeners();
    }
  }

  Future<void> _loadBackgroundMetadata() async {
    try {
      _releaseStatus = await _backend.latestRelease();
      _sendRequests = await _backend.listSendRequests();
      _aiSettings = await _backend.aiSettings(account: _activeAccount);
      notifyListeners();
    } catch (_) {
      // Non-critical metadata failures are ignored on boot.
    }
  }

  void _subscribeEvents() {
    _eventSub?.cancel();
    _eventSub = _backend.events(window: true).listen(_handleServiceEvent);
  }

  void _handleServiceEvent(ServiceEvent event) {
    switch (event) {
      case ReadyServiceEvent():
        break;
      case ResyncServiceEvent():
        unawaited(loadMailbox(refresh: false));
      case MailServiceEvent(:final account):
        if (account == _activeAccount) {
          unawaited(loadMailbox(refresh: false));
        }
      case SyncServiceEvent(:final sync):
        _syncByAccount = {..._syncByAccount, sync.account: sync};
        notifyListeners();
      case ApprovalServiceEvent(:final from, :final subject):
        unawaited(loadSendRequests());
        showToast(
          'Approval requested',
          detail: '$from wants to send “$subject”.',
        );
      case NavigateServiceEvent(:final target, :final account):
        if (account.isNotEmpty &&
            account != _activeAccount &&
            _accounts.any((a) => a.id == account)) {
          unawaited(selectAccount(account));
        }
        _handleNavigateTarget(target);
      case McpServiceEvent(:final on):
        _preferences = Preferences(
          mcp: on,
          account: _preferences.account,
          workspace: _preferences.workspace,
          accent: _preferences.accent,
          font: _preferences.font,
          design: _preferences.design,
          zoom: _preferences.zoom,
          fontSize: _preferences.fontSize,
          textSpacing: _preferences.textSpacing,
          notifications: _preferences.notifications,
          remoteImages: _preferences.remoteImages,
          accountsEnabled: _preferences.accountsEnabled,
          onboarded: _preferences.onboarded,
          listDensity: _preferences.listDensity,
          dateGroups: _preferences.dateGroups,
          notifiedRelease: _preferences.notifiedRelease,
          prefetch: _preferences.prefetch,
          changed: _preferences.changed,
        );
        notifyListeners();
      case PreferencesServiceEvent():
        unawaited(reloadPreferences());
      case LabelsUpdatedServiceEvent(:final account):
        if (account == _activeAccount) {
          unawaited(loadLabels());
        }
    }
  }

  void _handleNavigateTarget(String target) {
    if (target.startsWith('settings/')) {
      final section = SettingsSection.fromSegment(
        target.substring('settings/'.length),
      );
      if (section != null) openSettings(section);
      return;
    }
    closeSettings();
    switch (target) {
      case 'inbox':
      case 'sent':
      case 'drafts':
        unawaited(selectFolder(target));
    }
  }

  Future<void> _loadSyncStatus() async {
    try {
      final statuses = await _backend.syncStatus();
      _syncByAccount = {for (final s in statuses) s.account: s};
    } catch (_) {}
  }

  Future<void> reloadPreferences() async {
    try {
      _preferences = await _backend.getPreferences();
      notifyListeners();
    } catch (_) {}
  }

  Future<void> updatePreferences(PreferencesPatch patch) async {
    try {
      _preferences = await _backend.updatePreferences(patch);
      notifyListeners();
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  Future<void> selectAccount(String accountId) async {
    if (_activeAccount == accountId) return;
    _activeAccount = accountId;
    _activeFolder = 'inbox';
    _activeLabel = null;
    _searchQuery = '';
    _selectedMessageId = null;
    _conversation = const [];
    _bulkSelectedIds.clear();
    notifyListeners();
    unawaited(updatePreferences(PreferencesPatch(account: accountId)));
    await loadMailbox(refresh: false);
  }

  Future<void> selectFolder(String folderId) async {
    _activeSettings = null;
    _activeFolder = folderId;
    _activeLabel = null;
    _selectedMessageId = null;
    _conversation = const [];
    _bulkSelectedIds.clear();
    notifyListeners();
    await loadMailbox(refresh: false);
  }

  Future<void> selectLabel(String labelName) async {
    _activeSettings = null;
    _activeLabel = labelName;
    _selectedMessageId = null;
    _conversation = const [];
    _bulkSelectedIds.clear();
    notifyListeners();
    await loadMailbox(refresh: false);
  }

  Future<void> setSearchQuery(String query) async {
    _searchQuery = query;
    notifyListeners();
    await loadMailbox(refresh: false);
  }

  Future<void> loadLabels() async {
    try {
      _labels = await _backend.labels(_activeAccount);
      notifyListeners();
    } catch (_) {}
  }

  Future<void> loadMailbox({bool refresh = true}) async {
    _refreshing = true;
    _error = null;
    notifyListeners();
    try {
      _folders = await _backend.folders(_activeAccount);
      _labels = await _backend.labels(_activeAccount);

      if (_activeFolder == 'drafts' && _activeLabel == null) {
        _drafts = await _backend.drafts(_activeAccount);
        _messages = const [];
        _threads = const [];
      } else {
        if (refresh) {
          _messages = await _backend.refreshMessages(
            _activeAccount,
            folder: _activeFolder,
            query: _searchQuery,
          );
        } else {
          final snap = await _backend.cachedMessages(
            _activeAccount,
            folder: _activeFolder,
            query: _searchQuery,
          );
          _messages = snap.messages;
        }
        var visible = _messages;
        if (_activeLabel != null) {
          final allowed = (_labels[_activeLabel!] ?? const <String>[]).toSet();
          visible = _messages
              .where((m) => allowed.contains(m.id))
              .toList(growable: false);
        }
        _threads = groupConversations(visible);
      }
      _syncedAt = DateTime.now();
      await _loadSyncStatus();
    } on MailBackendException catch (e) {
      if (e.isAuthRequired) _authRequired = true;
      _error = e.message;
      showToast(e.message, isError: true);
    } finally {
      _refreshing = false;
      notifyListeners();
    }
  }

  Future<void> selectThread(ConversationThread thread) async {
    final latest = thread.latest;
    _selectedMessageId = latest.id;
    _compose = null;
    notifyListeners();

    try {
      _conversation = await _backend.conversation(_activeAccount, latest.id);
      notifyListeners();
      for (final tm in _conversation) {
        unawaited(loadMessageContent(tm.message.id));
      }
      if (thread.hasUnread) {
        final unreadIds = thread.messages
            .where((m) => !m.seen)
            .map((m) => m.id)
            .toList(growable: false);
        if (unreadIds.isNotEmpty) {
          await organizeMessages('flags', ids: unreadIds, seen: true);
        }
      }
      unawaited(loadSummary(latest.id));
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  void clearSelection() {
    _selectedMessageId = null;
    _conversation = const [];
    notifyListeners();
  }

  void selectAdjacentThread(int delta) {
    if (_threads.isEmpty) return;
    final currentIdx = _selectedMessageId == null
        ? -1
        : _threads.indexWhere(
            (t) => t.messages.any((m) => m.id == _selectedMessageId),
          );
    final nextIdx = currentIdx < 0
        ? (delta > 0 ? 0 : _threads.length - 1)
        : (currentIdx + delta).clamp(0, _threads.length - 1);
    unawaited(selectThread(_threads[nextIdx]));
  }

  Future<void> loadMessageContent(
    String messageId, {
    bool? remoteImages,
    bool force = false,
  }) async {
    if (!force &&
        remoteImages == null &&
        _contentCache.containsKey(messageId)) {
      return;
    }
    _loadingContentIds.add(messageId);
    notifyListeners();
    try {
      final content = await _backend.messageContent(
        _activeAccount,
        messageId,
        remoteImages: remoteImages,
      );
      _contentCache[messageId] = content;
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    } finally {
      _loadingContentIds.remove(messageId);
      notifyListeners();
    }
  }

  void toggleBulkSelect(String messageId) {
    if (!_bulkSelectedIds.remove(messageId)) {
      _bulkSelectedIds.add(messageId);
    }
    notifyListeners();
  }

  void selectAllVisible() {
    _bulkSelectedIds
      ..clear()
      ..addAll(_threads.map((t) => t.latest.id));
    notifyListeners();
  }

  void clearBulkSelection() {
    _bulkSelectedIds.clear();
    notifyListeners();
  }

  Future<void> organizeMessages(
    String action, {
    required List<String> ids,
    bool? seen,
    bool? flagged,
    String? folder,
  }) async {
    if (ids.isEmpty) return;
    try {
      await _backend.organizeMessages(
        _activeAccount,
        action,
        ids: ids,
        seen: seen,
        flagged: flagged,
        folder: folder,
      );
      if (action == 'archive' || action == 'delete' || action == 'move') {
        if (ids.contains(_selectedMessageId)) {
          _selectedMessageId = null;
          _conversation = const [];
        }
        _bulkSelectedIds.removeAll(ids);
        showToast(switch (action) {
          'archive' => 'Archived ${_plural(ids.length, 'message')}',
          'delete' => 'Moved ${_plural(ids.length, 'message')} to Trash',
          _ => 'Moved ${_plural(ids.length, 'message')}',
        });
      }
      await loadMailbox(refresh: false);
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  Future<void> updateLabel({
    required String name,
    String? messageId,
    bool remove = false,
  }) async {
    try {
      _labels = await _backend.updateLabel(
        _activeAccount,
        name: name,
        messageId: messageId,
        remove: remove,
      );
      notifyListeners();
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  List<String> labelsForMessage(String messageId) {
    final result = <String>[];
    for (final entry in _labels.entries) {
      if (entry.value.contains(messageId)) result.add(entry.key);
    }
    return result;
  }

  // Drafts & Send
  Future<Draft?> saveDraft(DraftInput input, {bool quiet = false}) async {
    try {
      final saved = await _backend.saveDraft(_activeAccount, input);
      _drafts = await _backend.drafts(_activeAccount);
      if (!quiet) showToast('Draft saved');
      notifyListeners();
      return saved;
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
      return null;
    }
  }

  Future<DeliveryOutcome?> sendMail(DraftInput input) async {
    try {
      final outcome = await _backend.send(_activeAccount, input);
      if (outcome.requiresApproval) {
        showToast(
          'Waiting for approval',
          detail: 'Approve it in Settings → Send approvals to send it.',
        );
        unawaited(loadSendRequests());
      } else {
        showToast('Message sent');
      }
      await loadMailbox(refresh: false);
      return outcome;
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
      return null;
    }
  }

  // Accounts & Security
  Future<void> reloadAccounts() async {
    try {
      _accounts = await _backend.listAccounts();
      await _loadSyncStatus();
      notifyListeners();
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  Future<AccountInfo?> saveAccount(AccountInput input) async {
    try {
      final info = await _backend.upsertAccount(input);
      await reloadAccounts();
      showToast('Saved account ${info.displayName}');
      return info;
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
      return null;
    }
  }

  Future<DiscoveryResult?> discoverAccount(String address) async {
    try {
      return await _backend.discoverAccount(address);
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
      return null;
    }
  }

  Future<void> disconnectAccount(String id) async {
    try {
      await _backend.disconnectAccount(id);
      await reloadAccounts();
      showToast('Account disconnected');
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  Future<void> removeAccount(String id) async {
    try {
      await _backend.removeAccount(id);
      await reloadAccounts();
      if (_activeAccount == id && enabledAccounts.isNotEmpty) {
        await selectAccount(enabledAccounts.first.id);
      }
      showToast('Account removed');
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  Future<void> loadSecurity() async {
    try {
      _security = await _backend.security();
      notifyListeners();
    } catch (_) {}
  }

  Future<void> runSecurityAction(String action, {String password = ''}) async {
    try {
      _security = await _backend.securityAction(action, password: password);
      showToast('Vault updated');
      notifyListeners();
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  // AI
  Future<void> loadAiSettings() async {
    try {
      _aiSettings = await _backend.aiSettings(account: _activeAccount);
      notifyListeners();
    } catch (_) {}
  }

  Future<void> saveAiSettings(AiSettings settings) async {
    try {
      _aiSettings = await _backend.aiUpdateSettings(
        settings,
        account: _activeAccount,
      );
      showToast('AI settings saved');
      notifyListeners();
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  Future<void> saveAiKey(String kind, String? key) async {
    try {
      _aiSettings = await _backend.aiPutKey(kind: kind, key: key);
      showToast(key == null ? 'API key cleared' : 'API key saved');
      notifyListeners();
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  Future<bool> testAiConnection(String kind) async {
    try {
      final ok = await _backend.aiTestConnection(
        kind: kind,
        account: _activeAccount,
      );
      showToast(
        ok ? 'Connection succeeded' : 'Connection failed',
        isError: !ok,
      );
      return ok;
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
      return false;
    }
  }

  Future<PreviewResult?> previewAiRule(LabelRule rule) async {
    try {
      return await _backend.aiPreview(_activeAccount, rule);
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
      return null;
    }
  }

  Future<void> reclassifyMessages(List<String> ids) async {
    if (ids.isEmpty) return;
    try {
      final res = await _backend.aiReclassify(_activeAccount, ids);
      await loadLabels();
      showToast(
        'Reclassified ${_plural(res.checked, 'message')}',
        detail: res.changed == 0
            ? 'No labels changed.'
            : '${_plural(res.changed, 'label change')}.',
      );
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  Future<void> loadSummary(String messageId) async {
    try {
      final summary = await _backend.aiGetSummary(_activeAccount, messageId);
      if (summary != null && summary.isNotEmpty) {
        _summaries[messageId] = summary;
        notifyListeners();
      }
    } catch (_) {}
  }

  Future<void> generateSummary(String messageId) async {
    _loadingSummaryIds.add(messageId);
    notifyListeners();
    try {
      final summary = await _backend.aiCreateSummary(_activeAccount, messageId);
      _summaries[messageId] = summary;
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    } finally {
      _loadingSummaryIds.remove(messageId);
      notifyListeners();
    }
  }

  Future<void> deleteSummary(String messageId) async {
    try {
      await _backend.aiDeleteSummary(_activeAccount, messageId);
      _summaries.remove(messageId);
      notifyListeners();
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  Future<String?> generateAiReply({
    required String messageId,
    required String notes,
    required String tone,
  }) async {
    try {
      return await _backend.aiDraftReply(
        _activeAccount,
        messageId: messageId,
        input: notes,
        tone: tone,
      );
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
      return null;
    }
  }

  // API Keys & Send Approvals
  Future<void> loadApiKeys() async {
    try {
      _apiKeys = await _backend.listApiKeys();
      notifyListeners();
    } catch (_) {}
  }

  Future<MintedApiKey?> mintApiKey(ApiKeyInput input) async {
    try {
      final minted = await _backend.mintApiKey(input);
      await loadApiKeys();
      showToast('API key created');
      return minted;
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
      return null;
    }
  }

  Future<void> revokeApiKey(String id) async {
    try {
      await _backend.revokeApiKey(id);
      await loadApiKeys();
      showToast('API key revoked');
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  Future<void> loadSendRequests() async {
    try {
      _sendRequests = await _backend.listSendRequests();
      notifyListeners();
    } catch (_) {}
  }

  Future<void> decideSendRequest(String id, {required bool approve}) async {
    try {
      if (approve) {
        await _backend.approveSend(id);
        showToast('Send request approved');
      } else {
        await _backend.rejectSend(id);
        showToast('Send request rejected');
      }
      await loadSendRequests();
    } on MailBackendException catch (e) {
      showToast(e.message, isError: true);
    }
  }

  @override
  void dispose() {
    _heldToasts.clear();
    for (final timer in _toastTimers.values) {
      timer.cancel();
    }
    _toastTimers.clear();
    _eventSub?.cancel();
    _backend.dispose();
    super.dispose();
  }
}
