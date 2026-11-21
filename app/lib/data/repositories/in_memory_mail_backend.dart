import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import '../../domain/models/account_models.dart';
import '../../domain/models/ai_models.dart';
import '../../domain/models/automation_models.dart';
import '../../domain/models/mail_models.dart';
import '../../domain/models/preferences_models.dart';
import '../../domain/models/service_event.dart';
import 'mail_backend.dart';

part 'in_memory_worst_case.dart';

/// Complete in-memory [MailBackend] used for offline Demo mode and tests.
class InMemoryMailBackend implements MailBackend {
  /// [worstCase] swaps the friendly demo mailbox for worst realistic data
  /// (`--dart-define=COLECTO_DEMO=worst`) to stress the UI.
  InMemoryMailBackend({bool worstCase = false}) {
    _seed();
    if (worstCase) _seedWorstCase();
  }

  /// `--dart-define=COLECTO_DEMO=worst` (development only).
  static const bool worstCaseDefine =
      String.fromEnvironment('COLECTO_DEMO') == 'worst';

  final _events = StreamController<ServiceEvent>.broadcast();

  Preferences _preferences = const Preferences(
    account: 'demo',
    accountsEnabled: {'demo': true},
    onboarded: false,
  );
  SecurityInfo _security = const SecurityInfo(
    cleanupPending: false,
    protected: false,
    locked: false,
  );
  AiSettings _aiSettings = const AiSettings();
  bool _hasTextKey = false;
  bool _hasDecisionKey = false;

  final List<AccountInfo> _accounts = [];
  final Map<String, Map<String, List<MailMessage>>> _mailboxes = {};
  final Map<String, Map<String, MessageContent>> _contents = {};
  final Map<String, Map<String, List<String>>> _labels = {};
  final Map<String, List<Draft>> _drafts = {};
  final Map<String, Map<String, String>> _summaries = {};
  final List<ApiKeyMetadata> _apiKeys = [];
  final List<PendingSend> _sendRequests = [];

  int _idSeq = 100;

