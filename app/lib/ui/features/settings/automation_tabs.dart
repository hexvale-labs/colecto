import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/automation_models.dart';
import '../../../domain/models/preferences_models.dart';
import '../../core/kit/kit.dart';
import '../../core/theme/colecto_palette.dart';
import '../shell/mail_controller.dart';

class ApiKeysSettingsTab extends StatefulWidget {
  const ApiKeysSettingsTab({super.key, required this.controller});

  final MailController controller;

  @override
  State<ApiKeysSettingsTab> createState() => _ApiKeysSettingsTabState();
}

class _ApiKeysSettingsTabState extends State<ApiKeysSettingsTab> {
  final _nameCtrl = TextEditingController();
  final Set<String> _accounts = {};
  final Set<MailCapability> _caps = {
    MailCapability.accountRead,
    MailCapability.messagesList,
    MailCapability.messagesRead,
  };
  String? _secret;

  MailController get c => widget.controller;

  @override
  void initState() {
    super.initState();
    _accounts.add(c.activeAccount);
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    super.dispose();
  }

  Future<void> _mint() async {
    if (_nameCtrl.text.trim().isEmpty) {
      c.showToast('Name the key first', isError: true);
      return;
    }
    if (_accounts.isEmpty || _caps.isEmpty) {
      c.showToast(
        'Pick at least one account and one permission',
        isError: true,
      );
      return;
    }
    final minted = await c.mintApiKey(
      ApiKeyInput(
        name: _nameCtrl.text.trim(),
        accounts: _accounts.toList(),
        capabilities: _caps.toList(),
      ),
    );
    if (mounted && minted != null) {
      _nameCtrl.clear();
      setState(() => _secret = minted.secret);
    }
  }

  Future<void> _revoke(ApiKeyMetadata key) async {
    final ok = await showColectoConfirm(
      context: context,
      title: 'Revoke “${key.name}”?',
      message: 'Anything using this key loses access immediately.',
      confirmLabel: 'Revoke key',
      danger: true,
    );
    if (ok) await c.revokeApiKey(key.id);
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const ColectoPageHead(
          title: 'API keys & MCP',
          description:
              'Let assistants and scripts use your mail through scoped keys. '
              'Each key sees only the accounts and actions you allow.',
        ),
        ColectoSection(
          children: [
            ColectoSettingRow(
              title: 'MCP server',
              description:
                  'Serve mail tools at /api/v1/mcp and through colecto-mcp.',
              trailing: ColectoSwitch(
                value: c.preferences.mcp,
                onChanged: (v) => c.updatePreferences(PreferencesPatch(mcp: v)),
              ),
            ),
          ],
        ),
        ColectoSection(
          title: 'New key',
          children: [
            SizedBox(
              width: 420,
              child: ColectoTextField(
                controller: _nameCtrl,
                labelText: 'Name',
                hintText: 'Claude Desktop',
              ),
            ),
            const SizedBox(height: 16),
            const ColectoLabel('Accounts'),
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: [
                for (final acc in c.accounts)
                  ColectoChip(
                    label: acc.displayName,
                    showCheck: true,
                    selected: _accounts.contains(acc.id),
                    onTap: () => setState(() {
                      if (!_accounts.remove(acc.id)) _accounts.add(acc.id);
                    }),
                  ),
              ],
            ),
            const SizedBox(height: 16),
            const ColectoLabel('Permissions'),
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: [
                for (final cap in MailCapability.values)
                  Tooltip(
                    message: cap.wire,
                    child: ColectoChip(
                      label: cap.label,
                      showCheck: true,
                      selected: _caps.contains(cap),
                      onTap: () => setState(() {
                        if (!_caps.remove(cap)) _caps.add(cap);
                      }),
                    ),
                  ),
              ],
            ),
            const SizedBox(height: 18),
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoButton(
                label: 'Create key',
                icon: LucideIcons.keyRound,
                variant: ColectoButtonVariant.primary,
                onPressed: _mint,
              ),
            ),
            if (_secret != null) ...[
              const SizedBox(height: 16),
              ColectoNotice(
                icon: LucideIcons.keyRound,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text('Copy this key now. It won’t be shown again.'),
                    const SizedBox(height: 8),
                    Row(
                      children: [
                        Expanded(
                          child: SelectableText(
                            _secret!,
                            style: TextStyle(
                              fontFamily: 'monospace',
                              fontSize: 12.5,
                              color: p.ink,
                            ),
                          ),
                        ),
                        ColectoButton(
                          label: 'Copy',
                          icon: LucideIcons.copy,
                          compact: true,
                          onPressed: () {
                            Clipboard.setData(ClipboardData(text: _secret!));
                            c.showToast('Key copied');
                          },
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ],
          ],
        ),
        ColectoSection(
          title: 'Keys',
          children: [
            if (c.apiKeys.isEmpty)
              const ColectoParagraph(
                'No keys yet. Nothing outside Colecto can read your mail.',
                tone: ColectoTone.muted,
              )
            else
              for (final key in c.apiKeys)
                Container(
                  padding: const EdgeInsets.symmetric(vertical: 12),
                  decoration: BoxDecoration(
                    border: Border(top: BorderSide(color: p.lineSoft)),
                  ),
                  child: Row(
                    children: [
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              children: [
                                Flexible(
                                  child: ColectoText(
                                    key.name,
                                    role: ColectoTextRole.bodyStrong,
                                    maxLines: 1,
                                  ),
                                ),
                                if (key.isRevoked) ...[
                                  const SizedBox(width: 8),
                                  const ColectoBadge(
                                    text: 'Revoked',
                                    danger: true,
                                  ),
                                ],
                              ],
                            ),
                            const SizedBox(height: 2),
                            ColectoText.caption(
                              '${key.accounts.join(', ')} · '
                              '${key.capabilities.map((e) => e.label).join(', ')}',
                            ),
                          ],
                        ),
                      ),
                      if (!key.isRevoked)
                        ColectoButton(
                          label: 'Revoke',
                          variant: ColectoButtonVariant.danger,
                          compact: true,
                          onPressed: () => _revoke(key),
                        ),
                    ],
                  ),
                ),
          ],
        ),
      ],
    );
  }
}

