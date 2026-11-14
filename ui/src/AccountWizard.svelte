<script lang="ts">
  // Adding or editing a mail account in three steps: email address (with a settings
  // lookup), server settings, and a review. Used by Settings → Accounts and onboarding.
  // Going back never loses what the owner typed: a new address only changes fields a
  // lookup filled, never ones the owner edited.
  import { tick } from 'svelte';
  import Icon from './Icon.svelte';
  import PortPicker from './PortPicker.svelte';
  import { hostService } from './lib/api';
  import type { AccountInfo, AccountSettings } from './lib/api';
  import { applyLookup, blankServers, domainOf, joinJmap, reviewWarnings, sourceSentence, splitJmap } from './lib/discovery';
  import type { Discovery, Field, ServerFields } from './lib/discovery';
  import { hue, initial } from './lib/initial';
  let { account, working = $bindable(false), submitLabel, oncancel, onsaved, ondirty }: {
    account?: AccountInfo; working?: boolean; submitLabel: string;
    oncancel?: () => void; onsaved: (saved: AccountInfo, name: string) => void | Promise<void>; ondirty?: (value: boolean) => void;
  } = $props();

  type Step = 'email' | 'servers' | 'review';
  const steps: { id: Step; label: string }[] = [{ id: 'email', label: 'Email' }, { id: 'servers', label: 'Servers' }, { id: 'review', label: 'Review' }];
  // svelte-ignore state_referenced_locally
  const saved = account?.settings;
  const jmap = splitJmap(saved?.session_url ?? '');
  const initialFields: ServerFields = saved
    ? { protocol: saved.protocol, imap_host: saved.imap_host, imap_port: saved.imap_port, imap_starttls: !!saved.imap_starttls, smtp_host: saved.smtp_host, smtp_port: saved.smtp_port, jmap_host: jmap.host, jmap_port: jmap.port, username: saved.username }
    : { ...blankServers(), username: '' };
  let step = $state<Step>(saved ? 'servers' : 'email');
  let direction = $state(1);
  let address = $state(saved?.address ?? '');
  let name = $state(saved?.name ?? '');
  let fields = $state<ServerFields>({ ...initialFields });
  let certificate = $state(saved?.tls_certificate ?? '');
  let secret = $state('');
  let reveal = $state(false);
  let error = $state('');

  // An existing account counts as edited everywhere, so a lookup never rewrites it.
  const touched = new Set<Field>(saved ? Object.keys(initialFields) as Field[] : []);
  /** The lookup behind the current page 2 values, for the address it ran for. */
  let lookup = $state<{ address: string; found: Discovery | null; kept: Field[] } | null>(saved ? { address: saved.address, found: null, kept: [] } : null);
  let looking = $state(false);
  let attempt = 0;
  /** Lookups by domain, so editing only the part before @ does not ask again. */
  const cache = new Map<string, { address: string; found: Discovery | null }>();

  const settings = (): AccountSettings => ({ name, address: address.trim(), protocol: fields.protocol, imap_host: fields.imap_host.trim(), imap_port: fields.imap_port, imap_starttls: fields.imap_starttls, smtp_host: fields.smtp_host.trim(), smtp_port: fields.smtp_port, username: fields.username.trim(), session_url: jmapSession(), tls_certificate: certificate });
  /** An unchanged JMAP address keeps its saved spelling, so opening an account never makes it dirty. */
  function jmapSession() {
    if (fields.protocol !== 'jmap') return saved?.session_url ?? '';
    if (saved && fields.jmap_host === initialFields.jmap_host && fields.jmap_port === initialFields.jmap_port) return saved.session_url;
    return joinJmap(fields.jmap_host, fields.jmap_port);
  }
  const baseline = JSON.stringify(settings());
  let dirty = $derived(JSON.stringify(settings()) !== baseline || !!secret);
  $effect(() => ondirty?.(dirty));

  let body = $state<HTMLElement>();
  let form = $state<HTMLFormElement>();
  async function go(next: Step, focus?: string) {
    direction = steps.findIndex(s => s.id === next) >= steps.findIndex(s => s.id === step) ? 1 : -1;
    step = next; error = '';
    await tick();
    body?.scrollTo({ top: 0 });
    const target = focus ? document.getElementById(focus) : body?.querySelector<HTMLElement>(next === 'email' ? 'input' : 'h3');
    target?.focus();
    if (focus) target?.scrollIntoView({ block: 'center' });
  }
  function touch(...names: Field[]) { for (const field of names) touched.add(field); }

  const limited = $derived(lookup?.address === address.trim() && (lookup.found?.limitation === 'oauth_only' || lookup.found?.limitation === 'no_access'));
  const lookupDomain = $derived(domainOf(address));
  async function findSettings() {
    const current = address.trim();
    if (lookup?.address === current) { if (!limited) await go('servers'); return; }
    const domain = domainOf(current);
    let cached = cache.get(domain);
    if (!cached) {
      const mine = ++attempt;
      looking = true;
      let result: Discovery | null = null;
      try { result = await hostService.discover(current); } catch { /* A failed lookup is the same as finding nothing. */ }
      if (mine !== attempt) return;
      looking = false;
      cached = { address: current, found: result };
      cache.set(domain, cached);
    }
    let found = cached.found;
    // A lookup reused from another address on the same domain carries that address's username.
    const localPart = (value: string) => value.slice(0, value.lastIndexOf('@'));
    if (found?.username === cached.address) found = { ...found, username: current };
    else if (found?.username && found.username === localPart(cached.address)) found = { ...found, username: localPart(current) };
    apply(current, found);
    if (!limited) await go('servers');
  }
  function apply(current: string, found: Discovery | null) {
    applyLookup(fields, found, current, touched);
    const kept = found ? [...touched].filter(field => field !== 'username') : [];
    lookup = { address: current, found, kept };
  }
  /** Skip or bypass the lookup; page 2 keeps whatever is already there. */
  async function manual() {
    attempt++; looking = false;
    const current = address.trim();
    if (lookup?.address !== current) apply(current, null);
    await go('servers');
  }

  function imapSecurity(starttls: boolean) {
    fields.imap_starttls = starttls;
    if (starttls && fields.imap_port === 993) fields.imap_port = 143;
    if (!starttls && fields.imap_port === 143) fields.imap_port = 993;
    touch('imap_starttls', 'imap_port');
  }
  // SMTP security follows the port: 465 starts with TLS, every other port requires STARTTLS.
  function smtpSecurity(tls: boolean) {
    if (tls) fields.smtp_port = 465;
    else if (fields.smtp_port === 465) fields.smtp_port = 587;
    touch('smtp_port');
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
    certificate = text;
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (step === 'email') return findSettings();
    if (step === 'servers') return go('review');
    error = ''; working = true;
    try {
      const value = settings();
      const stored = await hostService.save({ ...value, id: account?.id, secret });
      secret = '';
      ondirty?.(false);
      await onsaved(stored, value.name || value.address);
    } catch (e) { error = (e as Error).message; }
    finally { working = false; }
  }
  function back() {
    if (step === 'review') return go('servers');
    if (step === 'servers') return go('email');
  }

  const found = $derived(lookup?.address === address.trim() ? lookup.found : null);
  const showFound = $derived(!!found && (!!found.imap || !!found.jmap));
  const fieldNames: Partial<Record<Field, string>> = { protocol: 'protocol', imap_host: 'incoming server', imap_port: 'incoming port', imap_starttls: 'incoming security', smtp_host: 'outgoing server', smtp_port: 'outgoing port', jmap_host: 'JMAP server', jmap_port: 'JMAP port' };
  const keptText = $derived(lookup?.kept.length ? `Your changes to the ${new Intl.ListFormat('en').format([...new Set(lookup.kept.map(field => fieldNames[field]).filter(Boolean) as string[])])} were kept.` : '');
  const warnings = $derived(reviewWarnings(fields));
  const security = (starttls: boolean) => starttls ? 'STARTTLS' : 'TLS';
  const passwordLabel = $derived(fields.protocol === 'jmap' && !fields.username ? 'API token' : found?.app_password ? 'App password' : 'Password');
  const imapPorts = [{ port: 993, hint: 'TLS' }, { port: 143, hint: 'STARTTLS' }];
  const smtpPorts = [{ port: 465, hint: 'TLS' }, { port: 587, hint: 'STARTTLS' }, { port: 25, hint: 'STARTTLS, often blocked' }];
  const jmapPorts = [{ port: 443, hint: 'HTTPS' }, { port: 8443, hint: 'HTTPS, alternate' }];
  const jmapUrl = $derived(jmapSession());