  void _seed() {
    _accounts.add(
      const AccountInfo(
        id: 'demo',
        settings: AccountSettings(
          name: 'Demo account',
          address: 'demo@colecto.local',
          protocol: MailProtocol.imap,
          imapHost: 'local',
          smtpHost: 'local',
        ),
        connected: true,
        configured: true,
      ),
    );

    final now = DateTime.now();
    final today1 = now.subtract(const Duration(minutes: 18)).toIso8601String();
    final today2 = now.subtract(const Duration(hours: 2)).toIso8601String();
    final yesterday = now
        .subtract(const Duration(days: 1, hours: 3))
        .toIso8601String();
    final lastWeek = now.subtract(const Duration(days: 5)).toIso8601String();

    _mailboxes['demo'] = {
      'inbox': [
        MailMessage(
          id: 'm1',
          sender: 'Linnea Holm',
          address: 'linnea@kogical.dev',
          subject: 'Welcome to Colecto',
          body:
              'Colecto pairs the Rust mail core with a crisp native Flutter UI.\n\n'
              'Try j/k to navigate, r to reply, e to archive, / to search, and Cmd/Ctrl+K for the command palette.',
          date: today1,
          seen: false,
          flagged: true,
          thread: 't-welcome',
        ),
        MailMessage(
          id: 'm2',
          sender: 'Soren Lind',
          address: 'soren@kogical.dev',
          subject: 'Re: Q4 Architecture Review',
          body: 'Looks great! Dropping the Wasm extension sandbox keeps memory low and makes every feature first-party.',
          date: today2,
          seen: false,
          flagged: false,
          thread: 't-arch',
        ),
        MailMessage(
          id: 'm2-prev',
          sender: 'Mina Patel',
          address: 'mina@kogical.dev',
          subject: 'Q4 Architecture Review',
          body:
              'Sharing the updated architecture notes for the Rust core and Flutter multi-platform UI. '
              'Plain-text renders natively; HTML emails stay sanitized and block remote images by default.',
          date: yesterday,
          seen: true,
          flagged: false,
          thread: 't-arch',
        ),
        MailMessage(
          id: 'm3',
          sender: 'Design Digest',
          address: 'digest@newsletter.example',
          subject: 'Weekly Typography & Motion Notes',
          body: 'WCAG 1.4.12 text spacing, crisp 1px borders, and zero-animation keyboard shortcuts make desktop tools feel instant.',
          date: lastWeek,
          seen: true,
          flagged: false,
          thread: 't-digest',
        ),
      ],
      'sent': [
        MailMessage(
          id: 's1',
          sender: 'Demo account',
          address: 'demo@colecto.local',
          subject: 'Re: Q4 Architecture Review',
          body: 'Thanks Mina — reviewing the draft now.',
          date: yesterday,
          seen: true,
          flagged: false,
          thread: 't-arch',
        ),
      ],
      'archive': [],
      'trash': [],
      'junk': [],
    };

    _contents['demo'] = {
      'm1': const MessageContent(
        text:
            'Colecto pairs the Rust mail core with a crisp native Flutter UI.\n\n'
            'Try j/k to navigate, r to reply, e to archive, / to search, and Cmd/Ctrl+K for the command palette.\n\n'
            'Everything Colecto did is built in as first-party modules: multi-account IMAP/JMAP, autodiscovery, local labels, AI summaries & reply drafts, auto-labeling rules, API keys, MCP, and send approvals.',
        html:
            '<h2>Welcome to Colecto</h2>'
            '<p>Colecto pairs the <strong>Rust mail core</strong> with a crisp native Flutter UI.</p>'
            '<p>Try <code>j/k</code> to navigate, <code>r</code> to reply, <code>e</code> to archive, <code>/</code> to search, and <code>Cmd/Ctrl+K</code> for the command palette.</p>'
            '<p><em>Remote images are blocked by default for privacy.</em></p>',
        blockedRemoteImages: 2,
        attachments: [
          AttachmentInfo(
            index: 0,
            name: 'colecto-quickstart.txt',
            contentType: 'text/plain',
            size: 1420,
          ),
        ],
        messageId: '<m1@kogical.dev>',
        to: 'demo@colecto.local',
      ),
      'm2': const MessageContent(
        text: 'Looks great! Dropping the Wasm extension sandbox keeps memory low and makes every feature first-party.\n\nLet us ship the Flutter scaffold next.',
        messageId: '<m2@kogical.dev>',
        references: '<m2-prev@kogical.dev>',
        to: 'mina@kogical.dev, demo@colecto.local',
      ),
      'm2-prev': const MessageContent(
        text:
            'Sharing the updated architecture notes for the Rust core and Flutter multi-platform UI. '
            'Plain-text renders natively; HTML emails stay sanitized and block remote images by default.',
        messageId: '<m2-prev@kogical.dev>',
        to: 'demo@colecto.local',
        cc: 'soren@kogical.dev',
      ),
      'm3': const MessageContent(
        text: 'WCAG 1.4.12 text spacing, crisp 1px borders, and zero-animation keyboard shortcuts make desktop tools feel instant.',
        html:
            '<h3>Weekly Typography &amp; Motion Notes</h3>'
            '<p>WCAG 1.4.12 text spacing, crisp 1px borders, and zero-animation keyboard shortcuts make desktop tools feel instant.</p>',
        blockedRemoteImages: 3,
        messageId: '<m3@newsletter.example>',
        to: 'demo@colecto.local',
      ),
    };

    _labels['demo'] = {
      'Priority': ['m1'],
      'Engineering': ['m2', 'm2-prev'],
      'Newsletters': ['m3'],
    };

    _drafts['demo'] = [
      const Draft(
        id: 'd1',
        recipient: 'linnea@kogical.dev',
        subject: 'Re: Welcome to Colecto',
        body: 'Testing out the new Flutter client — feels super snappy!',
        inReplyTo: '<m1@kogical.dev>',
      ),
    ];

    final unixNow = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    _sendRequests.add(
      PendingSend(
        id: 'req-demo-1',
        requester: 'Calendar Assistant',
        account: 'demo',
        from: 'demo@colecto.local',
        snapshot: const DraftInput(
          recipient: 'soren@kogical.dev',
          subject: 'Confirming Friday architecture sync',
          body: 'Hi Soren, confirming our 10:00 AM sync on Friday.',
        ),
        createdAt: unixNow - 180,
        expiresAt: unixNow + 3600,
        status: ApprovalStatus.pending,
      ),
    );
  }

