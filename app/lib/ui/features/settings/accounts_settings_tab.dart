import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';

import '../../../domain/models/account_models.dart';
import '../../../domain/models/preferences_models.dart';
import '../../core/theme/colecto_palette.dart';
import '../../core/kit/kit.dart';
import '../shell/mail_controller.dart';

class AccountsSettingsTab extends StatefulWidget {
  const AccountsSettingsTab({super.key, required this.controller});

  final MailController controller;

  @override
  State<AccountsSettingsTab> createState() => _AccountsSettingsTabState();
}

class _AccountsSettingsTabState extends State<AccountsSettingsTab> {
  final _nameCtrl = TextEditingController();
  final _addressCtrl = TextEditingController();
  final _usernameCtrl = TextEditingController();
  final _secretCtrl = TextEditingController();
  final _imapHostCtrl = TextEditingController();
  final _imapPortCtrl = TextEditingController(text: '993');
  final _smtpHostCtrl = TextEditingController();
  final _smtpPortCtrl = TextEditingController(text: '587');
  final _sessionUrlCtrl = TextEditingController();
  final _tlsCertCtrl = TextEditingController();
  final _vaultPasswordCtrl = TextEditingController();

  MailProtocol _protocol = MailProtocol.imap;
  bool _imapStarttls = false;
  bool _discovering = false;
  bool _saving = false;
  String? _editingId;
  DiscoveryResult? _discovery;

  @override
  void dispose() {
    _nameCtrl.dispose();
    _addressCtrl.dispose();
    _usernameCtrl.dispose();
    _secretCtrl.dispose();
    _imapHostCtrl.dispose();
    _imapPortCtrl.dispose();
    _smtpHostCtrl.dispose();
    _smtpPortCtrl.dispose();
    _sessionUrlCtrl.dispose();
    _tlsCertCtrl.dispose();
    _vaultPasswordCtrl.dispose();
    super.dispose();
  }

  void _populateForEdit(AccountInfo info) {
    final s = info.settings;
    setState(() {
      _editingId = info.id;
      _nameCtrl.text = s?.name ?? '';
      _addressCtrl.text = s?.address ?? '';
      _usernameCtrl.text = s?.username ?? '';
      _secretCtrl.clear();
      _protocol = s?.protocol ?? MailProtocol.imap;
      _imapHostCtrl.text = s?.imapHost ?? '';
      _imapPortCtrl.text = '${s?.imapPort ?? 993}';
      _smtpHostCtrl.text = s?.smtpHost ?? '';
      _smtpPortCtrl.text = '${s?.smtpPort ?? 587}';
      _sessionUrlCtrl.text = s?.sessionUrl ?? '';
      _imapStarttls = s?.imapStarttls ?? false;
      _tlsCertCtrl.text = s?.tlsCertificate ?? '';
      _discovery = null;
    });
  }

  void _clearForm() {
    setState(() {
      _editingId = null;
      _nameCtrl.clear();
      _addressCtrl.clear();
      _usernameCtrl.clear();
      _secretCtrl.clear();
      _protocol = MailProtocol.imap;
      _imapHostCtrl.clear();
      _imapPortCtrl.text = '993';
      _smtpHostCtrl.clear();
      _smtpPortCtrl.text = '587';
      _sessionUrlCtrl.clear();
      _imapStarttls = false;
      _tlsCertCtrl.clear();
      _discovery = null;
    });
  }

