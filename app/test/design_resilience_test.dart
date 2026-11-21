import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:colecto/ui/core/theme/colecto_palette.dart';
import 'package:colecto/ui/features/shell/mail_controller.dart';

import 'test_helpers.dart';

/// Worst-case layout resilience across all 5 native design systems.
///
/// Rather than running a 30-test cartesian product (5 designs × 3 sizes × 2 datasets),
/// every design is stressed at the narrowest mobile width (360 px — where overflows
/// actually happen), plus representative desktop (1280 px) and tablet (820 px)
/// viewports on two distinct desktop designs (`colecto` and `fluent`).
void main() {
  Future<void> exerciseWorstCase(
    WidgetTester tester,
    Size size,
    ColectoPlatformStyle design,
  ) async {
    final c = await pumpColectoApp(
      tester,
      size: size,
      design: design,
      worstCase: true,
    );

    // Longest message (attachments, recipients, long subject),
    // missing sender/subject, and a 27-message conversation.
    await c.selectThread(c.threads.first);
    await tester.pumpAndSettle();
    await c.selectThread(c.threads[1]);
    await tester.pumpAndSettle();
    await c.selectThread(c.threads.firstWhere((t) => t.messages.length > 20));
    await tester.pumpAndSettle();

    c.startCompose();
    await tester.pumpAndSettle();
    c.closeCompose();
    await tester.pumpAndSettle();

    // Check representative dense settings screens.
    for (final section in const [
      SettingsSection.accounts,
      SettingsSection.appearance,
      SettingsSection.ai,
    ]) {
      c.openSettings(section);
      await tester.pumpAndSettle();
    }
    c.closeSettings();
    await tester.pumpAndSettle();
    c.clearSelection();
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  }

  group('Worst-case layout resilience', () {
    for (final design in ColectoPlatformStyle.values) {
      testWidgets('${design.id} survives worst-case data at 360 px mobile', (
        tester,
      ) async {
        await exerciseWorstCase(tester, const Size(360, 740), design);
      });
    }

    for (final (design, size) in const [
      (ColectoPlatformStyle.desktop, Size(1280, 800)),
      (ColectoPlatformStyle.fluent, Size(820, 700)),
    ]) {
      testWidgets(
        '${design.id} survives worst-case data at ${size.width.toInt()} px',
        (tester) async {
          await exerciseWorstCase(tester, size, design);
        },
      );
    }
  });
}