  @override
  String get backendLabel => 'Built-in Demo Mode (in-memory)';

  @override
  Future<VersionInfo> version() async => const VersionInfo(
    name: 'colecto',
    version: '0.1.0',
    commit: 'demo',
    api: 'v1',
  );

  @override
  Future<ReleaseStatus> latestRelease() async =>
      const ReleaseStatus(current: '0.1.0');

  @override
  Stream<ServiceEvent> events({bool window = true}) => _events.stream;

  @override
  Future<Preferences> getPreferences() async => _preferences;

  @override
  Future<Preferences> updatePreferences(PreferencesPatch patch) async {
    final enabled = Map<String, bool>.from(_preferences.accountsEnabled);
    if (patch.accountsEnabled != null) {
      enabled.addAll(patch.accountsEnabled!);
    }
    final changed = Set<String>.from(_preferences.changed);
    for (final key in patch.reset) {
      changed.remove(key);
    }

    String? accent = _preferences.accent;
    if (patch.accent != null) {
      accent = patch.accent!.isEmpty ? null : patch.accent!.toLowerCase();
      if (accent == null) {
        changed.remove('accent');
      } else {
        changed.add('accent');
      }
    }
    if (patch.reset.contains('accent')) accent = null;

    _preferences = Preferences(
      mcp: patch.mcp ?? _preferences.mcp,
      account: patch.account ?? _preferences.account,
      workspace: patch.workspace ?? _preferences.workspace,
      accent: accent,
      font: patch.reset.contains('font') || patch.font == ''
          ? null
          : (patch.font ?? _preferences.font),
      fontSize: patch.reset.contains('font_size')
          ? null
          : (patch.fontSize ?? _preferences.fontSize),
      zoom: patch.reset.contains('zoom') || patch.zoom == 100
          ? null
          : (patch.zoom ?? _preferences.zoom),
      design: patch.reset.contains('design') || patch.design == ''
          ? null
          : (patch.design ?? _preferences.design),
      textSpacing: patch.reset.contains('text_spacing')
          ? const TextSpacing()
          : (patch.textSpacing ?? _preferences.textSpacing),
      notifications: patch.reset.contains('notifications.position')
          ? _preferences.notifications.copyWith(
              position: NotificationCorner.top,
            )
          : (patch.notifications ?? _preferences.notifications),
      remoteImages: patch.remoteImages ?? _preferences.remoteImages,
      accountsEnabled: enabled,
      onboarded: patch.onboarded ?? _preferences.onboarded,
      listDensity: patch.listDensity ?? _preferences.listDensity,
      dateGroups: patch.dateGroups ?? _preferences.dateGroups,
      autoLoadMore: patch.autoLoadMore ?? _preferences.autoLoadMore,
      notifiedRelease: patch.notifiedRelease ?? _preferences.notifiedRelease,
      prefetch: patch.prefetch ?? _preferences.prefetch,
      changed: changed,
    );
    _events.add(const PreferencesServiceEvent());
    if (patch.mcp != null) {
      _events.add(McpServiceEvent(on: _preferences.mcp));
    }
    return _preferences;
  }

  @override
  Future<List<AccountInfo>> listAccounts() async =>
      List<AccountInfo>.unmodifiable(_accounts);

