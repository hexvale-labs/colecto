<script lang="ts">
  import CommandRunner from '../CommandRunner.svelte';
  import type { Command } from '../CommandRunner.svelte';
  import type { Labels } from '../lib/api';
  import { initial, hue } from '../lib/initial';
  import Icon from '../Icon.svelte';
  import Select from '../Select.svelte';
  import mark from '../assets/logo-square.svg';
  import wordmark from '../assets/wordmark.svg';
  import { setAccessToken } from '../lib/api';
  import { onMount, tick } from 'svelte';
  import ExtensionPanel from '../ExtensionPanel.svelte';
  import { mailExtensions } from './registry';
  import ApiKeys from '../ApiKeys.svelte';
  import SendApprovals from '../SendApprovals.svelte';
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
  const settingsPages = [{ name: 'Accounts', icon: 'account' }, { name: 'Servers', icon: 'server' }, { name: 'Extensions', icon: 'extension' }, { name: 'API keys', icon: 'key' }, { name: 'Send approvals', icon: 'shield' }] as const;
  let settingsPage = $state<typeof settingsPages[number]['name']>('Accounts');
  let tabs = $state<{ account: string; id: string; subject: string; folder: 'Inbox' | 'Sent' }[]>([]);
  let currentTabs = $derived(tabs.filter(tab => tab.account === accountId));
  let query = $state('');
  let appliedQuery = $state('');
  let labels = $state<Labels>({});
  let labelFilter = $state('');
  let labelName = $state('');
  let labelError = $state('');
  let labeling = $state(false);
  let runner: CommandRunner;
  async function label(name: string, messageId?: string, remove = false) {
    if (labeling || loading || busy || !initialized) return;
    const account = accountId;
    labeling = true; labelError = '';
    try {
      const result = await services.reader.label(name, messageId, remove);
      if (account === accountId) { labels = result; labelName = ''; if (!labels[labelFilter]) labelFilter = ''; }
    } catch (e) { if (account === accountId) labelError = (e as Error).message; }
    finally { labeling = false; }
  }
  async function searchMail(target = 'mail-search') {
    if (settingsOpen || panelId || view) { if (!canLeave()) return; const reload = settingsOpen || !!panelId; clearView(); settingsOpen = false; if (reload) void loadList(); }
    await tick(); document.getElementById(target)?.focus();
  }
  function keyboard(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'k' && !event.isComposing && !event.repeat) {
      if (document.querySelector('dialog[open]:not(.command-runner)')) return;
      event.preventDefault(); runner.open();
    }
  }
  let serverUrl = $state('');
  let serverError = $state('');
  function visitServer(event: SubmitEvent) {
    event.preventDefault(); serverError = '';
    try { const url = new URL(serverUrl); if (url.protocol !== 'https:' && !(url.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname))) throw new Error(); if (url.username || url.password || url.search || url.hash) throw new Error(); window.location.assign(url.origin); }
    catch { serverError = 'Enter an HTTPS server address, or HTTP for a local server. Do not include credentials or an access token.'; }
  }
  function settingsTab(next: typeof settingsPage) { if (next === settingsPage || !canLeave()) return; dirty = false; settingsPage = next; if (next === 'Extensions') onsettings(); }
  function activateTab(tab: typeof tabs[number]) { if (!canLeave()) return; clearView(); invalidateList(); settingsOpen = false; view = { kind: 'message', id: tab.id, folder: tab.folder }; }
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
  function openPanel(id: string) { if (!canLeave()) return; clearView(); invalidateList(); settingsOpen = false; panelId = id; }
  let accountError = $state('');
  let services = $derived(createServices(accountId));
  let activeAccount = $derived(accounts.find(a => a.id === accountId));
  let sender = $derived(activeAccount?.settings?.address ?? 'Demo account');
  let accountLabel = $derived(activeAccount?.settings?.name || activeAccount?.settings?.address || 'Demo account');
  async function refreshAccounts() {
    const [nextAccounts, nextSecurity] = await Promise.all([hostService.accounts(), hostService.security()]);
    accounts = nextAccounts; security = nextSecurity;
    connected = nextAccounts.find(a => a.id === accountId)?.connected ?? false;
  }
  let accountOptions = $derived(accounts.length ? accounts.map(account => ({ value: account.id, label: account.settings?.name || account.settings?.address || 'Demo account', hint: account.settings?.address ?? 'Sample mail' })) : [{ value: 'demo', label: 'Demo account', hint: 'Sample mail' }]);
  const folders = [{ name: 'Inbox', icon: 'inbox' }, { name: 'Drafts', icon: 'drafts' }, { name: 'Sent', icon: 'sent' }] as const;
  function selectAccount(next: string) {
    if (!accounts.some(account => account.id === next) || !canLeave()) return;
    clearView(); invalidateList(); items = []; fetchedAt = null; labels = {}; labelFilter = ''; labelError = '';
    accountId = next; try { localStorage.setItem('colecto.account', next); } catch { /* Selection persistence is optional. */ } connected = accounts.find(a => a.id === next)?.connected ?? false;
    settingsOpen = false; query = ''; folder = 'Inbox'; void loadList();
  }
  function manageAccounts() {
    if (!canLeave()) return;
    clearView(); invalidateList(); settingsOpen = true; settingsPage = 'Accounts';
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
  let fetching = $state(false);
  let fetchedAt = $state<number | null>(null);
  let error = $state('');
  let openError = $state('');
  let dirty = $state(false);
  let busy = $state(false);
  let opening = $state(false);
  let listGeneration = 0;
  let openGeneration = 0;
  let selected = $derived(view?.kind === 'message' ? view.id : view?.kind === 'draft' ? view.draft?.id ?? null : null);
  let visibleItems = $derived(labelFilter ? items.filter(item => labels[labelFilter]?.includes(item.id)) : items);
  let commands: Command[] = $derived([
    { id: 'compose', name: 'Compose a new message', run: () => compose(), disabled: !Composer },
    ...(['Inbox', 'Drafts', 'Sent'] as const).map(next => ({ id: next, name: `Go to ${next}`, run: () => navigate(next) })),
    { id: 'search', name: 'Search emails', run: () => void searchMail(), disabled: !Mailbox },
    { id: 'new-label', name: 'Create a local label', run: () => void searchMail('new-label'), disabled: !Reader },
    ...Object.keys(labels).filter(() => view?.kind === 'message').map(name => ({ id: `assign-${name}`, name: `${labels[name].includes(view?.kind === 'message' ? view.id : '') ? 'Remove' : 'Add'} label: ${name}`, run: () => { if (view?.kind === 'message') void label(name, view.id, labels[name].includes(view.id)); }, disabled: loading })),
    { id: 'refresh', name: 'Refresh current mailbox', run: () => void loadList(), disabled: settingsOpen || !!panelId },
    { id: 'back', name: 'Back to message list', run: back },
    ...settingsPages.map(({ name: page }) => ({ id: `settings-${page}`, name: `Settings: ${page}`, run: () => { manageAccounts(); if (settingsOpen && !dirty) settingsTab(page); } })),
    ...accounts.map(account => ({ id: `account-${account.id}`, name: `Switch account: ${account.settings?.name || account.settings?.address || 'Demo account'}`, run: () => selectAccount(account.id) })),
    ...currentTabs.map(tab => ({ id: `tab-${tab.id}`, name: `Open message: ${tab.subject}`, run: () => activateTab(tab) })),
    ...extensionStates.filter(extension => !extension.bundled && extension.enabled).map(extension => ({ id: `extension-${extension.manifest.id}`, name: `Open extension: ${extension.manifest.name}`, run: () => openPanel(extension.manifest.id) })),
    ...Object.keys(labels).map(name => ({ id: `label-${name}`, name: `Filter label: ${name}`, run: () => { navigate('Inbox'); if (!dirty && folder === 'Inbox' && !settingsOpen) labelFilter = name; } })),
    ...(view?.kind === 'draft' ? ['Save draft', ...(connected ? ['Send message'] : [])].map(name => ({ id: name, name, run: () => { document.querySelector<HTMLButtonElement>(`.composer [data-command="${name === 'Save draft' ? 'save' : 'send'}"]`)?.click(); } })) : []),
    ...(view?.kind === 'message' ? [{ id: 'close-tab', name: 'Close current message tab', run: () => { if (view?.kind === 'message') void closeTab(view.id); } }] : []),
  ].map(command => ({ ...command, disabled: command.disabled || busy || labeling || !initialized })));
  function invalidateList() { ++listGeneration; loading = false; fetching = false; }
  async function loadList() {
    if (labeling) return;
    const generation = ++listGeneration;
    if (!Mailbox) { items = []; loading = false; return; }
    const requestedFolder = folder;
    const requestedQuery = query.trim();
    const requestedAccount = accountId;
    const inboxService = services.inbox;
    loading = items.length === 0; fetching = true; error = ''; openError = '';
    try {
      if (requestedFolder !== 'Drafts' && !requestedQuery) {
        const snapshot = await inboxService.cached(requestedFolder === 'Sent' ? 'sent' : 'inbox');
        if (generation !== listGeneration || requestedAccount !== accountId) return;
        items = snapshot.messages.map(m => ({ id: m.id, name: m.sender, subject: m.subject, preview: m.body, date: m.date }));
        fetchedAt = snapshot.fetched_at; loading = false;
      }
      const normalizedQuery = requestedQuery.toLocaleLowerCase();
      const [nextLabels, result] = await Promise.all([
        inboxService.labels(),
        requestedFolder !== 'Drafts'
          ? inboxService.messages(requestedFolder === 'Sent' ? 'sent' : 'inbox', requestedQuery).then(messages => messages.map(m => ({ id: m.id, name: m.sender, subject: m.subject, preview: m.body, date: m.date })))
          : inboxService.drafts().then(drafts => drafts.filter(draft => [draft.recipient, draft.subject, draft.body].some(text => text.toLocaleLowerCase().includes(normalizedQuery))).slice().reverse().map(d => ({ id: d.id, name: d.recipient || 'No recipient', subject: d.subject, preview: d.body, date: 'Draft' }))),
      ]);
      if (generation === listGeneration && requestedAccount === accountId) { items = result; labels = nextLabels; appliedQuery = requestedQuery; fetchedAt = Math.floor(Date.now() / 1000); }
    } catch (e) { if (generation === listGeneration) error = (e as Error).message; }
    finally { if (generation === listGeneration) { loading = false; fetching = false; } }
  }
  function canLeave() { return initialized && !busy && !labeling && (!dirty || window.confirm('Discard unsaved changes? Your last saved draft will be kept.')); }
  function clearView() { panelId = null; ++openGeneration; view = null; dirty = false; openError = ''; opening = false; }
  function navigate(next: 'Inbox' | 'Drafts' | 'Sent') {
    if (!canLeave()) return;
    clearView(); settingsOpen = false; query = ''; labelFilter = '';
    if (next !== folder) { items = []; fetchedAt = null; appliedQuery = ''; }
    folder = next; void loadList();
  }
  function compose(draft: Draft | null = null) { if (!Composer || !canLeave()) return; clearView(); settingsOpen = false; view = { kind: 'draft', draft }; }
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

<svelte:window onbeforeunload={beforeUnload} onkeydown={keyboard} />
<CommandRunner bind:this={runner} {commands} />
<div class="app-shell" class:detail-open={view !== null || !!panelId} class:settings-open={settingsOpen}>
  <aside class="rail">
    <header class="rail-head">
      <a class="brand" href="/" aria-label="Colecto, go to Inbox" onclick={(event) => { event.preventDefault(); navigate('Inbox'); }}><img class="brand-mark" src={mark} alt="" width="28" height="28" /><img class="brand-word" src={wordmark} alt="" width="73" height="17" /></a>
      <button class="icon-button" aria-label="Commands" title="Commands  Ctrl K" onclick={() => runner.open()}><Icon name="command" /></button>
      <button class="compose-button" disabled={busy || !initialized} onclick={() => compose()}><Icon name="compose" />Compose</button>
    </header>
    {#if settingsOpen}
      <nav class="settings-nav" aria-label="Settings sections">
        <button class="nav-back" disabled={busy} onclick={closeSettings}><Icon name="back" />Back to mail</button>
        <h1 tabindex="-1">Settings</h1>
        {#each settingsPages as page (page.name)}<button class="nav-item" class:active={settingsPage === page.name} aria-current={settingsPage === page.name ? 'page' : undefined} disabled={busy} onclick={() => settingsTab(page.name)}><Icon name={page.icon} />{page.name}</button>{/each}
      </nav>
    {:else}
      {#if Mailbox}<form class="mail-search" role="search" onsubmit={(event) => { event.preventDefault(); if (!busy && !labeling) void loadList(); }}>
        <Icon name="search" />
        <label class="sr-only" for="mail-search">Search {folder}</label>
        <input id="mail-search" type="search" enterkeyhint="search" autocapitalize="none" maxlength="1024" bind:value={query} placeholder={`Search ${folder.toLowerCase()}`} />
        <button class="search-submit" type="submit" aria-label="Search" disabled={busy}><kbd>↵</kbd></button>
      </form>{/if}
      <nav class="folder-nav" aria-label="Mail folders">
        {#each folders as item (item.name)}<button class:active={folder === item.name && !panelId} aria-current={folder === item.name && !panelId ? 'page' : undefined} disabled={busy || !initialized} onclick={() => navigate(item.name)}><Icon name={item.icon} /><span>{item.name}</span>{#if folder === item.name && !loading && items.length}<span class="count" aria-hidden="true">{items.length}</span>{/if}</button>{/each}
        {#each extensionStates.filter(extension => !extension.bundled && extension.enabled) as extension (extension.manifest.id)}
          <button disabled={busy || !initialized} class:active={panelId === extension.manifest.id} aria-current={panelId === extension.manifest.id ? 'page' : undefined} onclick={() => openPanel(extension.manifest.id)}><Icon name="extension" /><span>{extension.manifest.name}</span></button>
        {/each}
      </nav>
      {#if Mailbox}
        {#if folder !== 'Drafts'}<div class="label-bar" role="group" aria-label="Filter by label">
          {#if Object.keys(labels).length}<button class="chip" aria-pressed={!labelFilter} onclick={() => labelFilter = ''}>All</button>{/if}
          {#each Object.keys(labels) as name (name)}<span class="chip-group">
            <button class="chip" aria-pressed={labelFilter === name} onclick={() => labelFilter = labelFilter === name ? '' : name}>{name}</button>
            {#if labelFilter === name}<button class="chip-remove" disabled={labeling || loading || busy} aria-label={`Delete label ${name}`} onclick={() => { if (window.confirm(`Delete label “${name}”? Messages will be kept.`)) void label(name, undefined, true); }}><Icon name="close" size={12} /></button>{/if}
          </span>{/each}
          <form class="label-new" onsubmit={(event) => { event.preventDefault(); void label(labelName); }}>
            <input id="new-label" aria-label="New label" placeholder="New label" maxlength="80" bind:value={labelName} />
            <button aria-label="Create label" disabled={!labelName.trim() || labeling || loading || busy}><Icon name="plus" size={14} /></button>
          </form>
        </div>{/if}
        {#if labelError}<p class="error-box" role="alert">{labelError}</p>{/if}
        <Mailbox items={visibleItems} searching={!!appliedQuery || !!labelFilter} {selected} {folder} {loading} {fetching} {fetchedAt} error={error || openError} disabled={busy || opening} onopen={open} onretry={loadList} />
      {:else}<p class="rail-note">The mailbox extension is turned off. Turn it on in Settings → Extensions.</p>{/if}
    {/if}
    <footer class="rail-foot">
      <Select label="Current account" value={accountId} options={accountOptions} placement="above" disabled={busy || !initialized} onchange={selectAccount}>
        {#snippet trigger(current)}
          <span class="avatar" style:--hue={hue(current?.label ?? accountLabel)} aria-hidden="true">{initial(current?.label ?? accountLabel)}</span>
          <span class="account-name">{current?.label ?? accountLabel}<small>{security.locked ? 'Credentials locked' : activeAccount?.configured ? connected ? 'Connected' : 'Not connected' : 'Local sample mail'}</small></span>
        {/snippet}
      </Select>
      <button class="icon-button" class:active={settingsOpen} aria-label="Settings" title="Settings" disabled={busy || !initialized} onclick={() => settingsOpen ? closeSettings() : manageAccounts()}><Icon name="settings" size={18} /></button>
    </footer>
  </aside>
  <main class="stage">
    {#if accountError}<div class="account-error error-box" role="alert">{accountError}<button class="text-button" onclick={() => { accountError = ''; void refreshAccounts().catch(e => accountError = (e as Error).message); }}>Retry account settings</button></div>{/if}
    {#if settingsOpen}<section class="settings-page" aria-label="Settings">
      {#if settingsPage === 'Accounts'}<AccountSettings {accounts} {security} onrefresh={refreshAccounts} ondirty={(value) => dirty = value} onbusy={(value) => busy = value} />
      {:else if settingsPage === 'Servers'}<section class="settings-section"><h2>Mail service</h2><p>You are connected to <strong>{window.location.origin}</strong>. Accounts and saved drafts belong to this service.</p><p>To use a remote daemon, open its HTTPS address. Your credentials stay on that server. Local services can also use HTTP on loopback.</p><form class="inline-form" onsubmit={visitServer}><label class="field">Server address<input type="url" bind:value={serverUrl} placeholder="https://mail.example.com" required autocomplete="url" /></label>{#if serverError}<p class="error-box" role="alert">{serverError}</p>{/if}<div class="button-row"><button class="secondary">Open server</button><button type="button" class="text-button" onclick={() => { setAccessToken(''); window.location.reload(); }}>Forget this session’s access token</button></div></form></section>
      {:else if settingsPage === 'Send approvals'}<SendApprovals />
      {:else if settingsPage === 'API keys'}<ApiKeys {accounts} />
      {:else}{@render extensionSettings()}{/if}
    </section>
    {:else if panelId}{#key `${accountId}:${panelId}`}<ExtensionPanel id={panelId} account={accountId} onback={() => navigate('Inbox')} />{/key}
    {:else}
      {#if currentTabs.length}<nav class="tab-strip" aria-label="Open messages">
        {#each currentTabs as tab (tab.id)}<div class="message-tab" class:active={view?.kind === 'message' && view.id === tab.id}>
          <button class="tab-title" aria-pressed={view?.kind === 'message' && view.id === tab.id} disabled={busy} onclick={() => activateTab(tab)} title={tab.subject}>{tab.subject}</button>
          <button class="tab-close" aria-label={`Close ${tab.subject}`} disabled={busy} onclick={() => closeTab(tab.id)}><Icon name="close" size={13} /></button>
        </div>{/each}
      </nav>{/if}
      <div class="reading-pane">
        {#key view?.kind === 'message' ? `message:${accountId}:${view.id}` : view}
          {#if view?.kind === 'message' && Reader}<Reader id={view.id} service={services.reader} folder={view.folder} {labels} labeling={labeling || loading || busy} {labelError} self={activeAccount?.settings?.address ?? ''} onreply={Composer ? compose : undefined} onlabel={(name, remove) => void label(name, view?.kind === 'message' ? view.id : undefined, remove)} onback={back} />
          {:else if view?.kind === 'draft' && Composer}<Composer {connected} draft={view.draft} service={services.composer} {sender} onback={back} ondirty={(value) => dirty = value} onbusy={(value) => busy = value} onsaved={saved} />
          {:else}
            <section class="welcome">
              <img src={mark} alt="" width="56" height="56" />
              <p>{opening ? 'Opening your draft…' : !Reader ? 'The reader extension is turned off.' : 'Pick a message from the list to read it here.'}</p>
              <p class="hint">Press <kbd>Ctrl</kbd> <kbd>K</kbd> to search commands.</p>
            </section>
          {/if}
        {/key}
      </div>
    {/if}
  </main>
</div>
