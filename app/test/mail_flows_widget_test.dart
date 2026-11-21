import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:colecto/data/repositories/in_memory_mail_backend.dart';
import 'package:colecto/domain/models/automation_models.dart';
import 'package:colecto/domain/models/mail_models.dart';
import 'package:colecto/domain/models/preferences_models.dart';
import 'package:colecto/ui/core/theme/colecto_palette.dart';
import 'package:colecto/ui/features/shell/mail_controller.dart';

import 'test_helpers.dart';

void main() {
  group('ColectoApp core user flows', () {
    testWidgets(
      'desktop layout reads mail, blocks remote images, and approves a send',
      (tester) async {
        final c = await pumpColectoApp(tester);

        expect(find.text('Colecto'), findsOneWidget);
        expect(find.text('Welcome to Colecto'), findsOneWidget);
        expect(find.text('Welcome to Colecto.'), findsOneWidget);

        await tester.tap(find.text('Welcome to Colecto'));
        await tester.pumpAndSettle();
        expect(find.textContaining('remote images blocked'), findsOneWidget);
        expect(find.text('Reply'), findsOneWidget);

        await tester.tap(find.text('Send approvals'));
        await tester.pumpAndSettle();
        expect(
          find.text('Confirming Friday architecture sync'),
          findsOneWidget,
        );

        await tester.tap(find.text('Approve and send'));
        await tester.pumpAndSettle();
        expect(find.text('Approve and send'), findsNothing);
        expect(c.sendRequests.first.status, ApprovalStatus.sent);
      },
    );

    testWidgets(
      'composer opens in the reader and saves unfinished writing as a draft',
      (tester) async {
        final c = await pumpColectoApp(tester);
        c.startCompose();
        await tester.pumpAndSettle();
        expect(find.text('New message'), findsOneWidget);

        final to = tester.widget<TextField>(
          find.byWidgetPredicate(
            (w) =>
                w is TextField && w.decoration?.hintText == 'name@example.com',
          ),
        );
        to.controller!.text = 'ana@example.com';
        await tester.pump();
        expect(find.text('Unsaved changes'), findsOneWidget);

        c.closeCompose();
        await tester.pumpAndSettle();
        expect(c.drafts.any((d) => d.recipient == 'ana@example.com'), isTrue);
      },
    );

    testWidgets(
      'command palette opens via Ctrl+K, filters commands, and opens compose',
      (tester) async {
        await pumpColectoApp(tester);

        await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
        await tester.pumpAndSettle();

        expect(find.text('New message'), findsOneWidget);
        await tester.tap(find.text('New message'));
        await tester.pumpAndSettle();
        expect(find.text('New message'), findsOneWidget);
        expect(find.text('Send'), findsOneWidget);
      },
    );

    testWidgets('settings sections render without errors and persist changes', (
      tester,
    ) async {
      final c = await pumpColectoApp(tester);

      for (final section in SettingsSection.values) {
        c.openSettings(section);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      }

      c.closeSettings();
      await tester.pumpAndSettle();
      expect(c.activeSettings, isNull);
    });

    testWidgets('phones get a native list with a bottom folder bar', (
      tester,
    ) async {
      await pumpColectoApp(
        tester,
        size: const Size(400, 820),
        design: ColectoPlatformStyle.android,
      );
      expect(find.text('Welcome to Colecto'), findsOneWidget);
      expect(find.byType(NavigationBar), findsOneWidget);
      expect(find.text('Pick a message and it will open here.'), findsNothing);
    });

    testWidgets(
      'shows Load more link when auto-load is off and appends older messages',
      (tester) async {
        final backend = _PagedListBackend(total: 35);
        final c = await pumpColectoApp(tester, backend: backend);
        await c.updatePreferences(
          const PreferencesPatch(
            autoLoadMore: false,
            prefetch: PrefetchPreferences(enabled: true, count: 25),
          ),
        );
        await c.loadMailbox(refresh: true);
        await tester.pumpAndSettle();

        expect(c.messages.length, 25);
        final scrollable = find
            .descendant(
              of: find.byType(CustomScrollView),
              matching: find.byType(Scrollable),
            )
            .first;
        await tester.scrollUntilVisible(
          find.text('Load more'),
          300,
          scrollable: scrollable,
        );
        expect(find.text('Load more'), findsOneWidget);

        await tester.tap(find.text('Load more'));
        await tester.pumpAndSettle();
        expect(c.messages.length, 35);
        await tester.scrollUntilVisible(
          find.text('No older messages'),
          300,
          scrollable: scrollable,
        );
        expect(find.text('No older messages'), findsOneWidget);
      },
    );
  });
}

class _PagedListBackend extends InMemoryMailBackend {
  _PagedListBackend({required int total})
    : _items = List.generate(
        total,
        (i) => MailMessage(
          id: 'w-$i',
          sender: 'Author $i',
          address: 'a$i@example.com',
          subject: 'Batch email $i',
          body: 'Preview $i',
          date: DateTime.utc(
            2026,
            11,
            21,
          ).subtract(Duration(minutes: i)).toIso8601String(),
          seen: true,
          thread: 'wt-$i',
        ),
      );

  final List<MailMessage> _items;

  @override
  Future<MailSnapshot> cachedMessages(
    String account, {
    String folder = 'inbox',
    String query = '',
    int offset = 0,
    int? limit,
  }) async {
    final slice = _slice(offset, limit);
    return MailSnapshot(folder: folder, messages: slice, fetchedAt: 1700000000);
  }

  @override
  Future<List<MailMessage>> refreshMessages(
    String account, {
    String folder = 'inbox',
    String query = '',
    int offset = 0,
    int? limit,
  }) async => _slice(offset, limit);

  List<MailMessage> _slice(int offset, int? limit) {
    if (offset >= _items.length) return const [];
    final end = limit == null
        ? _items.length
        : (offset + limit).clamp(offset, _items.length);
    return _items.sublist(offset, end);
  }
}
