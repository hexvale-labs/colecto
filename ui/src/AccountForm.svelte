<script lang="ts" module>
  import type { AccountSettings } from './lib/api';
  export const emptyAccount = (): AccountSettings => ({ name: '', address: '', protocol: 'imap', imap_host: '', imap_port: 993, smtp_host: '', smtp_port: 587, username: '', session_url: '', imap_starttls: false, tls_certificate: '' });
</script>
<script lang="ts">
  // The account fields, shared by Settings → Accounts and first-run onboarding.
  import Icon from './Icon.svelte';
  import { hostService } from './lib/api';
  import type { AccountInfo } from './lib/api';
  let { account, working = $bindable(false), submitLabel, oncancel, onsaved, ondirty }: {
    account?: AccountInfo; working?: boolean; submitLabel: string;
    oncancel?: () => void; onsaved: (saved: AccountInfo, name: string) => void | Promise<void>; ondirty?: (value: boolean) => void;
  } = $props();
  // The Host omits unset optional fields, so fill them in to keep the dirty check honest.
  // svelte-ignore state_referenced_locally
  const initial = account?.settings ? { ...emptyAccount(), ...account.settings } : emptyAccount();
  let settings = $state(initial);
  const baseline = JSON.stringify(initial);
  let secret = $state('');
  let reveal = $state(false);
  let error = $state('');
  let dirty = $derived(JSON.stringify(settings) !== baseline || !!secret);
  $effect(() => ondirty?.(dirty));

  // The username follows the address until the owner types a different one.
  let followed = initial.address;
  function follow(address: string) {
    if (!settings.username || settings.username === followed) settings.username = address;
    followed = address;
  }
  /** Switching security moves a default port along with it; a custom port stays. */
  function imapSecurity(starttls: boolean) {
    settings.imap_starttls = starttls;
    if (starttls && settings.imap_port === 993) settings.imap_port = 143;
    if (!starttls && settings.imap_port === 143) settings.imap_port = 993;
  }
  // SMTP security follows the port: 465 starts with TLS, every other port requires STARTTLS.
  function smtpSecurity(tls: boolean) {
    if (tls) settings.smtp_port = 465;
    else if (settings.smtp_port === 465) settings.smtp_port = 587;
  }
  async function chooseCertificate(event: Event & { currentTarget: HTMLInputElement }) {
    const input = event.currentTarget;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    error = '';
    const text = file.size <= 16384 ? await file.text() : '';
    if (text.split('-----BEGIN CERTIFICATE-----').length !== 2) {
      error = 'Choose a PEM file with exactly one certificate (BEGIN CERTIFICATE), at most 16 KB.';
      return;
    }
    settings.tls_certificate = text;
  }
  async function save(event: SubmitEvent) {
    event.preventDefault();
    error = ''; working = true;
    try {
      const saved = await hostService.save({ ...settings, id: account?.id, secret });
      secret = '';
      ondirty?.(false);
      await onsaved(saved, settings.name || settings.address);
    } catch (e) { error = (e as Error).message; }
    finally { working = false; }
  }
</script>

<form class="account-form" onsubmit={save}>
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
        <fieldset class="segmented-field"><legend>Security<span class="sr-only"> for incoming mail</span></legend><div class="segmented"><label><input type="radio" name="imap-security" checked={!settings.imap_starttls} onchange={() => imapSecurity(false)} />TLS</label><label><input type="radio" name="imap-security" checked={settings.imap_starttls} onchange={() => imapSecurity(true)} />STARTTLS</label></div></fieldset>
      </div>
      <div class="server-row">
        <label class="field">Outgoing (SMTP) server<input required maxlength="253" autocapitalize="none" spellcheck="false" bind:value={settings.smtp_host} placeholder="smtp.example.com" /></label>
        <label class="field port">Port<input required type="number" min="1" max="65535" inputmode="numeric" bind:value={settings.smtp_port} /></label>
        <fieldset class="segmented-field"><legend>Security<span class="sr-only"> for outgoing mail</span></legend><div class="segmented"><label title="Port 465"><input type="radio" name="smtp-security" checked={settings.smtp_port === 465} onchange={() => smtpSecurity(true)} />TLS</label><label title="Any port except 465"><input type="radio" name="smtp-security" checked={settings.smtp_port !== 465} onchange={() => smtpSecurity(false)} />STARTTLS</label></div></fieldset>
      </div>
      <details class="certificate" open={!!settings.tls_certificate}>
        <summary>Trusted certificate</summary>
        <div class="certificate-body">
          {#if settings.tls_certificate}
            <span class="certificate-state"><Icon name="shield" size={15} />Custom certificate in use</span>
            <button type="button" class="text-button" onclick={() => settings.tls_certificate = ''}>Use system certificates</button>
          {:else}
            <label class="secondary certificate-pick"><Icon name="upload" size={15} />Choose certificate…<input class="sr-only" type="file" accept=".pem,.crt,.cer,application/x-pem-file" onchange={chooseCertificate} /></label>
          {/if}
        </div>
        <p class="hint">For Proton Mail Bridge or a self-signed server. Both servers must present this certificate, and it replaces your system’s certificate authorities for this account.</p>
      </details>
    {:else}
      <label class="field">JMAP server or session URL<input required type="url" maxlength="2048" autocapitalize="none" spellcheck="false" bind:value={settings.session_url} placeholder="https://mail.example.com" /></label>
    {/if}
    <div class="form-grid">
      <label class="field"><span>Username{#if settings.protocol === 'jmap'} <span class="optional">(optional)</span>{/if}</span><input required={settings.protocol === 'imap'} maxlength="320" autocomplete="username" autocapitalize="none" spellcheck="false" bind:value={settings.username} /></label>
      <label class="field">{settings.protocol === 'jmap' && !settings.username ? 'API token' : 'Password'}
        <span class="secret-input"><input type={reveal ? 'text' : 'password'} required={!account} maxlength="8192" autocomplete="new-password" bind:value={secret} placeholder={account ? 'Unchanged' : ''} /><button type="button" class="reveal" aria-label={reveal ? 'Hide password' : 'Show password'} aria-pressed={reveal} onclick={() => reveal = !reveal}><Icon name={reveal ? 'eye-off' : 'eye'} size={15} /></button></span>
      </label>
    </div>
    {#if settings.protocol === 'jmap'}<p class="hint">Leave the username empty to sign in with an API token instead of a password.</p>{/if}
    {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  </fieldset>
  <footer class="sheet-foot">
    {#if oncancel}<button type="button" class="secondary" disabled={working} onclick={oncancel}>Cancel</button>{/if}
    <button class="primary" type="submit" disabled={working}>{working ? 'Saving…' : submitLabel}</button>
  </footer>
</form>
