import 'package:flutter_test/flutter_test.dart';
import 'package:colecto/data/repositories/in_memory_mail_backend.dart';
import 'package:colecto/domain/models/automation_models.dart';
import 'package:colecto/domain/models/mail_models.dart';
import 'package:colecto/ui/core/theme/colecto_palette.dart';
import 'package:colecto/ui/features/shell/mail_controller.dart';

void main() {
  group('MailController integration', () {
    late MailController controller;

    setUp(() async {
      controller = MailController(backend: InMemoryMailBackend());
      await controller.initialize();
    });

    tearDown(() => controller.dispose());

    test('filters by folder, label, and search query cleanly', () async {
      expect(controller.activeFolder, 'inbox');
      expect(controller.threads, isNotEmpty);

      await controller.setSearchQuery('Architecture');
      expect(
        controller.threads.every(
          (t) =>
              t.subject.contains('Architecture') ||
              t.messages.any((m) => m.body.contains('Architecture')),
        ),
        isTrue,
      );

      await controller.setSearchQuery('');
      await controller.selectFolder('sent');
      expect(controller.activeFolder, 'sent');
      expect(controller.activeLabel, isNull);

      await controller.updateLabel(name: 'Urgent', messageId: 'm1');
      await controller.selectLabel('Urgent');
      expect(controller.activeLabel, 'Urgent');
      expect(controller.labelsForMessage('m1'), contains('Urgent'));
    });

    test('bulk selection and organize mutate mailbox state', () async {
      final initialCount = controller.threads.length;
      final firstId = controller.threads.first.latest.id;

      controller.toggleBulkSelect(firstId);
      expect(controller.bulkSelectedIds, {firstId});

      await controller.organizeMessages('archive', ids: [firstId]);
      expect(controller.bulkSelectedIds, isEmpty);
      expect(controller.threads.length, lessThan(initialCount));

      await controller.selectFolder('archive');
      expect(controller.messages.any((m) => m.id == firstId), isTrue);
    });

    test(
      'drafts, send approvals, and API key lifecycle work end-to-end',
      () async {
        final draft = await controller.saveDraft(
          const DraftInput(
            recipient: 'bob@example.com',
            subject: 'Integration draft',
            body: 'Checking persistence',
          ),
        );
        expect(draft, isNotNull);
        expect(controller.drafts.any((d) => d.id == draft!.id), isTrue);

        final outcome = await controller.sendMail(
          DraftInput(
            id: draft!.id,
            recipient: 'bob@example.com',
            subject: 'Integration draft',
            body: 'Checking persistence',
          ),
        );
        expect(outcome, isNotNull);
        await controller.selectFolder('sent');
        expect(
          controller.messages.any((m) => m.subject == 'Integration draft'),
          isTrue,
        );

        final minted = await controller.mintApiKey(
          const ApiKeyInput(
            name: 'CI Assistant',
            accounts: ['demo'],
            capabilities: [MailCapability.messagesList],
          ),
        );
        expect(minted, isNotNull);
        expect(minted!.secret, isNotEmpty);
        expect(controller.apiKeys.any((k) => k.id == minted.key.id), isTrue);

        await controller.revokeApiKey(minted.key.id);
        expect(
          controller.apiKeys.firstWhere((k) => k.id == minted.key.id).isRevoked,
          isTrue,
        );

        final pending = controller.sendRequests.firstWhere(
          (r) => r.status == ApprovalStatus.pending,
        );
        await controller.decideSendRequest(pending.id, approve: false);
        expect(
          controller.sendRequests.firstWhere((r) => r.id == pending.id).status,
          ApprovalStatus.rejected,
        );
      },
    );

    test('zoom steps and design preferences stay within bounds', () async {
      expect(controller.zoom, 1.0);
      await controller.zoomIn();
      expect(controller.zoom, 1.1);
      await controller.resetZoom();
      expect(controller.zoom, 1.0);
      await controller.zoomOut();
      expect(controller.zoom, 0.9);

      await controller.setDesign(ColectoPlatformStyle.fluent);
      expect(controller.design, ColectoPlatformStyle.fluent);
      expect(controller.preferences.design, 'fluent');
    });

    test(
      'settings navigation and adjacent thread selection update state',
      () async {
        controller.openSettings(SettingsSection.ai);
        expect(controller.activeSettings, SettingsSection.ai);
        controller.closeSettings();
        expect(controller.activeSettings, isNull);

        await controller.selectThread(controller.threads.first);
        final firstSelected = controller.selectedMessageId;
        controller.selectAdjacentThread(1);
        await Future<void>.delayed(Duration.zero);
        expect(controller.selectedMessageId, isNot(firstSelected));
      },
    );

    test('selectAccount clears content cache and summaries', () async {
      final first = controller.threads.first;
      await controller.selectThread(first);
      await controller.loadMessageContent(first.latest.id);
      expect(controller.contentFor(first.latest.id), isNotNull);

      await controller.selectAccount('other');
      expect(controller.contentFor(first.latest.id), isNull);
      expect(controller.summaryFor(first.latest.id), isNull);
    });
  });
}
