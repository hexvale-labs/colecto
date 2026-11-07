<script lang="ts">
  import { hostService } from './lib/api';
  import type { AccountInfo, AccountSettings, SecurityInfo } from './lib/api';
  let { accounts, security, onrefresh, onback, ondirty, onbusy }: {
    accounts: AccountInfo[]; security: SecurityInfo; onrefresh: () => Promise<void>; onback: () => void;
    ondirty: (value: boolean) => void; onbusy: (value: boolean) => void;
  } = $props();
  const empty = (): AccountSettings => ({ name: '', address: '', protocol: 'imap', imap_host: '', imap_port: 993, smtp_host: '', smtp_port: 587, username: '', session_url: '' });
  let settings = $state(empty());
  let editing = $state<string | undefined>();
  let secret = $state('');
  let password = $state('');
  let confirm = $state('');
  let baseline = $state(JSON.stringify(empty()));
  let working = $state(false);
  let error = $state('');
  let notice = $state('');
  let dirty = $derived(JSON.stringify(settings) !== baseline || !!secret || !!password || !!confirm);
  $effect(() => ondirty(dirty));
  function resetForm() { settings = empty(); baseline = JSON.stringify(settings); editing = undefined; secret = ''; }
  function edit(account: AccountInfo) {
    if (dirty && !window.confirm('Discard unsaved account settings?')) return;
    if (!account.settings) return;
    settings = { ...account.settings }; baseline = JSON.stringify(settings); editing = account.id; secret = ''; notice = ''; error = '';
  }
  async function run(action: () => Promise<unknown>) {
    working = true; onbusy(true); error = ''; notice = '';
    try { await action(); }
    catch (e) { error = (e as Error).message; }
    finally {
      // Reload authoritative state even if post-commit credential cleanup failed.
      try { await onrefresh(); } catch (e) { error ||= (e as Error).message; }
      working = false; onbusy(false);
    }
  }
  async function save(event: SubmitEvent) {
    event.preventDefault();
    await run(async () => {
      await hostService.save({ ...settings, id: editing, secret });
      resetForm(); notice = 'Account saved. Open Inbox to check the connection.';
    });
  }
  async function protect(action: string) {
    if (action === 'protect' && password !== confirm) { error = 'Master passwords do not match'; return; }
    if (action === 'reset' && !window.confirm('Forget all saved mail credentials? You must reconnect every account. Local drafts will be kept.')) return;
    await run(async () => {
      await hostService.securityAction(action, password);
      password = ''; confirm = ''; secret = '';
      notice = action === 'reset' ? 'Credentials reset. Reconnect your accounts.' : '';
    });
  }
