<script lang="ts">
  import { appearance, setTheme } from '../lib/theme.svelte';
  import type { Theme } from '../lib/theme.svelte';
  import { setAccessToken } from '../lib/api';
  import { onMount, tick } from 'svelte';
  import ExtensionPanel from '../ExtensionPanel.svelte';
  import { mailExtensions } from './registry';
  import AccountSettings from '../AccountSettings.svelte';
  import { createServices, hostService } from '../lib/api';
  import type { Draft, MailItem, AccountInfo, SecurityInfo } from '../lib/api';
  let { extensionStates, extensionSettings, onsettings }: { extensionSettings: import('svelte').Snippet; onsettings: () => void; extensionStates: import('../lib/api').ExtensionState[] } = $props();
  let Mailbox = $derived(extensionStates.find(e => e.manifest.id === 'inbox')?.enabled ? mailExtensions.inbox : null);
  let Reader = $derived(extensionStates.find(e => e.manifest.id === 'reader')?.enabled ? mailExtensions.reader : null);
  let Composer = $derived(extensionStates.find(e => e.manifest.id === 'composer')?.enabled ? mailExtensions.composer : null);
  type View = { kind: 'message'; id: string; folder: 'Inbox' | 'Sent' } | { kind: 'draft'; draft: Draft | null } | null;
  let connected = $state(false);
  let initialized = $state(false);
  let accounts = $state<AccountInfo[]>([]);
  let security = $state<SecurityInfo>({ protected: false, locked: false, cleanup_pending: false });
  let accountId = $state('demo');
  let settingsOpen = $state(false);
  let settingsPage = $state<'Accounts' | 'Appearance' | 'Servers' | 'Extensions'>('Accounts');
  let tabs = $state<{ account: string; id: string; subject: string; folder: 'Inbox' | 'Sent' }[]>([]);
  let currentTabs = $derived(tabs.filter(tab => tab.account === accountId));
  let query = $state('');
  let serverUrl = $state('');
  let serverError = $state('');
  function visitServer(event: SubmitEvent) {
    event.preventDefault(); serverError = '';
    try { const url = new URL(serverUrl); if (url.protocol !== 'https:' && !(url.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname))) throw new Error(); if (url.username || url.password || url.search || url.hash) throw new Error(); window.location.assign(url.origin); }
    catch { serverError = 'Enter an HTTPS server address, or HTTP for a local server. Do not include credentials or an access token.'; }
  }
  function settingsTab(next: typeof settingsPage) { if (next === settingsPage || !canLeave()) return; dirty = false; settingsPage = next; if (next === 'Extensions') onsettings(); }
  function activateTab(tab: typeof tabs[number]) { if (!canLeave()) return; clearView(); settingsOpen = false; view = { kind: 'message', id: tab.id, folder: tab.folder }; }
  async function closeTab(id: string) {
    const index = currentTabs.findIndex(tab => tab.id === id);
    const active = view?.kind === 'message' && view.id === id;
    tabs = tabs.filter(tab => tab.account !== accountId || tab.id !== id);
    if (active) {
      const next = currentTabs[Math.min(index, currentTabs.length - 1)];
      clearView();
      if (next) view = { kind: 'message', id: next.id, folder: next.folder };
    }
    await tick();
    const target = document.querySelector<HTMLButtonElement>('.message-tab.active .tab-title') ?? document.querySelector<HTMLButtonElement>('.folder-nav button');
    target?.focus();
  }

  let panelId = $state<string | null>(null);
  function openPanel(id: string) { if (!canLeave()) return; clearView(); ++listGeneration; settingsOpen = false; panelId = id; }
  let accountError = $state('');
  let services = $derived(createServices(accountId));
  let activeAccount = $derived(accounts.find(a => a.id === accountId));
  let sender = $derived(activeAccount?.settings?.address ?? 'Demo account · sending unavailable');
  let accountLabel = $derived(activeAccount?.settings?.name || activeAccount?.settings?.address || 'Demo account');
  async function refreshAccounts() {
    const [nextAccounts, nextSecurity] = await Promise.all([hostService.accounts(), hostService.security()]);
    accounts = nextAccounts; security = nextSecurity;
    connected = nextAccounts.find(a => a.id === accountId)?.connected ?? false;
  }
  function changeAccount(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const next = select.value;
    if (!canLeave()) { select.value = accountId; return; }
    clearView(); ++listGeneration; items = [];
    accountId = next; try { localStorage.setItem('colecto.account', next); } catch { /* Selection persistence is optional. */ } connected = accounts.find(a => a.id === next)?.connected ?? false;
    settingsOpen = false; query = ''; folder = 'Inbox'; void loadList();
  }
  function manageAccounts() {
    if (!canLeave()) return;
    clearView(); ++listGeneration; settingsOpen = true; settingsPage = 'Accounts';
    void refreshAccounts().catch(e => accountError = (e as Error).message);
  }
  function closeSettings() {
    if (!canLeave()) return;
    clearView(); settingsOpen = false; void loadList();
  }
  let folder = $state<'Inbox' | 'Drafts' | 'Sent'>('Inbox');
  let items = $state<MailItem[]>([]);
  let view = $state<View>(null);
  let loading = $state(true);
  let error = $state('');
  let openError = $state('');
  let dirty = $state(false);
  let busy = $state(false);
  let opening = $state(false);
  let listGeneration = 0;
  let openGeneration = 0;
  let selected = $derived(view?.kind === 'message' ? view.id : view?.kind === 'draft' ? view.draft?.id ?? null : null);
  async function loadList() {
    const generation = ++listGeneration;
    if (!Mailbox) { items = []; loading = false; return; }
    const requestedFolder = folder;
    const inboxService = services.inbox;
    loading = true; error = ''; openError = '';
    try {
      const result: MailItem[] = requestedFolder !== 'Drafts'
        ? (await inboxService.messages(requestedFolder === 'Sent' ? 'sent' : 'inbox', query)).map(m => ({ id: m.id, name: m.sender, subject: m.subject, preview: m.body, date: m.date }))
        : (await inboxService.drafts()).slice().reverse().map(d => ({ id: d.id, name: d.recipient || 'No recipient', subject: d.subject, preview: d.body, date: 'Draft' }));
      if (generation === listGeneration) items = result;
    } catch (e) { if (generation === listGeneration) error = (e as Error).message; }
    finally { if (generation === listGeneration) loading = false; }
  }
  function canLeave() { return initialized && !busy && (!dirty || window.confirm('Discard unsaved changes? Your last saved draft will be kept.')); }
  function clearView() { panelId = null; ++openGeneration; view = null; dirty = false; openError = ''; opening = false; }
  function navigate(next: 'Inbox' | 'Drafts' | 'Sent') {
    if (!canLeave()) return;
    clearView(); settingsOpen = false; query = ''; folder = next; void loadList();
  }
  function compose() { if (!Composer || !canLeave()) return; clearView(); settingsOpen = false; view = { kind: 'draft', draft: null }; }
  function back() { if (!canLeave()) return; clearView(); }
  async function open(id: string) {
    if (!canLeave()) return;
    if (folder !== 'Drafts') {
      if (!currentTabs.some(tab => tab.id === id)) {
        if (currentTabs.length >= 30) { openError = 'You have 30 messages open in this account. Close a tab to open another.'; return; }
        tabs = [...tabs, { account: accountId, id, subject: items.find(item => item.id === id)?.subject || '(No subject)', folder }];
      }
      clearView(); view = { kind: 'message', id, folder }; return;
    }
    clearView();
    const generation = ++openGeneration;
    opening = true;
    try {
      const draft = await services.composer.read(id);
      if (generation === openGeneration) view = { kind: 'draft', draft };
    } catch (e) { if (generation === openGeneration) openError = (e as Error).message; }
    finally { if (generation === openGeneration) opening = false; }
  }
  function saved(draft: Draft) {
    // Keep the mounted editor intact; refreshing the list must not reset its state.
    if (view?.kind === 'draft') view.draft = draft;
    if (folder === 'Drafts') void loadList();
  }
  function beforeUnload(event: BeforeUnloadEvent) { if (dirty || busy) { event.preventDefault(); event.returnValue = ''; } }
  onMount(() => {
    void refreshAccounts().then(() => {
      let remembered = ''; try { remembered = localStorage.getItem('colecto.account') ?? ''; } catch { /* Storage can be disabled. */ }
      if (accounts.some(a => a.id === remembered)) accountId = remembered;
      connected = accounts.find(a => a.id === accountId)?.connected ?? false;
    }).catch(e => accountError = (e as Error).message).finally(() => { initialized = true; void loadList(); });
  });
