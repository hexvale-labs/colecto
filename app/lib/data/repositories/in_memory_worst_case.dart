part of 'in_memory_mail_backend.dart';

/// Worst realistic demo data (`--dart-define=COLECTO_DEMO=worst`, dev only):
/// long names and unbreakable addresses, one-letter names, missing subjects,
/// non-Latin and right-to-left text, emoji, huge counts, long label names,
/// a 27-message thread, enormous and empty attachments, and a long list.
/// It replaces the friendly demo mailbox so every screen meets it.
extension _WorstCase on InMemoryMailBackend {
  void _seedWorstCase() {
    _accounts
      ..clear()
      ..addAll(const [
        AccountInfo(
          id: 'demo',
          settings: AccountSettings(
            name: 'Aleksandra Wiśniewska-Kowalczyk (Northwind Industries)',
            address: 'aleksandra.wisniewska-kowalczyk@northwind-industries-holdings.example.com',
            protocol: MailProtocol.imap,
            imapHost: 'imap.northwind-industries-holdings.example.com',
            smtpHost: 'smtp.northwind-industries-holdings.example.com',
          ),
          connected: true,
          configured: true,
        ),
        AccountInfo(
          id: 'jo',
          settings: AccountSettings(
            name: 'Jo',
            address: 'jo@x.io',
            protocol: MailProtocol.jmap,
          ),
          connected: false,
          configured: true,
        ),
      ]);

    final now = DateTime.now();
    String ago(Duration d) => now.subtract(d).toIso8601String();
    MailMessage msg(
      String id, {
      required String sender,
      required String address,
      required String subject,
      required String body,
      required Duration age,
      bool seen = false,
      bool flagged = false,
      String? thread,
    }) => MailMessage(
      id: id,
      sender: sender,
      address: address,
      subject: subject,
      body: body,
      date: ago(age),
      seen: seen,
      flagged: flagged,
      thread: thread ?? 't-$id',
    );

    const longAddress =
        'bartholomew.fitzgerald@northwind-industries-holdings.example.com';
    final inbox = <MailMessage>[
      msg(
        'w1',
        sender:
            'Bartholomew Fitzgerald-Montgomery III, Director of Procurement',
        address: longAddress,
        subject:
            'Re: Re: Fwd: [EXTERNAL] Q3 vendor consolidation — revised '
            'statement of work, pricing appendix B (v14 FINAL final) and the '
            'signature page we still need before Friday',
        body: 'Hi Aleksandra,\n\nSee https://northwind-industries-holdings.example.com/procurement/vendors/2026/q3/consolidation/statement-of-work/appendix-b?version=14&final=true&signature=required for the latest.',
        age: const Duration(minutes: 1),
        flagged: true,
      ),
      msg(
        'w2',
        sender: '',
        address:
            'no-reply+billing-notifications-eu-west-1@accounts.example.com',
        subject: '',
        body: '',
        age: const Duration(minutes: 7),
      ),
      msg(
        'w3',
        sender: 'Jo',
        address: 'jo@x.io',
        subject: 'k',
        body: 'ok',
        age: const Duration(minutes: 40),
        seen: true,
      ),
      msg(
        'w4',
        sender: '山田 太郎',
        address: 'taro.yamada@example.jp',
        subject: '来週の打ち合わせについて（議事録と資料のご確認をお願いします）',
        body: 'お世話になっております。来週の打ち合わせの議事録を添付いたしました。',
        age: const Duration(hours: 3),
      ),
      msg(
        'w5',
        sender: 'محمد عبد الرحمن',
        address: 'mohammed@example.sa',
        subject: 'تأكيد موعد الاجتماع يوم الخميس',
        body: 'مرحباً، أود تأكيد موعد اجتماعنا يوم الخميس القادم.',
        age: const Duration(hours: 5),
        seen: true,
      ),
      msg(
        'w6',
        sender: '🎉 Party Planning Committee 🎈',
        address: 'party@example.com',
        subject: '🍕🍕🍕 Friday pizza vote closes soon!!! 🗳️🔥',
        body: '👉 Vote now 👈\n\n\n\nLots of blank lines above.',
        age: const Duration(days: 1, hours: 2),
        flagged: true,
      ),
      for (var i = 0; i < 27; i++)
        msg(
          'w7-$i',
          sender: i.isEven ? 'Søren Lindqvist' : 'Mina Patel',
          address: i.isEven ? 'soren@example.com' : 'mina@example.com',
          subject: i == 0 ? 'Launch checklist' : 'Re: Launch checklist',
          body: 'Reply number ${i + 1} in a very long conversation.',
          age: Duration(days: 2, minutes: 27 - i),
          seen: i < 25,
          thread: 't-long',
        ),
      for (var i = 0; i < 360; i++)
        msg(
          'bulk-$i',
          sender: 'Newsletter ${i + 1}',
          address: 'news$i@lists.example.com',
          subject: 'Issue #${1284 - i}: what changed this week',
          body: 'A plain weekly digest.',
          age: Duration(days: 3 + i * 2),
          seen: i > 12,
        ),
    ];

    _mailboxes['demo'] = {
      'inbox': inbox,
      'sent': const [],
      'archive': const [],
      'trash': const [],
      'junk': const [],
    };

    _contents['demo'] = {
      'w1': MessageContent(
        text: inbox.first.body,
        attachments: const [
          AttachmentInfo(
            index: 0,
            name: 'Northwind_Industries_Q3_Vendor_Consolidation_Statement_of_Work_Appendix_B_v14_FINAL_final_signed.pdf',
            contentType: 'application/pdf',
            size: 3435973837,
          ),
          AttachmentInfo(
            index: 1,
            name: 'empty.txt',
            contentType: 'text/plain',
            size: 0,
          ),
          AttachmentInfo(
            index: 2,
            name: 'a',
            contentType: 'application/octet-stream',
            size: 1,
          ),
        ],
        to:
            'aleksandra.wisniewska-kowalczyk@northwind-industries-holdings.example.com, '
            'procurement-team@northwind-industries-holdings.example.com, '
            'legal@northwind-industries-holdings.example.com',
        cc: 'jo@x.io, $longAddress',
      ),
    };

    _labels['demo'] = {
      'Receipts & invoices — 2026 tax year': ['w1'],
      'Clients/Northwind/Procurement/Q3': ['w1', 'w2'],
      '日本': ['w4'],
      '🔥': ['w6'],
      'A': ['w3'],
      for (var i = 1; i <= 9; i++) 'Project $i': <String>[],
    };

    _drafts['demo'] = [
      const Draft(id: 'wd1', recipient: '', subject: '', body: ''),
      const Draft(
        id: 'wd2',
        recipient: '$longAddress, jo@x.io, taro.yamada@example.jp',
        subject: 'Very long draft subject that keeps going well past the edge',
        body: 'Line one\nLine two',
      ),
    ];

    final unixNow = now.millisecondsSinceEpoch ~/ 1000;
    _sendRequests
      ..clear()
      ..add(
        PendingSend(
          id: 'req-worst',
          requester:
              'Claude Desktop (MCP key “assistant-with-a-very-long-name”)',
          account: 'demo',
          from: 'aleksandra.wisniewska-kowalczyk@northwind-industries-holdings.example.com',
          snapshot: const DraftInput(
            recipient: longAddress,
            subject: '',
            body: '',
          ),
          createdAt: unixNow - 30,
          expiresAt: unixNow + 60,
          status: ApprovalStatus.pending,
        ),
      );
  }
}
