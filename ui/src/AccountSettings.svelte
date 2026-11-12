<script lang="ts">
  import Icon from './Icon.svelte';
  import { createServices, hostService } from './lib/api';
  import type { AccountInfo, AccountSettings, SecurityInfo } from './lib/api';
  import { hue, initial } from './lib/initial';
  let { accounts, security, onrefresh, ondirty, onbusy }: {
    accounts: AccountInfo[]; security: SecurityInfo; onrefresh: () => Promise<void>;
    ondirty: (value: boolean) => void; onbusy: (value: boolean) => void;
  } = $props();
  const empty = (): AccountSettings => ({ name: '', address: '', protocol: 'imap', imap_host: '', imap_port: 993, smtp_host: '', smtp_port: 587, username: '', session_url: '' });
  let dialog = $state<HTMLDialogElement>();
  let settings = $state(empty());
  let baseline = $state(JSON.stringify(empty()));
  let editing = $state<string | undefined>();
  let secret = $state('');
  let reveal = $state(false);
  let formError = $state('');
  let password = $state('');
  let confirm = $state('');
  let working = $state(false);
  let checking = $state<string | undefined>();
  let error = $state('');
  let notice = $state('');
  let dirty = $derived(JSON.stringify(settings) !== baseline || !!secret || !!password || !!confirm);
  $effect(() => ondirty(dirty));

  const title = (account: AccountInfo) => account.settings?.name || account.settings?.address || 'Demo account';
  /** The second line names what the first does not: the address under a name, otherwise the server. */
  function detail(account: AccountInfo) {
    const s = account.settings;
    if (!s) return 'Sample mail, stored on this device';
    if (s.name) return s.address;
    if (s.protocol === 'jmap') { try { return new URL(s.session_url).host; } catch { return s.session_url; } }
    return s.imap_host;
  }
  function status(account: AccountInfo): { text: string; tone: 'on' | 'off' | 'warn' } {
    if (!account.settings) return { text: 'Sample', tone: 'off' };
    if (security.locked) return { text: 'Locked', tone: 'warn' };
    if (!account.configured) return { text: 'Signed out', tone: 'warn' };
    return account.connected ? { text: 'Connected', tone: 'on' } : { text: 'Not connected', tone: 'off' };
  }

  // The username follows the address until the owner types a different one.
  let followed = '';
  function follow(address: string) {
    if (!settings.username || settings.username === followed) settings.username = address;
    followed = address;
  }
  function open(account?: AccountInfo) {
    if (dirty && !window.confirm('Discard unsaved account settings?')) return;
    settings = account?.settings ? { ...account.settings } : empty();
    baseline = JSON.stringify(settings); followed = settings.address; editing = account?.id; secret = ''; reveal = false; formError = ''; notice = '';
    dialog?.showModal();
  }
  function close() {
    if (working || (dirty && !window.confirm('Discard unsaved account settings?'))) return;
    settings = empty(); baseline = JSON.stringify(settings); editing = undefined; secret = ''; formError = '';
    dialog?.close();
  }
  async function run(action: () => Promise<unknown>, onerror = (message: string) => { error = message; }) {
    working = true; onbusy(true); error = ''; notice = '';
    try { await action(); }
    catch (e) { onerror((e as Error).message); }
    finally {
      // Reload authoritative state even if post-commit credential cleanup failed.
      try { await onrefresh(); } catch (e) { error ||= (e as Error).message; }
      working = false; onbusy(false);
    }
  }
  async function save(event: SubmitEvent) {
    event.preventDefault();
    formError = '';
    await run(async () => {
      await hostService.save({ ...settings, id: editing, secret });
      const name = settings.name || settings.address;
      baseline = JSON.stringify(settings); secret = '';
      dialog?.close(); editing = undefined;
      notice = `${name} saved. Colecto checks the connection in the background.`;
    }, message => formError = message);
  }
  /** Reconnect asks the provider for a fresh Inbox; a signed-out account needs its password first. */
  async function reconnect(account: AccountInfo) {
    if (!account.configured) { open(account); return; }
    checking = account.id;
    await run(async () => {
      await createServices(account.id).inbox.messages('inbox');
      notice = `${title(account)} is connected.`;
    });
    checking = undefined;
  }
  async function remove(account: AccountInfo) {
    if (!window.confirm(`Remove ${title(account)} from Colecto? Its saved password, cached mail and local drafts are deleted from this device. Nothing changes on the mail server.`)) return;
    await run(async () => { await hostService.remove(account.id); notice = `${title(account)} removed.`; });
  }
  async function protect(action: string) {
    if (action === 'protect' && password !== confirm) { error = 'Master passwords do not match'; return; }
    if (action === 'reset' && !window.confirm('Forget all saved mail credentials? You must reconnect every account. Local drafts will be kept.')) return;
    await run(async () => {
      await hostService.securityAction(action, password);
      password = ''; confirm = '';
      notice = action === 'reset' ? 'Credentials reset. Reconnect your accounts.' : '';
    });
  }