</script>

<form class="account-wizard" bind:this={form} onsubmit={submit}>
  <ol class="wizard-steps" aria-label="Steps">
    {#each steps as item, index (item.id)}
      {@const at = steps.findIndex(s => s.id === step)}
      <li class:current={item.id === step} class:done={index < at} aria-current={item.id === step ? 'step' : undefined}>
        <span class="step-dot" aria-hidden="true">{#if index < at}<Icon name="check" size={11} />{:else}{index + 1}{/if}</span>{item.label}
      </li>
    {/each}
  </ol>
  <div class="sheet-body wizard-body" bind:this={body}>
    {#key step}
    <fieldset class="wizard-step" style:--dir={direction} disabled={working}>
    {#if step === 'email'}
      <label class="field">Email address<input required readonly={looking} type="email" maxlength="320" autocomplete="email" autocapitalize="none" spellcheck="false" bind:value={address} placeholder="you@example.com" /></label>
      {#if looking}
        <div class="lookup" role="status"><span class="spinner" aria-hidden="true"></span><span>Looking up settings for <strong>{lookupDomain}</strong>…</span><button type="button" class="text-button" onclick={manual}>Skip</button></div>
      {:else if limited && lookup?.found}
        <div class="limit-note" role="alert">
          <Icon name="alert" size={16} />
          <div>
            {#if lookup.found.limitation === 'oauth_only'}<p><strong>{lookup.found.provider ?? 'This provider'} only allows OAuth sign-in.</strong> Colecto signs in with a password or app password, and does not support OAuth yet.</p>
            {:else}<p><strong>{lookup.found.provider ?? 'This provider'} has no IMAP, SMTP or JMAP access</strong>, so mail apps like Colecto cannot connect to it.</p>{/if}
            <button type="button" class="text-button" onclick={() => go('servers')}>Enter settings anyway</button>
          </div>
        </div>
      {:else}
        <p class="hint lookup-hint">Colecto looks up your provider’s server settings from the address. For lesser-known domains, only the domain is sent to Thunderbird’s provider database.</p>
        <button type="button" class="text-button manual" onclick={() => form?.reportValidity() && manual()}>Enter settings myself</button>
      {/if}
    {:else if step === 'servers'}
      <h3 class="step-title sr-only" tabindex="-1">Server settings</h3>
      {#if showFound && found}
        <div class="found-note" role="status">
          <Icon name="check" size={16} />
          <div>
            <p><strong>{found.provider ? `Settings found for ${found.provider}` : 'Settings found'}</strong></p>
            <p>{sourceSentence(found, lookupDomain)} Check them before you continue.</p>
            {#if found.limitation === 'bridge'}<p>{found.provider} works through Proton Mail Bridge. Start Bridge, then use the password and certificate it shows.</p>{/if}
            {#if keptText}<p>{keptText}</p>{/if}
          </div>
        </div>
      {/if}
      <fieldset class="segmented-field">
        <legend>Protocol</legend>
        <div class="segmented fill"><label><input type="radio" name="protocol" value="imap" bind:group={fields.protocol} onchange={() => touch('protocol')} />IMAP + SMTP</label><label><input type="radio" name="protocol" value="jmap" bind:group={fields.protocol} onchange={() => touch('protocol')} />JMAP</label></div>
      </fieldset>
      {#if fields.protocol === 'imap'}
        <section class="server-block" aria-labelledby="incoming-title">
          <h4 id="incoming-title"><Icon name="inbox" size={15} />Incoming mail</h4>
          <label class="field">Incoming (IMAP) server<input id="wizard-imap-host" required maxlength="253" autocapitalize="none" spellcheck="false" bind:value={fields.imap_host} oninput={() => touch('imap_host')} placeholder="imap.example.com" /></label>
          <div class="port-row">
            <PortPicker id="wizard-imap-port" label="IMAP port" value={fields.imap_port} options={imapPorts} onpick={port => { fields.imap_port = port; touch('imap_port'); if (port === 993 || port === 143) { fields.imap_starttls = port === 143; touch('imap_starttls'); } }} />
            <fieldset class="segmented-field"><legend>Security<span class="sr-only"> for incoming mail</span></legend><div class="segmented"><label><input type="radio" name="imap-security" checked={!fields.imap_starttls} onchange={() => imapSecurity(false)} />TLS</label><label><input type="radio" name="imap-security" checked={fields.imap_starttls} onchange={() => imapSecurity(true)} />STARTTLS</label></div></fieldset>
          </div>
        </section>
        <section class="server-block" aria-labelledby="outgoing-title">
          <h4 id="outgoing-title"><Icon name="sent" size={15} />Outgoing mail</h4>
          <label class="field">Outgoing (SMTP) server<input id="wizard-smtp-host" required maxlength="253" autocapitalize="none" spellcheck="false" bind:value={fields.smtp_host} oninput={() => touch('smtp_host')} placeholder="smtp.example.com" /></label>
          <div class="port-row">
            <PortPicker id="wizard-smtp-port" label="SMTP port" value={fields.smtp_port} options={smtpPorts} onpick={port => { fields.smtp_port = port; touch('smtp_port'); }} />
            <fieldset class="segmented-field"><legend>Security<span class="sr-only"> for outgoing mail</span></legend><div class="segmented"><label title="Port 465"><input type="radio" name="smtp-security" checked={fields.smtp_port === 465} onchange={() => smtpSecurity(true)} />TLS</label><label title="Any port except 465"><input type="radio" name="smtp-security" checked={fields.smtp_port !== 465} onchange={() => smtpSecurity(false)} />STARTTLS</label></div></fieldset>
          </div>
        </section>
      {:else}
        <section class="server-block" aria-labelledby="jmap-title">
          <h4 id="jmap-title"><Icon name="server" size={15} />JMAP server</h4>
          <label class="field">JMAP server or session URL<input id="wizard-jmap-host" required maxlength="2000" autocapitalize="none" spellcheck="false" bind:value={fields.jmap_host} oninput={() => touch('jmap_host')} placeholder="mail.example.com" /></label>
          <PortPicker id="wizard-jmap-port" label="JMAP port" value={fields.jmap_port} options={jmapPorts} onpick={port => { fields.jmap_port = port; touch('jmap_port'); }} />
        </section>
      {/if}
      <section class="server-block" aria-labelledby="signin-title">
        <h4 id="signin-title"><Icon name="key" size={15} />Sign in</h4>
        <div class="form-grid">
          <label class="field"><span>Username{#if fields.protocol === 'jmap'} <span class="optional">(optional)</span>{/if}</span><input id="wizard-username" required={fields.protocol === 'imap'} maxlength="320" autocomplete="username" autocapitalize="none" spellcheck="false" bind:value={fields.username} oninput={() => touch('username')} /></label>
          <label class="field">{passwordLabel}
            <span class="secret-input"><input id="wizard-secret" type={reveal ? 'text' : 'password'} required={!account} maxlength="8192" autocomplete="new-password" bind:value={secret} placeholder={account ? 'Unchanged' : ''} /><button type="button" class="reveal" aria-label={reveal ? 'Hide password' : 'Show password'} aria-pressed={reveal} onclick={() => reveal = !reveal}><Icon name={reveal ? 'eye-off' : 'eye'} size={15} /></button></span>
          </label>
        </div>
        {#if found?.app_password && fields.protocol === 'imap'}<p class="hint">{found.provider} needs an app password from its security settings, not your normal password.</p>{/if}
        {#if fields.protocol === 'jmap'}<p class="hint">Leave the username empty to sign in with an API token instead of a password.</p>{/if}
      </section>
      {#if fields.protocol === 'imap'}
        <details class="certificate" open={!!certificate}>
          <summary>Trusted certificate</summary>
          <div class="certificate-body">
            {#if certificate}
              <span class="certificate-state"><Icon name="shield" size={15} />Custom certificate in use</span>
              <button type="button" class="text-button" onclick={() => certificate = ''}>Use system certificates</button>
            {:else}
              <label class="secondary certificate-pick"><Icon name="upload" size={15} />Choose certificate…<input class="sr-only" type="file" accept=".pem,.crt,.cer,application/x-pem-file" onchange={chooseCertificate} /></label>
            {/if}
          </div>
          <p class="hint">For Proton Mail Bridge or a self-signed server. Both servers must present this certificate, and it replaces your system’s certificate authorities for this account.</p>
        </details>
      {/if}
    {:else}
      <h3 class="step-title sr-only" tabindex="-1">Check and save</h3>
      <div class="review-identity">
        <span class="avatar" style:--hue={hue(name || address)} aria-hidden="true">{initial(name || address)}</span>
        <div><strong class="review-address">{address.trim()}</strong><span>{fields.protocol === 'imap' ? 'IMAP and SMTP' : 'JMAP'} account</span></div>
        <button type="button" class="text-button" onclick={() => go('email')}>Change</button>
      </div>
      <ul class="review-list">
        {#if fields.protocol === 'imap'}
          <li>
            <Icon name="inbox" size={16} />
            <div><span class="review-label">Receives mail from</span><strong class="review-value">{fields.imap_host}</strong><span class="review-meta">IMAP · port {fields.imap_port} · {security(fields.imap_starttls)}</span>{#each warnings.imap as warning (warning)}<span class="review-warn"><Icon name="alert" size={13} />{warning}</span>{/each}</div>
            <button type="button" class="text-button" aria-label="Change incoming server" onclick={() => go('servers', 'wizard-imap-host')}>Change</button>
          </li>
          <li>
            <Icon name="sent" size={16} />
            <div><span class="review-label">Sends mail through</span><strong class="review-value">{fields.smtp_host}</strong><span class="review-meta">SMTP · port {fields.smtp_port} · {fields.smtp_port === 465 ? 'TLS' : 'STARTTLS'}</span>{#each warnings.smtp as warning (warning)}<span class="review-warn"><Icon name="alert" size={13} />{warning}</span>{/each}</div>
            <button type="button" class="text-button" aria-label="Change outgoing server" onclick={() => go('servers', 'wizard-smtp-host')}>Change</button>
          </li>
        {:else}
          <li>
            <Icon name="server" size={16} />
            <div><span class="review-label">Connects to</span><strong class="review-value">{jmapUrl}</strong><span class="review-meta">JMAP · port {fields.jmap_port}</span>{#each warnings.jmap as warning (warning)}<span class="review-warn"><Icon name="alert" size={13} />{warning}</span>{/each}</div>
            <button type="button" class="text-button" aria-label="Change JMAP server" onclick={() => go('servers', 'wizard-jmap-host')}>Change</button>
          </li>
        {/if}
        <li>
          <Icon name="key" size={16} />
          <div><span class="review-label">Signs in as</span><strong class="review-value">{fields.username.trim() || 'API token (no username)'}</strong><span class="review-meta">{secret ? `${passwordLabel} entered` : account ? 'Saved password unchanged' : 'No password yet'}</span></div>
          <button type="button" class="text-button" aria-label="Change sign-in details" onclick={() => go('servers', 'wizard-username')}>Change</button>
        </li>
        {#if fields.protocol === 'imap' && certificate}
          <li><Icon name="shield" size={16} /><div><span class="review-label">Trusts</span><strong class="review-value">A custom certificate</strong><span class="review-meta">Instead of your system’s certificate authorities</span></div></li>
        {/if}
      </ul>
      <label class="field"><span>Account name <span class="optional">(optional)</span></span><input maxlength="100" autocomplete="off" bind:value={name} placeholder="Work, Personal…" /></label>
      <p class="hint">Saving does not test the connection; Colecto checks it in the background.</p>
    {/if}
    </fieldset>
    {/key}
    {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  </div>
  <footer class="sheet-foot">
    {#if step === 'email'}
      {#if oncancel}<button type="button" class="secondary" onclick={oncancel}>Cancel</button>{/if}
      <button class="primary" type="submit" disabled={looking}>{looking ? 'Looking up…' : 'Continue'}</button>
    {:else}
      <button type="button" class="secondary" disabled={working} onclick={back}>Back</button>
      <button class="primary" type="submit" disabled={working}>{step === 'servers' ? 'Continue' : working ? 'Saving…' : submitLabel}</button>
    {/if}
  </footer>
</form>

<style>
  .account-wizard { display: flex; flex-direction: column; width: 100%; min-height: 0; }
  .wizard-steps { display: flex; gap: 6px 18px; flex-wrap: wrap; padding: 10px 24px 0; color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); }
  .wizard-steps li { display: flex; align-items: center; gap: 7px; }
  .wizard-steps li.current { color: var(--ink); font-weight: 560; }
  .wizard-steps li.done { color: var(--ink-2); }
  .step-dot { display: grid; place-items: center; width: 18px; height: 18px; border-radius: 50%; background: var(--control); color: var(--ink-2); font-size: calc(11px * var(--text-scale)); font-variant-numeric: tabular-nums; transition: background-color 180ms ease, color 180ms ease; }
  .current .step-dot { background: var(--mint); color: var(--mint-ink); }
  .done .step-dot { background: var(--select); color: var(--mint); }
  .wizard-step { display: grid; gap: 16px; min-width: 0; animation: step-in 200ms var(--ease-out) both; }
  /* Steps slide a little in the direction of travel, so Back feels like going back. */
  @keyframes step-in { from { opacity: 0; transform: translateX(calc(var(--dir) * 10px)); } }
  @media (prefers-reduced-motion: reduce) { .wizard-step { animation: none; } }
  .step-title { outline: none; }
  .lookup { display: flex; align-items: center; gap: 10px; min-height: 30px; color: var(--ink-2); font-size: calc(13.5px * var(--text-scale)); overflow-wrap: anywhere; }
  .lookup strong { color: var(--ink); font-weight: 560; }
  .lookup .text-button { margin-left: auto; flex-shrink: 0; }
  .spinner { flex-shrink: 0; width: 15px; height: 15px; border: 2px solid var(--line); border-top-color: var(--mint); border-radius: 50%; animation: spin 700ms linear infinite; }
  .lookup-hint { margin-top: -6px; line-height: 1.5; }
  .manual { justify-self: start; }
  .found-note, .limit-note { display: flex; align-items: flex-start; gap: 10px; padding: 11px 12px; border-radius: 10px; font-size: calc(13px * var(--text-scale)); line-height: 1.5; overflow-wrap: anywhere; animation: note-in 260ms var(--ease-out) both; }
  .found-note { background: var(--select); color: var(--ink-2); }
  .found-note :global(.icon) { margin-top: 2px; color: var(--mint); }
  .found-note strong, .limit-note strong { color: var(--ink); font-weight: 600; }
  .found-note p + p { margin-top: 2px; }
  .limit-note { background: var(--danger-bg); color: oklch(0.9 0.04 30); }
  .limit-note :global(.icon) { margin-top: 2px; }
  .limit-note .text-button { margin-top: 4px; color: var(--ink); }
  @keyframes note-in { from { opacity: 0; transform: translateY(-4px); } }
  @media (prefers-reduced-motion: reduce) { .found-note, .limit-note { animation: none; } }
  .server-block { display: grid; gap: 12px; min-width: 0; }
  .server-block + .server-block { padding-top: 16px; border-top: 1px solid var(--line-soft); }
  .server-block h4 { display: flex; margin: 0; align-items: center; gap: 8px; color: var(--ink); font-size: calc(13.5px * var(--text-scale)); font-weight: 600; }
  .server-block h4 :global(.icon) { color: var(--ink-3); }
  .port-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: end; gap: 12px 16px; }
  .review-identity { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 12px; }
  .review-identity > div { display: grid; gap: 2px; min-width: 0; color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); }
  .review-address { color: var(--ink); font-size: calc(15px * var(--text-scale)); font-weight: 600; overflow-wrap: anywhere; }
  .review-list { display: grid; border: 1px solid var(--line); border-radius: 12px; background: var(--sunk); }
  .review-list li { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: start; gap: 12px; padding: 12px 12px 12px 14px; }
  .review-list li + li { border-top: 1px solid var(--line-soft); }
  .review-list li > :global(.icon) { margin-top: 2px; color: var(--ink-3); }
  .review-list li > div { display: grid; gap: 2px; min-width: 0; }
  .review-label { color: var(--ink-3); font-size: calc(12px * var(--text-scale)); }
  .review-value { color: var(--ink); font-size: calc(14px * var(--text-scale)); font-weight: 560; overflow-wrap: anywhere; }
  .review-meta { color: var(--ink-2); font-size: calc(12.5px * var(--text-scale)); font-variant-numeric: tabular-nums; }
  .review-warn { display: flex; align-items: flex-start; gap: 6px; margin-top: 4px; color: oklch(0.85 0.1 75); font-size: calc(12.5px * var(--text-scale)); line-height: 1.45; }
  .review-warn :global(.icon) { margin-top: 2px; }
  .review-list .text-button, .review-identity .text-button { flex-shrink: 0; }
  @media (max-width: 520px) {
    .wizard-steps { padding: 10px 16px 0; }
    .port-row { grid-template-columns: minmax(0, 1fr); }
  }
</style>
