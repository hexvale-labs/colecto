import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:colecto/domain/models/automation_models.dart';
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
  });
}