  @override
  Future<AccountInfo> upsertAccount(AccountInput input) async {
    final id = input.id?.isNotEmpty == true ? input.id! : 'acc-${++_idSeq}';
    final info = AccountInfo(
      id: id,
      settings: input.settings,
      connected: true,
      configured: true,
    );
    final idx = _accounts.indexWhere((a) => a.id == id);
    if (idx >= 0) {
      _accounts[idx] = info;
    } else {
      _accounts.add(info);
      _mailboxes[id] = {
        'inbox': [],
        'sent': [],
        'archive': [],
        'trash': [],
        'junk': [],
      };
      _labels[id] = {};
      _drafts[id] = [];
    }
    return info;
  }

  @override
  Future<DiscoveryResult?> discoverAccount(String address) async {
    final parts = address.trim().split('@');
    if (parts.length != 2 || parts[1].isEmpty) return null;
    final domain = parts[1].toLowerCase();
    if (domain == 'fastmail.com') {
      return DiscoveryResult(
        sources: const ['provider_list', 'jmap_well_known'],
        provider: 'Fastmail',
        jmap: 'https://api.fastmail.com/jmap/session',
        imap: const DiscoveryServer(
          host: 'imap.fastmail.com',
          port: 993,
          security: DiscoverySecurity.tls,
        ),
        smtp: const DiscoveryServer(
          host: 'smtp.fastmail.com',
          port: 465,
          security: DiscoverySecurity.tls,
        ),
        username: address.trim(),
        appPassword: true,
      );
    }
    return DiscoveryResult(
      sources: const ['autoconfig'],
      provider: domain,
      imap: DiscoveryServer(
        host: 'imap.$domain',
        port: 993,
        security: DiscoverySecurity.tls,
      ),
      smtp: DiscoveryServer(
        host: 'smtp.$domain',
        port: 587,
        security: DiscoverySecurity.starttls,
      ),
      username: address.trim(),
    );
  }

  @override
  Future<bool> removeAccount(String id) async {
    if (id == 'demo') {
      throw const MailBackendException('Demo account cannot be removed.');
    }
    _accounts.removeWhere((a) => a.id == id);
    _mailboxes.remove(id);
    return true;
  }

  @override
  Future<bool> disconnectAccount(String id) async {
    final idx = _accounts.indexWhere((a) => a.id == id);
    if (idx >= 0) {
      final prev = _accounts[idx];
      _accounts[idx] = AccountInfo(
        id: prev.id,
        settings: prev.settings,
        connected: false,
        configured: prev.configured,
      );
    }
    return true;
  }

  @override
  Future<bool> accountConnected(String account) async =>
      _accounts.any((a) => a.id == account && a.connected);

  @override
  Future<SecurityInfo> security() async => _security;

  @override
  Future<SecurityInfo> securityAction(
    String action, {
    String password = '',
  }) async {
    switch (action) {
      case 'protect':
        if (password.length < 8) {
          throw const MailBackendException(
            'Password must be at least 8 characters.',
          );
        }
        _security = const SecurityInfo(
          cleanupPending: false,
          protected: true,
          locked: false,
        );
      case 'lock':
        _security = SecurityInfo(
          cleanupPending: false,
          protected: _security.protected,
          locked: true,
        );
      case 'unlock':
        _security = SecurityInfo(
          cleanupPending: false,
          protected: _security.protected,
          locked: false,
        );
      case 'remove':
        _security = const SecurityInfo(
          cleanupPending: false,
          protected: false,
          locked: false,
        );
    }
    return _security;
  }

  @override
  Future<List<AccountSync>> syncStatus() async {
    final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    return _accounts
        .map(
          (a) => AccountSync(
            account: a.id,
            protocol: a.id == 'demo'
                ? 'demo'
                : (a.settings?.protocol.name ?? 'imap'),
            mode: a.id == 'demo'
                ? SyncMode.local
                : (a.connected ? SyncMode.imapIdle : SyncMode.offline),
            interval: 60,
            activity: SyncActivity.idle,
            updatedAt: now,
          ),
        )
        .toList(growable: false);
  }