</script>

<section class="account-settings" aria-labelledby="settings-title">
  <header class="section-head"><h2 id="settings-title">Accounts &amp; security</h2><p>Each account keeps its own mail and drafts. Credentials stay in your system’s credential store unless you protect them with a master password.</p></header>
  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  {#if notice}<p class="settings-notice" role="status">{notice}</p>{/if}
  <div class="settings-stack">
    <section class="settings-card" aria-labelledby="accounts-title">
      <h2 id="accounts-title">Mail accounts</h2>
      <ul class="account-rows">
        {#each accounts as account (account.id)}
          {@const state = status(account)}
          <li class="account-row">
            <span class="avatar" style:--hue={hue(title(account))} aria-hidden="true">{initial(title(account))}</span>
            <div class="account-text">
              <span class="account-line"><strong title={title(account)}>{title(account)}</strong>{#if account.settings}<span class="protocol-tag">{account.settings.protocol === 'imap' ? 'IMAP' : 'JMAP'}</span>{/if}</span>
              <span class="account-line meta"><span class="status-dot {state.tone}" class:on={state.tone === 'on'} aria-hidden="true"></span><span>{state.text}</span><span class="sep" aria-hidden="true">·</span><span class="account-detail">{detail(account)}</span></span>
            </div>
            {#if account.settings}
              <div class="row-actions">
                <button class="icon-button" class:spinning={checking === account.id} aria-label={`Reconnect ${title(account)}`} title={account.configured ? 'Reconnect' : 'Sign in again'} disabled={working || security.locked} onclick={() => reconnect(account)}><Icon name="refresh" size={16} /></button>
                <button class="icon-button" aria-label={`Edit ${title(account)}`} title="Edit" disabled={working || security.locked} onclick={() => open(account)}><Icon name="pencil" size={16} /></button>
                <button class="icon-button remove" aria-label={`Remove ${title(account)}`} title="Remove" disabled={working || security.locked} onclick={() => remove(account)}><Icon name="trash" size={16} /></button>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
      <button class="secondary add-account" disabled={working || security.locked} onclick={() => open()}><Icon name="plus" size={16} />Add account</button>
      {#if security.locked}<p class="hint">Unlock credentials below to add or change accounts.</p>{/if}
    </section>

    <section class="settings-card security-card" aria-labelledby="protection-title"><h2 id="protection-title">Credential protection</h2><p class="protection-state">{security.cleanup_pending ? 'Credential cleanup pending' : security.protected ? security.locked ? 'Credentials locked' : 'Password protection enabled' : 'System credential store'}</p><p>By default, passwords and tokens stay in your system credential store. Optionally protect them with a master password.</p><p class="security-disclosure">Protects credentials only. Local emails and drafts remain unencrypted. A forgotten master password cannot be recovered.</p>
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
  </div>
</section>

<dialog bind:this={dialog} class="sheet account-dialog" aria-labelledby="account-dialog-title" oncancel={(event) => { event.preventDefault(); close(); }}>
  <form onsubmit={save}>
    <header class="sheet-head">
      <div><h2 id="account-dialog-title">{editing ? 'Edit account' : 'Add an account'}</h2><p>Use your provider’s server settings, and an app password if it asks for one. POP3 is not supported.</p></div>
      <button type="button" class="icon-button" aria-label="Close" onclick={close}><Icon name="close" size={16} /></button>
    </header>
    <fieldset class="sheet-body" disabled={working}>
      <div class="form-grid">
        <label class="field">Email address<input required type="email" maxlength="320" autocomplete="email" autocapitalize="none" bind:value={settings.address} oninput={(event) => follow(event.currentTarget.value)} placeholder="you@example.com" /></label>
        <label class="field"><span>Name <span class="optional">(optional)</span></span><input maxlength="100" autocomplete="off" bind:value={settings.name} placeholder="Work, Personal…" /></label>
      </div>
      <fieldset class="segmented-field">
        <legend>Protocol</legend>
        <div class="segmented fill"><label><input type="radio" name="protocol" value="imap" bind:group={settings.protocol} />IMAP + SMTP</label><label><input type="radio" name="protocol" value="jmap" bind:group={settings.protocol} />JMAP</label></div>
      </fieldset>
      {#if settings.protocol === 'imap'}
        <div class="server-row">
          <label class="field">Incoming (IMAP) server<input required maxlength="253" autocapitalize="none" spellcheck="false" bind:value={settings.imap_host} placeholder="imap.example.com" /></label>
          <label class="field port">Port<input required type="number" min="1" max="65535" inputmode="numeric" bind:value={settings.imap_port} /></label>
        </div>
        <div class="server-row">
          <label class="field">Outgoing (SMTP) server<input required maxlength="253" autocapitalize="none" spellcheck="false" bind:value={settings.smtp_host} placeholder="smtp.example.com" /></label>
          <fieldset class="segmented-field"><legend>Security</legend><div class="segmented"><label title="Port 587"><input type="radio" name="smtp-port" value={587} bind:group={settings.smtp_port} />STARTTLS</label><label title="Port 465"><input type="radio" name="smtp-port" value={465} bind:group={settings.smtp_port} />TLS</label></div></fieldset>
        </div>
      {:else}
        <label class="field">JMAP server or session URL<input required type="url" maxlength="2048" autocapitalize="none" spellcheck="false" bind:value={settings.session_url} placeholder="https://mail.example.com" /></label>
      {/if}
      <div class="form-grid">
        <label class="field"><span>Username{#if settings.protocol === 'jmap'} <span class="optional">(optional)</span>{/if}</span><input required={settings.protocol === 'imap'} maxlength="320" autocomplete="username" autocapitalize="none" spellcheck="false" bind:value={settings.username} /></label>
        <label class="field">{settings.protocol === 'jmap' && !settings.username ? 'API token' : 'Password'}
          <span class="secret-input"><input type={reveal ? 'text' : 'password'} required={!editing} maxlength="8192" autocomplete="new-password" bind:value={secret} placeholder={editing ? 'Unchanged' : ''} /><button type="button" class="reveal" aria-label={reveal ? 'Hide password' : 'Show password'} aria-pressed={reveal} onclick={() => reveal = !reveal}><Icon name={reveal ? 'eye-off' : 'eye'} size={15} /></button></span>
        </label>
      </div>
      {#if settings.protocol === 'jmap'}<p class="hint">Leave the username empty to sign in with an API token instead of a password.</p>{/if}
      {#if formError}<p class="error-box" role="alert">{formError}</p>{/if}
    </fieldset>
    <footer class="sheet-foot">
      <button type="button" class="secondary" disabled={working} onclick={close}>Cancel</button>
      <button class="primary" type="submit" disabled={working}>{working ? 'Saving…' : editing ? 'Save changes' : 'Save account'}</button>
    </footer>
  </form>
</dialog>