</script>
<section class="settings-page" aria-labelledby="settings-title">
  <header class="settings-header"><div><div class="eyebrow">COLECTO SETTINGS</div><h1 id="settings-title">Accounts &amp; security</h1><p>Your accounts, kept separate. Your credentials, protected.</p></div><button class="secondary" disabled={working} onclick={onback}>Back to mail</button></header>
  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  {#if notice}<p class="settings-notice" role="status">{notice}</p>{/if}
  <div class="settings-grid">
    <section class="settings-card" aria-label="Saved accounts"><h2>Mail accounts</h2><p>Inbox and drafts stay with their account.</p>
      <ul class="account-list">{#each accounts as account (account.id)}<li><div><strong>{account.settings?.name ?? 'Demo account'}</strong><span>{account.settings?.address ?? 'Sample mail · local drafts'}</span><small>{account.configured ? security.locked ? 'Locked' : 'Configured · connection checked in Inbox' : account.settings ? 'Disconnected · drafts kept' : 'Local sample'}</small></div>
      {#if account.settings}<div class="account-actions"><button class="text-button" disabled={working || security.locked} onclick={() => edit(account)}>Edit / reconnect<span class="sr-only"> {account.settings.name}</span></button>{#if account.configured}<button class="text-button danger-text" disabled={working || security.locked} onclick={() => { if (window.confirm(`Disconnect ${account.settings?.name}? Local drafts will be kept.`)) void run(async () => { await hostService.disconnect(account.id); notice = 'Account disconnected. Local drafts kept.'; }); }}>Disconnect<span class="sr-only"> {account.settings.name}</span></button>{/if}</div>{/if}</li>{/each}</ul>
    </section>
    <section class="settings-card security-card" aria-label="Credential protection"><h2>Credential protection</h2><p class="protection-state">{security.cleanup_pending ? 'Credential cleanup pending' : security.protected ? security.locked ? 'Credentials locked' : 'Password protection enabled' : 'System credential store'}</p><p>By default, passwords and tokens stay in your system credential store. Optionally protect them with a master password.</p><p class="security-disclosure">Protects credentials only. Local emails and drafts remain unencrypted. A forgotten master password cannot be recovered.</p>
      {#if security.cleanup_pending}<p class="error-box">The new credential storage is saved, but the previous system entry still needs removal. Unlock your system keychain and retry.</p><button class="secondary" disabled={working} onclick={() => protect('cleanup')}>Finish credential cleanup</button>{/if}
      <fieldset disabled={working}>
        {#if !security.protected || security.locked}<label class="field">Master password<input type="password" autocomplete={security.locked ? 'current-password' : 'new-password'} minlength={security.locked ? undefined : 12} maxlength="1024" bind:value={password} /></label>
          {#if !security.locked}<label class="field">Confirm master password<input type="password" autocomplete="new-password" bind:value={confirm} maxlength="1024" /></label>{/if}
          <button class="primary" disabled={!password || (!security.locked && (password.length < 12 || !confirm))} onclick={() => protect(security.locked ? 'unlock' : 'protect')}>{working ? 'Working…' : security.locked ? 'Unlock' : 'Enable password protection'}</button>
        {:else}
          <div class="button-row"><button class="secondary" onclick={() => protect('lock')}>Lock credentials</button><button class="text-button" onclick={() => protect('unprotect')}>Use system credential store</button></div>
          <details><summary>Change master password</summary><label class="field">New master password<input type="password" autocomplete="new-password" bind:value={password} maxlength="1024" /></label><label class="field">Confirm new master password<input type="password" autocomplete="new-password" bind:value={confirm} maxlength="1024" /></label><button class="secondary" disabled={password.length < 12 || !confirm} onclick={() => protect('protect')}>Change password</button></details>
        {/if}
        {#if security.protected}<details><summary>Forgot your master password?</summary><p>Reset saved credentials and reconnect your accounts. Local drafts will be kept.</p><button class="text-button danger-text" onclick={() => protect('reset')}>Reset saved credentials</button></details>{/if}
      </fieldset>
    </section>
    <section class="settings-card add-account" aria-label="Account connection settings"><h2>{editing ? 'Edit account' : 'Add an account'}</h2><p>Use your provider’s server settings and an app password when required. POP3 is not supported.</p>
      {#if security.locked}<p class="settings-notice">Unlock credentials to add or reconnect an account.</p>{/if}
      <form onsubmit={save}><fieldset disabled={working || security.locked}><div class="form-grid">
        <label class="field">Account name<input required maxlength="100" autocomplete="off" bind:value={settings.name} placeholder="Work or Personal" /></label>
        <label class="field">Email address<input required type="email" maxlength="320" autocomplete="email" autocapitalize="none" bind:value={settings.address} placeholder="you@example.com" /></label>
        <label class="field">Protocol<select bind:value={settings.protocol}><option value="imap">IMAP + SMTP</option><option value="jmap">JMAP</option></select></label>
        {#if settings.protocol === 'imap'}
          <label class="field">Username<input required maxlength="320" autocomplete="username" autocapitalize="none" bind:value={settings.username} /></label>
          <label class="field">IMAP host<input required maxlength="253" autocapitalize="none" bind:value={settings.imap_host} placeholder="imap.example.com" /></label>
          <label class="field">IMAP port<input required type="number" min="1" max="65535" bind:value={settings.imap_port} /></label>
          <label class="field">SMTP host<input required maxlength="253" autocapitalize="none" bind:value={settings.smtp_host} placeholder="smtp.example.com" /></label>
          <label class="field">SMTP port<select bind:value={settings.smtp_port}><option value={587}>587 · STARTTLS</option><option value={465}>465 · TLS</option></select></label>
        {:else}<label class="field wide-field">JMAP session URL<input required type="url" maxlength="2048" autocapitalize="none" bind:value={settings.session_url} placeholder="https://mail.example.com/.well-known/jmap" /></label>{/if}
        <label class="field wide-field">{settings.protocol === 'imap' ? 'Password or app password' : 'JMAP bearer token'}<input type="password" required={!editing} maxlength="8192" autocomplete="new-password" bind:value={secret} placeholder={editing ? 'Leave blank to keep saved credential' : ''} /></label>
      </div><div class="button-row"><button class="primary" type="submit">{working ? 'Saving…' : 'Save account'}</button>{#if editing}<button class="secondary" type="button" onclick={resetForm}>Cancel editing</button>{/if}</div></fieldset></form>
    </section>
  </div>
</section>