  @override
  Future<List<Folder>> folders(String account) async {
    final box = _mailboxes[account] ?? const {};
    int unread(String key) =>
        (box[key] ?? const []).where((m) => !m.seen).length;
    int total(String key) => (box[key] ?? const []).length;
    final draftCount = (_drafts[account] ?? const []).length;
    return [
      Folder(
        id: 'inbox',
        name: 'Inbox',
        role: FolderRole.inbox,
        unread: unread('inbox'),
        total: total('inbox'),
      ),
      Folder(
        id: 'sent',
        name: 'Sent',
        role: FolderRole.sent,
        unread: 0,
        total: total('sent'),
      ),
      Folder(
        id: 'drafts',
        name: 'Drafts',
        role: FolderRole.drafts,
        unread: 0,
        total: draftCount,
      ),
      Folder(
        id: 'archive',
        name: 'Archive',
        role: FolderRole.archive,
        unread: unread('archive'),
        total: total('archive'),
      ),
      Folder(
        id: 'trash',
        name: 'Trash',
        role: FolderRole.trash,
        unread: unread('trash'),
        total: total('trash'),
      ),
      Folder(
        id: 'junk',
        name: 'Junk',
        role: FolderRole.junk,
        unread: unread('junk'),
        total: total('junk'),
      ),
    ];
  }

  @override
  Future<LabelMap> labels(String account) async {
    final map = _labels[account] ?? const {};
    return map.map((k, v) => MapEntry(k, List<String>.unmodifiable(v)));
  }

  @override
  Future<LabelMap> updateLabel(
    String account, {
    required String name,
    String? messageId,
    bool remove = false,
  }) async {
    final trimmed = name.trim();
    if (trimmed.isEmpty) {
      throw const MailBackendException('Label name cannot be empty.');
    }
    final map = _labels.putIfAbsent(account, () => {});
    if (messageId == null) {
      if (remove) {
        map.remove(trimmed);
      } else {
        map.putIfAbsent(trimmed, () => []);
      }
    } else {
      final list = map.putIfAbsent(trimmed, () => []);
      if (remove) {
        list.remove(messageId);
      } else if (!list.contains(messageId)) {
        list.add(messageId);
      }
    }
    _events.add(LabelsUpdatedServiceEvent(account: account));
    return labels(account);
  }

  List<MailMessage> _filter(
    String account,
    String folder,
    String query,
    int offset,
    int? limit,
  ) {
    final list = _mailboxes[account]?[folder] ?? const <MailMessage>[];
    final q = query.trim().toLowerCase();
    final filtered = q.isEmpty
        ? list
        : list
              .where((m) {
                return m.subject.toLowerCase().contains(q) ||
                    m.sender.toLowerCase().contains(q) ||
                    m.address.toLowerCase().contains(q) ||
                    m.body.toLowerCase().contains(q);
              })
              .toList(growable: false);
    if (offset >= filtered.length) return const [];
    final sliced = filtered.sublist(offset);
    if (limit != null && sliced.length > limit) {
      return sliced.sublist(0, limit);
    }
    return sliced;
  }

  @override
  Future<MailSnapshot> cachedMessages(
    String account, {
    String folder = 'inbox',
    String query = '',
    int offset = 0,
    int? limit,
  }) async {
    return MailSnapshot(
      folder: folder,
      messages: _filter(account, folder, query, offset, limit),
      fetchedAt: DateTime.now().millisecondsSinceEpoch ~/ 1000,
    );
  }

  @override
  Future<List<MailMessage>> refreshMessages(
    String account, {
    String folder = 'inbox',
    String query = '',
    int offset = 0,
    int? limit,
  }) async {
    return _filter(account, folder, query, offset, limit);
  }