</script>

<svelte:window onbeforeunload={beforeUnload} />
<div class="app-shell" class:detail-open={view !== null} class:settings-open={settingsOpen}>
  <aside class="sidebar">
    <a class="brand" href="/" onclick={(event) => { event.preventDefault(); navigate('Inbox'); }} aria-label="Colecto home"><span class="brand-mark" aria-hidden="true">c</span>colecto<span class="brand-dot">.</span></a>
    <button class="compose-button" disabled={busy || !initialized} onclick={compose}><span aria-hidden="true">＋</span> Compose</button>
    {#if currentTabs.length}<nav class="open-messages" aria-label="Open messages">
      {#each currentTabs as tab (tab.id)}<div class="message-tab" class:active={view?.kind === 'message' && view.id === tab.id}>
        <button class="tab-title" aria-pressed={view?.kind === 'message' && view.id === tab.id} disabled={busy} onclick={() => activateTab(tab)} title={tab.subject}>{tab.subject}</button>
        <button class="tab-close" aria-label={`Close ${tab.subject}`} disabled={busy} onclick={() => closeTab(tab.id)}>×</button>
      </div>{/each}
    </nav>{/if}
    <nav class="folder-nav" aria-label="Mail folders">
      <button class:active={folder === 'Inbox'} aria-current={folder === 'Inbox' ? 'page' : undefined} disabled={busy || !initialized} onclick={() => navigate('Inbox')}><span class="nav-icon" aria-hidden="true">▤</span>Inbox</button>
      <button class:active={folder === 'Drafts'} aria-current={folder === 'Drafts' ? 'page' : undefined} disabled={busy || !initialized} onclick={() => navigate('Drafts')}><span class="nav-icon" aria-hidden="true">✎</span>Drafts</button>
      <button class:active={folder === 'Sent'} aria-current={folder === 'Sent' ? 'page' : undefined} disabled={busy || !initialized} onclick={() => navigate('Sent')}><span class="nav-icon" aria-hidden="true">↗</span>Sent</button>
      {#each extensionStates.filter(extension => !extension.bundled && extension.enabled) as extension (extension.manifest.id)}
        <button disabled={busy || !initialized} class:active={panelId === extension.manifest.id} onclick={() => openPanel(extension.manifest.id)}>{extension.manifest.name}</button>
      {/each}
    </nav>
    <div class="profile-row">
      <div class="profile-account"><span class="avatar" aria-hidden="true">{Array.from(accountLabel)[0]?.toLocaleUpperCase()}</span><span class="profile-name">{accountLabel}<small>{security.locked ? 'Credentials locked' : activeAccount?.configured ? activeAccount.settings?.address : 'Local sample mail'}</small></span><span class="profile-chevron" aria-hidden="true">⌄</span>
        <select aria-label="Current account" value={accountId} disabled={busy || !initialized} onchange={changeAccount}>{#if !accounts.length}<option value="demo">Demo account</option>{/if}{#each accounts as account (account.id)}<option value={account.id}>{account.settings ? account.settings.name || account.settings.address : 'Demo account'}</option>{/each}</select>
      </div>
      <button class="settings-button" aria-label="Settings" title="Settings" disabled={busy || !initialized} onclick={manageAccounts}><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true"><path d="m9 3-1 3-3 1 1 3-2 2 2 2-1 3 3 1 1 3h6l1-3 3-1-1-3 2-2-2-2 1-3-3-1-1-3Z"/><circle cx="12" cy="12" r="3"/></svg></button>
    </div>
  </aside>
  <main class="workspace">
    {#if accountError}<div class="account-error error-box" role="alert">{accountError}<button class="text-button" onclick={() => { accountError = ''; void refreshAccounts().catch(e => accountError = (e as Error).message); }}>Retry account settings</button></div>{/if}
    {#if settingsOpen}<section class="settings-page" aria-label="Settings"><header class="settings-header"><div><div class="eyebrow">YOUR WORKSPACE</div><h1 tabindex="-1">Settings</h1></div><button class="secondary" disabled={busy} onclick={closeSettings}>Back to mail</button></header>
      <nav class="settings-nav" aria-label="Settings sections">{#each ['Accounts', 'Appearance', 'Servers', 'Extensions'] as page}<button class:active={settingsPage === page} aria-current={settingsPage === page ? 'page' : undefined} disabled={busy} onclick={() => settingsTab(page as typeof settingsPage)}>{page}</button>{/each}</nav>
      {#if settingsPage === 'Accounts'}<AccountSettings {accounts} {security} onrefresh={refreshAccounts} ondirty={(value) => dirty = value} onbusy={(value) => busy = value} />
      {:else if settingsPage === 'Appearance'}<section class="settings-card"><h2>Appearance</h2><p>Choose a theme for this device. System follows your device’s color preference.</p><label class="field">Color theme<select value={appearance.theme} onchange={(event) => setTheme(event.currentTarget.value as Theme)}><option value="system">System</option><option value="dark">Dark</option><option value="light">Light</option></select></label></section>
      {:else if settingsPage === 'Servers'}<section class="settings-card"><h2>Mail service</h2><p>Your current service is <strong>{window.location.origin}</strong>. Accounts and saved drafts belong to this service.</p><p>To use a remote daemon, open its HTTPS address. Your credentials stay on that server. Local services can also use HTTP on loopback.</p><form onsubmit={visitServer}><label class="field">Server address<input type="url" bind:value={serverUrl} placeholder="https://mail.example.com" required autocomplete="url" /></label>{#if serverError}<p class="error-box" role="alert">{serverError}</p>{/if}<button class="secondary">Open server</button></form><button class="text-button" onclick={() => { setAccessToken(''); window.location.reload(); }}>Forget this session’s access token</button></section>
      {:else}{@render extensionSettings()}{/if}
    </section>
    {:else if panelId}{#key `${accountId}:${panelId}`}<ExtensionPanel id={panelId} account={accountId} onback={() => navigate('Inbox')} />{/key}
    {:else}{#if Mailbox}<div class="mailbox-column">{#if folder !== 'Drafts'}<form class="mail-search" role="search" onsubmit={(event) => { event.preventDefault(); void loadList(); }}><label class="sr-only" for="mail-search">Search {folder}</label><input id="mail-search" type="search" bind:value={query} placeholder={`Search ${folder.toLowerCase()}…`} /><button class="text-button" type="submit">Search</button></form>{/if}<Mailbox {items} {selected} {folder} {loading} error={error || openError} disabled={busy || opening} onopen={open} onretry={loadList} /></div>{:else}<p class="error-box">Mailbox extension disabled. Open Extensions to enable it.</p>{/if}
    <div class="reading-pane">
      {#key view?.kind === 'message' ? `message:${view.id}` : view}
        {#if view?.kind === 'message' && Reader}<Reader id={view.id} service={services.reader} folder={view.folder} {accountLabel} onback={back} />
        {:else if view?.kind === 'draft' && Composer}<Composer {connected} draft={view.draft} service={services.composer} {sender} onback={back} ondirty={(value) => dirty = value} onbusy={(value) => busy = value} onsaved={saved} />
        {:else}
          <section class="welcome"><div class="eyebrow">{accountLabel}</div><h2>Your mail, in focus.</h2><p>{opening ? 'Opening your draft…' : 'Select a message to read, or compose a new one.'}</p><button class="secondary" disabled={busy || !initialized} onclick={compose}>New message</button></section>
        {/if}
      {/key}
    </div>{/if}
  </main>
</div>
