<script lang="ts">
  import Icon from './Icon.svelte';
  import { hostService } from './lib/api';
  import type { AccountInfo, AccountSettings, SecurityInfo } from './lib/api';
  let { accounts, security, onrefresh, ondirty, onbusy }: {
    accounts: AccountInfo[]; security: SecurityInfo; onrefresh: () => Promise<void>;
    ondirty: (value: boolean) => void; onbusy: (value: boolean) => void;
  } = $props();
  const empty = (): AccountSettings => ({ name: '', address: '', protocol: 'imap', imap_host: '', imap_port: 993, smtp_host: '', smtp_port: 587, username: '', session_url: '' });
  let dialog = $state<HTMLDialogElement>();
  let reveal = $state(false);
  function closeForm() {
    if (working || (dirty && !window.confirm('Discard unsaved account settings?'))) return;
    dialog?.close(); resetForm();
  }
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
    settings = { ...account.settings }; baseline = JSON.stringify(settings); editing = account.id; reveal = false; secret = ''; notice = ''; error = ''; dialog?.showModal();
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
      dialog?.close(); resetForm(); notice = 'Account saved. Open Inbox to check the connection.';
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
<section class="account-settings" aria-labelledby="settings-title">
  <header class="section-head"><h2 id="settings-title">Accounts &amp; security</h2><p>Each account keeps its own mail and drafts. Credentials stay in your system’s credential store unless you protect them with a master password.</p></header>
  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  {#if notice}<p class="settings-notice" role="status">{notice}</p>{/if}
  <div class="settings-stack">
    <section class="settings-card" aria-label="Saved accounts"><h2>Mail accounts</h2><p>Inbox and drafts stay with their account.</p>
      <ul class="account-list compact-accounts">{#each accounts as account (account.id)}
        <li>
          <span class="settings-avatar" aria-hidden="true">{Array.from(account.settings?.name || account.settings?.address || 'Demo')[0]}</span>
          <div class="account-copy"><strong>{account.settings?.name || account.settings?.address || 'Demo account'}</strong>
            <span>{account.settings?.address ?? 'Sample mail · local drafts'}</span>
            <small><i class:connected={account.connected} class="account-dot"></i>{security.locked ? 'Locked' : account.connected ? 'Connected' : account.configured ? 'Not checked · open Inbox' : 'Disconnected · drafts kept'}
              {#if account.settings}<span class="protocol-tag" title={account.settings.protocol === 'imap' ? account.settings.imap_host + ' / ' + account.settings.smtp_host : account.settings.session_url}>{account.settings.protocol === 'imap' ? 'IMAP/SMTP' : 'JMAP'}</span>{/if}
            </small>
          </div>
          {#if account.settings}<div class="account-actions">
            <button class="icon-button" title="Reconnect" aria-label="Reconnect" disabled={working || security.locked} onclick={() => edit(account)}><Icon name="refresh" /></button>
            <button class="icon-button" title="Edit" aria-label="Edit" disabled={working || security.locked} onclick={() => edit(account)}><Icon name="pencil" /></button>
            <button class="icon-button danger-text" title="Remove" aria-label="Remove" disabled={working || security.locked || !account.configured} onclick={() => { if (window.confirm(`Remove saved credentials for ${account.settings?.name || account.settings?.address}? This disconnects the account. Local mail and drafts will be kept.`)) void run(async () => { await hostService.disconnect(account.id); notice = 'Account disconnected. Local drafts kept.'; }); }}><Icon name="trash" /></button>
          </div>{/if}
        </li>
      {/each}</ul>
      <button class="secondary" disabled={working || security.locked} onclick={() => { resetForm(); reveal = false; dialog?.showModal(); }}><Icon name="plus" />Add account</button>
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
    <dialog bind:this={dialog} class="account-dialog" aria-labelledby="account-dialog-title" oncancel={(event) => { event.preventDefault(); closeForm(); }}><h2 id="account-dialog-title" class="sr-only">Account connection settings</h2><h2>{editing ? 'Edit account' : 'Add an account'}</h2><p>Use your provider’s server settings and an app password when required. POP3 is not supported.</p>
      {#if security.locked}<p class="settings-notice">Unlock credentials to add or reconnect an account.</p>{/if}
      {#if error}<p class="error-box" role="alert">{error}</p>{/if}<form onsubmit={save}><fieldset disabled={working || security.locked}><h3>Identity</h3><div class="form-grid">
        <label class="field">Account name (optional)<input maxlength="100" autocomplete="off" bind:value={settings.name} placeholder="Work or Personal" /></label>
        <label class="field">Email address<input required type="email" maxlength="320" autocomplete="email" autocapitalize="none" bind:value={settings.address} oninput={(event) => { if (!settings.username || settings.username === settings.address) settings.username = event.currentTarget.value; }} placeholder="you@example.com" /></label>
        </div><h3>Server</h3><div class="form-grid"><fieldset class="field segmented-field wide-field"><legend>Protocol</legend><div class="segmented"><label><input type="radio" name="protocol" value="imap" bind:group={settings.protocol} />IMAP + SMTP</label><label><input type="radio" name="protocol" value="jmap" bind:group={settings.protocol} />JMAP</label></div></fieldset>
        {#if settings.protocol === 'imap'}
          <label class="field">IMAP host<input required maxlength="253" autocapitalize="none" bind:value={settings.imap_host} placeholder="imap.example.com" /></label>
          <label class="field">IMAP port<input required type="number" min="1" max="65535" bind:value={settings.imap_port} /></label>
          <label class="field">SMTP host<input required maxlength="253" autocapitalize="none" bind:value={settings.smtp_host} placeholder="smtp.example.com" /></label>
          <fieldset class="field segmented-field"><legend>SMTP port</legend><div class="segmented"><label><input type="radio" name="smtp-port" value={587} bind:group={settings.smtp_port} />587 STARTTLS</label><label><input type="radio" name="smtp-port" value={465} bind:group={settings.smtp_port} />465 TLS</label></div></fieldset>
        {:else}<label class="field wide-field">JMAP server or session URL<input required type="url" maxlength="2048" autocapitalize="none" bind:value={settings.session_url} placeholder="https://mail.example.com" /></label>
{/if}
        </div><h3>Sign in</h3><div class="form-grid"><label class="field wide-field">Username<input required={settings.protocol === 'imap'} maxlength="320" autocomplete="username" bind:value={settings.username} placeholder="Leave empty for a JMAP API token" /></label><label class="field wide-field">{settings.protocol === 'imap' ? 'Password or app password' : settings.username ? 'Password or app password' : 'API token'}<input type={reveal ? 'text' : 'password'} required={!editing} maxlength="8192" autocomplete="new-password" bind:value={secret} placeholder={editing ? 'Leave blank to keep saved credential' : ''} /></label>
      </div><label class="choice"><input type="checkbox" bind:checked={reveal} />Show password</label><div class="button-row"><button class="secondary" type="button" onclick={closeForm}>Cancel</button><button class="primary" type="submit">{working ? 'Saving…' : 'Save account'}</button></div></fieldset></form>
    </dialog>
  </div>
</section>