  @override
  Future<MailMessage> message(String account, String id) async {
    final box = _mailboxes[account] ?? const {};
    for (final list in box.values) {
      for (final msg in list) {
        if (msg.id == id) return msg;
      }
    }
    throw const MailBackendException(
      'Message not found',
      category: 'not_found',
    );
  }

  @override
  Future<MessageContent> messageContent(
    String account,
    String id, {
    bool cached = false,
    bool? remoteImages,
  }) async {
    final content = _contents[account]?[id];
    if (content != null) {
      final allowRemote = remoteImages ?? _preferences.remoteImages;
      return MessageContent(
        text: content.text,
        html: content.html,
        blockedRemoteImages: allowRemote ? 0 : content.blockedRemoteImages,
        attachments: content.attachments,
        messageId: content.messageId,
        references: content.references,
        replyTo: content.replyTo,
        to: content.to,
        cc: content.cc,
      );
    }
    final msg = await message(account, id);
    return MessageContent(text: msg.body, to: account);
  }

  @override
  Future<DownloadedAttachment> downloadAttachment(
    String account,
    String messageId,
    int index,
  ) async {
    final content = await messageContent(account, messageId);
    final info = content.attachments.firstWhere(
      (a) => a.index == index,
      orElse: () => const AttachmentInfo(
        index: 0,
        name: 'attachment.txt',
        contentType: 'text/plain',
        size: 0,
      ),
    );
    final bytes = Uint8List.fromList(
      utf8.encode('Colecto demo attachment: ${info.name}\n'),
    );
    return DownloadedAttachment(
      name: info.name,
      contentType: info.contentType,
      bytes: bytes,
    );
  }

  @override
  Future<List<ThreadMessage>> conversation(String account, String id) async {
    final target = await message(account, id);
    final box = _mailboxes[account] ?? const {};
    final results = <ThreadMessage>[];
    for (final entry in box.entries) {
      for (final msg in entry.value) {
        if (target.thread.isNotEmpty
            ? msg.thread == target.thread
            : msg.id == target.id) {
          results.add(ThreadMessage(folder: entry.key, message: msg));
        }
      }
    }
    results.sort((a, b) => a.message.date.compareTo(b.message.date));
    return results;
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
    final box = _mailboxes[account];
    if (box == null) return;
    final idSet = ids.toSet();
    if (action == 'flags') {
      for (final list in box.values) {
        for (var i = 0; i < list.length; i++) {
          if (idSet.contains(list[i].id)) {
            list[i] = list[i].copyWith(seen: seen, flagged: flagged);
          }
        }
      }
    } else {
      final dest = switch (action) {
        'archive' => 'archive',
        'delete' => 'trash',
        'move' => folder ?? 'inbox',
        _ => null,
      };
      if (dest != null) {
        final moved = <MailMessage>[];
        for (final entry in box.entries) {
          if (entry.key == dest) continue;
          entry.value.removeWhere((m) {
            if (idSet.contains(m.id)) {
              moved.add(m);
              return true;
            }
            return false;
          });
        }
        box.putIfAbsent(dest, () => []).insertAll(0, moved);
      }
    }
    _events.add(MailServiceEvent(account: account, ids: ids));
  }

  @override
  Future<List<Draft>> drafts(String account) async =>
      List<Draft>.unmodifiable(_drafts[account] ?? const []);

  @override
  Future<Draft> draft(String account, String id) async {
    final list = _drafts[account] ?? const [];
    return list.firstWhere(
      (d) => d.id == id,
      orElse: () => throw const MailBackendException('Draft not found'),
    );
  }

  @override
  Future<Draft> saveDraft(String account, DraftInput input) async {
    final list = _drafts.putIfAbsent(account, () => []);
    final id = input.id?.isNotEmpty == true ? input.id! : 'd-${++_idSeq}';
    final draft = Draft(
      id: id,
      recipient: input.recipient,
      cc: input.cc,
      subject: input.subject,
      body: input.body,
      inReplyTo: input.inReplyTo,
      references: input.references,
      forwardOf: input.forwardOf,
    );
    final idx = list.indexWhere((d) => d.id == id);
    if (idx >= 0) {
      list[idx] = draft;
    } else {
      list.insert(0, draft);
    }
    return draft;
  }

