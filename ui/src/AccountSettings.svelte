<script lang="ts">
  import Icon from './Icon.svelte';
  import AccountForm from './AccountForm.svelte';
  import { createServices, hostService } from './lib/api';
  import type { AccountInfo, SecurityInfo } from './lib/api';
  import { hue, initial } from './lib/initial';
  let { accounts, security, enabled, ontoggle, onrefresh, ondirty, onbusy }: {
    accounts: AccountInfo[]; security: SecurityInfo; onrefresh: () => Promise<void>;
    /** Whether each account is on; off accounts are hidden from the workspace and not synced. */
    enabled: (id: string) => boolean; ontoggle: (id: string, on: boolean) => void;
    ondirty: (value: boolean) => void; onbusy: (value: boolean) => void;
  } = $props();
  let dialog = $state<HTMLDialogElement>();
  let editing = $state<AccountInfo | undefined>();
  // Each open mounts a fresh form, so a cancelled edit never leaks into the next one.
  let session = $state(0);
  let formDirty = $state(false);
  let saving = $state(false);
  let password = $state('');
  let confirm = $state('');
  let working = $state(false);
  let checking = $state<string | undefined>();
  let error = $state('');
  let notice = $state('');
  let dirty = $derived(formDirty || !!password || !!confirm);
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
    if (!enabled(account.id)) return { text: 'Off', tone: 'off' };
    if (!account.settings) return { text: 'Sample', tone: 'off' };
    if (security.locked) return { text: 'Locked', tone: 'warn' };
    if (!account.configured) return { text: 'Signed out', tone: 'warn' };
    return account.connected ? { text: 'Connected', tone: 'on' } : { text: 'Not connected', tone: 'off' };
  }
  function open(account?: AccountInfo) {
    if (formDirty && !window.confirm('Discard unsaved account settings?')) return;
    editing = account; formDirty = false; notice = ''; session += 1;
    dialog?.showModal();
  }
  function close() {
    if (saving || (formDirty && !window.confirm('Discard unsaved account settings?'))) return;
    formDirty = false; editing = undefined;
    dialog?.close();
  }
  async function saved(_: AccountInfo, name: string) {
    formDirty = false; dialog?.close(); editing = undefined;
    notice = `${name} saved. Colecto checks the connection in the background.`;
    try { await onrefresh(); } catch (e) { error = (e as Error).message; }
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
          <li class="account-row" class:off={!enabled(account.id)}>
            <span class="avatar" style:--hue={hue(title(account))} aria-hidden="true">{initial(title(account))}</span>
            <div class="account-text">
              <span class="account-line"><strong title={title(account)}>{title(account)}</strong>{#if account.settings}<span class="protocol-tag">{account.settings.protocol === 'imap' ? 'IMAP' : 'JMAP'}</span>{/if}</span>
              <span class="account-line meta"><span class="status-dot {state.tone}" class:on={state.tone === 'on'} aria-hidden="true"></span><span>{state.text}</span><span class="sep" aria-hidden="true">·</span><span class="account-detail">{detail(account)}</span></span>
            </div>
            <div class="row-actions">
            {#if account.settings}
                <button class="icon-button" class:spinning={checking === account.id} aria-label={`Reconnect ${title(account)}`} title={account.configured ? 'Reconnect' : 'Sign in again'} disabled={working || security.locked} onclick={() => reconnect(account)}><Icon name="refresh" size={16} /></button>
                <button class="icon-button" aria-label={`Edit ${title(account)}`} title="Edit" disabled={working || security.locked} onclick={() => open(account)}><Icon name="pencil" size={16} /></button>
                <button class="icon-button remove" aria-label={`Remove ${title(account)}`} title="Remove" disabled={working || security.locked} onclick={() => remove(account)}><Icon name="trash" size={16} /></button>
            {/if}
              <label class="switch" title={enabled(account.id) ? 'On: shown and synced' : 'Off: hidden and not synced'}><input type="checkbox" role="switch" aria-label={`Use ${title(account)}`} checked={enabled(account.id)} disabled={working} onchange={(event) => ontoggle(account.id, event.currentTarget.checked)} /></label>
            </div>
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
  <header class="sheet-head">
    <div><h2 id="account-dialog-title">{editing ? 'Edit account' : 'Add an account'}</h2><p>Use your provider’s server settings, and an app password if it asks for one. POP3 is not supported.</p></div>
    <button type="button" class="icon-button" aria-label="Close" onclick={close}><Icon name="close" size={16} /></button>
  </header>
  {#key session}<AccountForm account={editing} bind:working={saving} submitLabel={editing ? 'Save changes' : 'Save account'} oncancel={close} onsaved={saved} ondirty={(value) => formDirty = value} />{/key}
</dialog>