class SendApprovalsSettingsTab extends StatelessWidget {
  const SendApprovalsSettingsTab({super.key, required this.controller});

  final MailController controller;

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final requests = controller.sendRequests;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ColectoPageHead(
          title: 'Send approvals',
          description:
              'Mail that assistants and scripts want to send waits here until '
              'you approve it.',
          action: ColectoButton(
            label: 'Refresh',
            icon: LucideIcons.refreshCw,
            variant: ColectoButtonVariant.ghost,
            onPressed: controller.loadSendRequests,
          ),
        ),
        if (requests.isEmpty)
          const ColectoSection(
            children: [
              ColectoParagraph(
                'Nothing is waiting. Requests appear here and as a '
                'notification.',
                tone: ColectoTone.muted,
              ),
            ],
          )
        else
          for (final req in requests)
            ColectoSection(
              title: req.snapshot?.subject.isNotEmpty == true
                  ? req.snapshot!.subject
                  : '(No subject)',
              description:
                  '${req.requester} wants to send from ${req.from}'
                  '${req.snapshot != null ? ' to ${req.snapshot!.recipient}' : ''}.',
              trailing: ColectoBadge(
                text: switch (req.status) {
                  ApprovalStatus.pending => 'Waiting',
                  ApprovalStatus.sent => 'Sent',
                  ApprovalStatus.rejected => 'Rejected',
                  _ => req.status.name,
                },
                accent:
                    req.status == ApprovalStatus.pending ||
                    req.status == ApprovalStatus.sent,
                danger:
                    req.status == ApprovalStatus.rejected ||
                    req.status == ApprovalStatus.uncertain,
              ),
              children: [
                if (req.snapshot != null)
                  Container(
                    padding: const EdgeInsets.all(14),
                    decoration: BoxDecoration(
                      color: p.sunk,
                      borderRadius: BorderRadius.circular(
                        colectoRadius(context),
                      ),
                      border: Border.all(color: p.lineSoft),
                    ),
                    child: SelectableText(
                      req.snapshot!.body,
                      style: colectoType(context, ColectoTextRole.body),
                    ),
                  ),
                if (req.status == ApprovalStatus.pending) ...[
                  const SizedBox(height: 14),
                  Wrap(
                    spacing: 8,
                    children: [
                      ColectoButton(
                        label: 'Approve and send',
                        icon: LucideIcons.send,
                        variant: ColectoButtonVariant.primary,
                        onPressed: () =>
                            controller.decideSendRequest(req.id, approve: true),
                      ),
                      ColectoButton(
                        label: 'Reject',
                        variant: ColectoButtonVariant.danger,
                        onPressed: () => controller.decideSendRequest(
                          req.id,
                          approve: false,
                        ),
                      ),
                    ],
                  ),
                ],
              ],
            ),
      ],
    );
  }
}