  @override
  Future<DeliveryOutcome> send(String account, DraftInput input) async {
    final id = 's-${++_idSeq}';
    final sentMsg = MailMessage(
      id: id,
      sender: 'Demo account',
      address: 'demo@colecto.local',
      subject: input.subject.isEmpty ? '(No subject)' : input.subject,
      body: input.body,
      date: DateTime.now().toIso8601String(),
      seen: true,
    );
    _mailboxes
        .putIfAbsent(account, () => {})
        .putIfAbsent('sent', () => [])
        .insert(0, sentMsg);
    if (input.id != null) {
      _drafts[account]?.removeWhere((d) => d.id == input.id);
    }
    _events.add(MailServiceEvent(account: account, ids: [id]));
    return const DeliveryOutcome(sent: true);
  }

  @override
  Future<List<PendingSend>> listSendRequests() async =>
      List<PendingSend>.unmodifiable(_sendRequests);

  @override
  Future<PendingSend> approveSend(String id) async {
    final idx = _sendRequests.indexWhere((r) => r.id == id);
    if (idx < 0) throw const MailBackendException('Send request not found');
    final cur = _sendRequests[idx];
    final next = PendingSend(
      id: cur.id,
      requester: cur.requester,
      account: cur.account,
      from: cur.from,
      snapshot: cur.snapshot,
      createdAt: cur.createdAt,
      expiresAt: cur.expiresAt,
      status: ApprovalStatus.sent,
    );
    _sendRequests[idx] = next;
    return next;
  }

  @override
  Future<PendingSend> rejectSend(String id) async {
    final idx = _sendRequests.indexWhere((r) => r.id == id);
    if (idx < 0) throw const MailBackendException('Send request not found');
    final cur = _sendRequests[idx];
    final next = PendingSend(
      id: cur.id,
      requester: cur.requester,
      account: cur.account,
      from: cur.from,
      snapshot: cur.snapshot,
      createdAt: cur.createdAt,
      expiresAt: cur.expiresAt,
      status: ApprovalStatus.rejected,
    );
    _sendRequests[idx] = next;
    return next;
  }

  @override
  Future<List<ApiKeyMetadata>> listApiKeys() async =>
      List<ApiKeyMetadata>.unmodifiable(_apiKeys);

  @override
  Future<MintedApiKey> mintApiKey(ApiKeyInput input) async {
    final id = 'key${++_idSeq}'.padRight(64, '0');
    final meta = ApiKeyMetadata(
      id: id,
      name: input.name,
      accounts: input.accounts,
      capabilities: input.capabilities,
      createdAt: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      expiresAt: input.expiresAt,
    );
    _apiKeys.insert(0, meta);
    return MintedApiKey(key: meta, secret: 'colecto_demo_secret_${++_idSeq}');
  }

  @override
  Future<bool> revokeApiKey(String id) async {
    final idx = _apiKeys.indexWhere((k) => k.id == id);
    if (idx < 0) return false;
    final prev = _apiKeys[idx];
    _apiKeys[idx] = ApiKeyMetadata(
      id: prev.id,
      name: prev.name,
      accounts: prev.accounts,
      capabilities: prev.capabilities,
      createdAt: prev.createdAt,
      expiresAt: prev.expiresAt,
      lastUsedAt: prev.lastUsedAt,
      revokedAt: DateTime.now().millisecondsSinceEpoch ~/ 1000,
    );
    return true;
  }

  @override
  Future<AiSettingsView> aiSettings({String? account}) async => AiSettingsView(
    settings: _aiSettings,
    keys: AiKeysView(text: _hasTextKey, decision: _hasDecisionKey),
  );