  Future<void> _discover() async {
    final addr = _addressCtrl.text.trim();
    if (addr.isEmpty) return;
    setState(() => _discovering = true);
    final res = await widget.controller.discoverAccount(addr);
    if (!mounted) return;
    setState(() {
      _discovering = false;
      _discovery = res;
      if (res != null) {
        if (res.username != null && _usernameCtrl.text.isEmpty) {
          _usernameCtrl.text = res.username!;
        }
        if (res.jmap != null && res.jmap!.isNotEmpty) {
          _protocol = MailProtocol.jmap;
          _sessionUrlCtrl.text = res.jmap!;
        } else if (res.imap != null) {
          _protocol = MailProtocol.imap;
          _imapHostCtrl.text = res.imap!.host;
          _imapPortCtrl.text = '${res.imap!.port}';
          _imapStarttls = res.imap!.security == DiscoverySecurity.starttls;
        }
        if (res.smtp != null) {
          _smtpHostCtrl.text = res.smtp!.host;
          _smtpPortCtrl.text = '${res.smtp!.port}';
        }
      }
    });
  }

  Future<void> _saveAccount() async {
    if (_saving) return;
    setState(() => _saving = true);
    final input = AccountInput(
      id: _editingId,
      settings: AccountSettings(
        name: _nameCtrl.text.trim(),
        address: _addressCtrl.text.trim(),
        protocol: _protocol,
        imapHost: _imapHostCtrl.text.trim(),
        imapPort: int.tryParse(_imapPortCtrl.text.trim()) ?? 993,
        smtpHost: _smtpHostCtrl.text.trim(),
        smtpPort: int.tryParse(_smtpPortCtrl.text.trim()) ?? 587,
        username: _usernameCtrl.text.trim(),
        sessionUrl: _sessionUrlCtrl.text.trim(),
        imapStarttls: _imapStarttls,
        tlsCertificate: _tlsCertCtrl.text.trim(),
      ),
      secret: _secretCtrl.text,
    );
    final saved = await widget.controller.saveAccount(input);
    if (!mounted) return;
    setState(() => _saving = false);
    if (saved != null) _clearForm();
  }

  Future<void> _confirmRemove(AccountInfo acc) async {
    final ok = await showColectoConfirm(
      context: context,
      title: 'Remove ${acc.displayName}?',
      message:
          'Colecto forgets this account, its password and its downloaded mail. '
          'Nothing is deleted on your mail server.',
      confirmLabel: 'Remove account',
      danger: true,
    );
    if (ok) await widget.controller.removeAccount(acc.id);
  }