  @override
  Future<AiSettingsView> aiUpdateSettings(
    AiSettings settings, {
    String? account,
  }) async {
    _aiSettings = settings;
    return aiSettings();
  }

  @override
  Future<AiSettingsView> aiPutKey({required String kind, String? key}) async {
    final present = key != null && key.trim().isNotEmpty;
    if (kind == 'text') _hasTextKey = present;
    if (kind == 'decision') _hasDecisionKey = present;
    return aiSettings();
  }

  @override
  Future<bool> aiTestConnection({required String kind, String? account}) async {
    if (!_aiSettings.enabled) {
      throw const MailBackendException(
        'AI features are off. Turn them on in Settings → AI.',
      );
    }
    return true;
  }

  @override
  Future<PreviewResult> aiPreview(String account, LabelRule rule) async {
    final inbox = _mailboxes[account]?['inbox'] ?? const <MailMessage>[];
    final errors = <String?>[];
    final compiled = <RegExp?>[];
    for (final r in rule.rules) {
      try {
        compiled.add(RegExp(r.pattern, caseSensitive: false));
        errors.add(null);
      } catch (e) {
        compiled.add(null);
        errors.add('Invalid regex: $e');
      }
    }
    final matches = <PreviewMatch>[];
    for (final m in inbox) {
      var hit = false;
      for (var i = 0; i < rule.rules.length; i++) {
        final rx = compiled[i];
        if (rx == null) continue;
        final text = switch (rule.rules[i].field) {
          RuleField.sender => '${m.sender} ${m.address}',
          RuleField.subject => m.subject,
          RuleField.body => m.body,
          RuleField.any => '${m.sender} ${m.address} ${m.subject} ${m.body}',
        };
        if (rx.hasMatch(text)) {
          hit = true;
          break;
        }
      }
      if (hit) {
        matches.add(PreviewMatch(subject: m.subject, sender: m.displaySender));
      }
    }
    return PreviewResult(
      errors: errors,
      checked: inbox.length,
      count: matches.length,
      matches: matches,
    );
  }

  @override
  Future<ReclassifiedResult> aiReclassify(
    String account,
    List<String> ids,
  ) async {
    return ReclassifiedResult(checked: ids.length, changed: 0, unanswered: 0);
  }

  @override
  Future<String?> aiGetSummary(String account, String messageId) async =>
      _summaries[account]?[messageId];

  @override
  Future<String> aiCreateSummary(String account, String messageId) async {
    if (!_aiSettings.enabled) {
      throw const MailBackendException(
        'AI features are off. Turn them on in Settings → AI.',
      );
    }
    final msg = await message(account, messageId);
    final summary =
        'Summary of "${msg.subject}" from ${msg.displaySender}: '
        '${msg.preview.length > 140 ? '${msg.preview.substring(0, 140)}…' : msg.preview}';
    _summaries.putIfAbsent(account, () => {})[messageId] = summary;
    return summary;
  }

  @override
  Future<void> aiDeleteSummary(String account, String messageId) async {
    _summaries[account]?.remove(messageId);
  }

  @override
  Future<String> aiDraftReply(
    String account, {
    required String messageId,
    required String input,
    required String tone,
  }) async {
    if (!_aiSettings.enabled) {
      throw const MailBackendException(
        'AI features are off. Turn them on in Settings → AI.',
      );
    }
    final msg = await message(account, messageId);
    return switch (tone) {
      'lazy' =>
        'hey ${msg.displaySender.split(' ').first.toLowerCase()}, $input',
      'roast' =>
        'I would normally pretend I read your whole email carefully, ${msg.displaySender}, but here is the real answer: $input',
      'troll' => 'lOl oKaY ${msg.displaySender} — $input 😅',
      _ =>
        'Hi ${msg.displaySender},\n\nThank you for your message regarding "${msg.subject}".\n\n$input\n\nBest regards,',
    };
  }

  @override
  void dispose() {
    _events.close();
  }
}