  @override
  Widget build(BuildContext context) {
    final p = ColectoPalette.of(context);
    final c = widget.controller;
    final sec = c.security;
    final editing = _editingId != null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const ColectoPageHead(
          title: 'Accounts',
          description:
              'IMAP/SMTP and JMAP accounts. Passwords and tokens are sealed '
              'in the encrypted vault on this device.',
        ),
        ColectoSection(
          title: 'Your accounts',
          description: 'Turn an account off to hide it without removing it.',
          children: [
            for (final acc in c.accounts)
              Container(
                padding: const EdgeInsets.symmetric(vertical: 12),
                decoration: BoxDecoration(
                  border: Border(top: BorderSide(color: p.lineSoft)),
                ),
                child: Row(
                  children: [
                    ColectoAvatar(
                      name: acc.displayName,
                      hueKey: acc.address,
                      size: 34,
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          ColectoText(
                            acc.displayName,
                            role: ColectoTextRole.bodyStrong,
                            maxLines: 1,
                          ),
                          Row(
                            children: [
                              ColectoStatusDot(on: acc.connected),
                              const SizedBox(width: 6),
                              Flexible(
                                child: ColectoText.caption(
                                  '${acc.address} · '
                                  '${acc.settings?.protocol.name.toUpperCase() ?? 'Demo'}',
                                  maxLines: 1,
                                ),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                    if (acc.id != 'demo')
                      Builder(
                        builder: (ctx) => ColectoIconButton(
                          icon: LucideIcons.ellipsis,
                          tooltip: 'More',
                          onPressed: () => showColectoContextMenu(
                            context: ctx,
                            globalPosition: colectoRectOf(ctx).bottomLeft,
                            title: acc.displayName,
                            items: [
                              ColectoMenuItem(
                                label: 'Edit',
                                icon: LucideIcons.pencil,
                                onSelected: () => _populateForEdit(acc),
                              ),
                              ColectoMenuItem(
                                label: 'Disconnect',
                                icon: LucideIcons.unplug,
                                onSelected: () => c.disconnectAccount(acc.id),
                              ),
                              const ColectoMenuItem.divider(),
                              ColectoMenuItem(
                                label: 'Remove…',
                                icon: LucideIcons.trash2,
                                danger: true,
                                onSelected: () => _confirmRemove(acc),
                              ),
                            ],
                          ),
                        ),
                      ),
                    const SizedBox(width: 8),
                    ColectoSwitch(
                      value: c.preferences.accountEnabled(acc.id),
                      semanticLabel: 'Show ${acc.displayName}',
                      onChanged: (on) => c.updatePreferences(
                        PreferencesPatch(accountsEnabled: {acc.id: on}),
                      ),
                    ),
                  ],
                ),
              ),
          ],
        ),
        ColectoSection(
          title: editing ? 'Edit account' : 'Add an account',
          description: editing
              ? null
              : 'Enter your address and Colecto will look up the servers.',
          trailing: editing
              ? ColectoButton(
                  label: 'Cancel',
                  variant: ColectoButtonVariant.ghost,
                  onPressed: _clearForm,
                )
              : null,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Expanded(
                  child: ColectoTextField(
                    controller: _addressCtrl,
                    labelText: 'Email address',
                    hintText: 'you@example.com',
                    keyboardType: TextInputType.emailAddress,
                    autofillHints: const [AutofillHints.email],
                    onSubmitted: (_) => _discover(),
                  ),
                ),
                const SizedBox(width: 8),
                ColectoButton(
                  label: _discovering ? 'Looking up…' : 'Find servers',
                  icon: LucideIcons.search,
                  onPressed: _discovering ? null : _discover,
                ),
              ],
            ),
            if (_discovery != null) ...[
              const SizedBox(height: 12),
              ColectoNotice(
                icon: LucideIcons.circleCheck,
                child: Text(
                  'Found ${_discovery!.provider ?? 'your mail server'} '
                  '(${_discovery!.sources.join(', ')}).'
                  '${_discovery!.appPassword ? ' It needs an app password.' : ''}'
                  '${_discovery!.limitation != null ? ' ${_discovery!.limitation}' : ''}',
                ),
              ),
            ],
            const SizedBox(height: 16),
            _pair(
              ColectoTextField(
                controller: _nameCtrl,
                labelText: 'Name',
                hintText: 'Work, Personal…',
              ),
              ColectoTextField(
                controller: _usernameCtrl,
                labelText: 'Username',
                hintText: 'Usually your email address',
                autofillHints: const [AutofillHints.username],
              ),
            ),
            const SizedBox(height: 16),
            ColectoTextField(
              controller: _secretCtrl,
              labelText: _protocol == MailProtocol.jmap
                  ? 'API token'
                  : 'Password or app password',
              obscureText: true,
              autofillHints: const [AutofillHints.password],
            ),
            const SizedBox(height: 16),
            const ColectoLabel('Protocol'),
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoSegmented<MailProtocol>(
                value: _protocol,
                options: const [
                  (MailProtocol.imap, 'IMAP + SMTP'),
                  (MailProtocol.jmap, 'JMAP'),
                ],
                onChanged: (v) => setState(() => _protocol = v),
              ),
            ),
            const SizedBox(height: 16),
            if (_protocol == MailProtocol.jmap)
              ColectoTextField(
                controller: _sessionUrlCtrl,
                labelText: 'Session URL',
                hintText: 'https://api.fastmail.com/jmap/session',
                keyboardType: TextInputType.url,
              )
            else ...[
              _hostPort(
                'IMAP server',
                _imapHostCtrl,
                _imapPortCtrl,
                'imap.example.com',
              ),
              const SizedBox(height: 14),
              _hostPort(
                'SMTP server',
                _smtpHostCtrl,
                _smtpPortCtrl,
                'smtp.example.com',
              ),
              const SizedBox(height: 14),
              ColectoSettingRow(
                title: 'Use STARTTLS for IMAP',
                description: 'For servers such as Proton Mail Bridge.',
                trailing: ColectoSwitch(
                  value: _imapStarttls,
                  onChanged: (v) => setState(() => _imapStarttls = v),
                ),
              ),
              const SizedBox(height: 10),
              ColectoTextField(
                controller: _tlsCertCtrl,
                labelText: 'Pinned certificate (optional)',
                hintText: '-----BEGIN CERTIFICATE-----',
                helperText: 'Only for servers with a self-signed certificate.',
                monospace: true,
                maxLines: 3,
              ),
            ],
            const SizedBox(height: 18),
            Align(
              alignment: Alignment.centerLeft,
              child: ColectoButton(
                label: _saving
                    ? 'Connecting…'
                    : (editing ? 'Save changes' : 'Add account'),
                variant: ColectoButtonVariant.primary,
                onPressed: _saving ? null : _saveAccount,
              ),
            ),
          ],
        ),
        ColectoSection(
          title: 'Vault',
          description: sec.locked
              ? 'Locked. Enter the vault password to read saved passwords.'
              : sec.protected
              ? 'Protected by a password (Argon2id).'
              : 'Protected by your system keychain. Add a password for an '
                    'extra lock.',
          trailing: ColectoBadge(
            text: sec.locked
                ? 'Locked'
                : (sec.protected ? 'Password' : 'Keychain'),
            accent: !sec.locked,
            danger: sec.locked,
          ),
          children: [
            SizedBox(
              width: 360,
              child: ColectoTextField(
                controller: _vaultPasswordCtrl,
                labelText: 'Vault password',
                obscureText: true,
              ),
            ),
            const SizedBox(height: 14),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                if (!sec.protected)
                  ColectoButton(
                    label: 'Set password',
                    variant: ColectoButtonVariant.primary,
                    onPressed: () => _vault('protect'),
                  ),
                if (sec.protected && sec.locked)
                  ColectoButton(
                    label: 'Unlock',
                    variant: ColectoButtonVariant.primary,
                    onPressed: () => _vault('unlock'),
                  ),
                if (sec.protected && !sec.locked) ...[
                  ColectoButton(
                    label: 'Lock now',
                    icon: LucideIcons.lock,
                    onPressed: () => c.runSecurityAction('lock'),
                  ),
                  ColectoButton(
                    label: 'Remove password',
                    variant: ColectoButtonVariant.danger,
                    onPressed: () => _vault('remove'),
                  ),
                ],
              ],
            ),
          ],
        ),
      ],
    );
  }

  Future<void> _vault(String action) async {
    await widget.controller.runSecurityAction(
      action,
      password: _vaultPasswordCtrl.text,
    );
    _vaultPasswordCtrl.clear();
  }

  Widget _pair(Widget a, Widget b) => LayoutBuilder(
    builder: (context, constraints) => constraints.maxWidth < 480
        ? Column(children: [a, const SizedBox(height: 14), b])
        : Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(child: a),
              const SizedBox(width: 12),
              Expanded(child: b),
            ],
          ),
  );

  Widget _hostPort(
    String label,
    TextEditingController host,
    TextEditingController port,
    String hint,
  ) => Row(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Expanded(
        child: ColectoTextField(
          controller: host,
          labelText: label,
          hintText: hint,
          keyboardType: TextInputType.url,
        ),
      ),
      const SizedBox(width: 10),
      SizedBox(
        width: 96,
        child: ColectoTextField(
          controller: port,
          labelText: 'Port',
          keyboardType: TextInputType.number,
        ),
      ),
    ],
  );
}
