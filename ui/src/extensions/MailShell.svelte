<script lang="ts">
  import { onMount } from 'svelte';
  import ExtensionPanel from '../ExtensionPanel.svelte';
  import { mailExtensions } from './registry';
  import AccountSettings from '../AccountSettings.svelte';
  import { createServices, hostService } from '../lib/api';
  import type { Draft, MailItem, AccountInfo, SecurityInfo } from '../lib/api';
  let { extensionStates, hostControls }: { hostControls: import('svelte').Snippet; extensionStates: import('../lib/api').ExtensionState[] } = $props();
  let Mailbox = $derived(extensionStates.find(e => e.manifest.id === 'inbox')?.enabled ? mailExtensions.inbox : null);
  let Reader = $derived(extensionStates.find(e => e.manifest.id === 'reader')?.enabled ? mailExtensions.reader : null);
  let Composer = $derived(extensionStates.find(e => e.manifest.id === 'composer')?.enabled ? mailExtensions.composer : null);
  type View = { kind: 'message'; id: string } | { kind: 'draft'; draft: Draft | null } | null;
  let connected = $state(false);
  let initialized = $state(false);
  let accounts = $state<AccountInfo[]>([]);
  let security = $state<SecurityInfo>({ protected: false, locked: false, cleanup_pending: false });
  let accountId = $state('demo');
  let settingsOpen = $state(false);
  let panelId = $state<string | null>(null);
  function openPanel(id: string) { if (!canLeave()) return; clearView(); ++listGeneration; settingsOpen = false; panelId = id; }
  let accountError = $state('');
  let services = $derived(createServices(accountId));
  let activeAccount = $derived(accounts.find(a => a.id === accountId));
  let sender = $derived(activeAccount?.settings?.address ?? 'Demo account · sending unavailable');
  let accountLabel = $derived(activeAccount?.settings?.name ?? 'Demo account');
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
    settingsOpen = false; folder = 'Inbox'; void loadList();
  }
  function manageAccounts() {
    if (!canLeave()) return;
    clearView(); ++listGeneration; settingsOpen = true;
    void refreshAccounts().catch(e => accountError = (e as Error).message);
  }
  function closeSettings() {
    if (!canLeave()) return;
    clearView(); settingsOpen = false; void loadList();
  }
  let folder = $state<'Inbox' | 'Drafts'>('Inbox');
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
      const result: MailItem[] = requestedFolder === 'Inbox'
        ? (await inboxService.messages()).map(m => ({ id: m.id, name: m.sender, subject: m.subject, preview: m.body, date: m.date }))
        : (await inboxService.drafts()).slice().reverse().map(d => ({ id: d.id, name: d.recipient || 'No recipient', subject: d.subject, preview: d.body, date: 'Draft' }));
      if (generation === listGeneration) items = result;
    } catch (e) { if (generation === listGeneration) error = (e as Error).message; }
    finally { if (generation === listGeneration) loading = false; }
  }
  function canLeave() { return initialized && !busy && (!dirty || window.confirm('Discard unsaved changes? Your last saved draft will be kept.')); }
  function clearView() { panelId = null; ++openGeneration; view = null; dirty = false; openError = ''; opening = false; }
  function navigate(next: 'Inbox' | 'Drafts') {
    if (!canLeave()) return;
    clearView(); settingsOpen = false; folder = next; void loadList();
  }
  function compose() { if (!Composer || !canLeave()) return; clearView(); settingsOpen = false; view = { kind: 'draft', draft: null }; }
  function back() { if (!canLeave()) return; clearView(); }
  async function open(id: string) {
    if (!canLeave()) return;
    clearView();
    if (folder === 'Inbox') { view = { kind: 'message', id }; return; }
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
<div class="app-shell" class:detail-open={view !== null}>
  <aside class="sidebar">
    <a class="brand" href="/" onclick={(event) => { event.preventDefault(); navigate('Inbox'); }} aria-label="Colecto home"><span class="brand-mark" aria-hidden="true">c</span>colecto<span class="brand-dot">.</span></a>
    <button class="compose-button" disabled={busy || !initialized} onclick={compose}><span aria-hidden="true">＋</span> Compose</button>
    <nav aria-label="Mail folders">
      <button class:active={folder === 'Inbox'} aria-current={folder === 'Inbox' ? 'page' : undefined} disabled={busy || !initialized} onclick={() => navigate('Inbox')}><span class="nav-icon" aria-hidden="true">▤</span>Inbox<span class="nav-dot" aria-hidden="true"></span></button>
      <button class:active={folder === 'Drafts'} aria-current={folder === 'Drafts' ? 'page' : undefined} disabled={busy || !initialized} onclick={() => navigate('Drafts')}><span class="nav-icon" aria-hidden="true">✎</span>Drafts</button>
      {#each extensionStates.filter(extension => !extension.bundled && extension.enabled) as extension (extension.manifest.id)}
        <button disabled={busy || !initialized} class:active={panelId === extension.manifest.id} onclick={() => openPanel(extension.manifest.id)}>{extension.manifest.name}</button>
      {/each}
    </nav>
    <div class="account-controls"><label for="current-account">Account</label><select id="current-account" aria-label="Current account" value={accountId} disabled={busy || !initialized} onchange={changeAccount}>{#if !accounts.length}<option value="demo">Demo account</option>{/if}{#each accounts as account (account.id)}<option value={account.id}>{account.settings ? `${account.settings.name} · ${account.settings.address}` : 'Demo account'}</option>{/each}</select><button class="manage-button" disabled={busy || !initialized} onclick={manageAccounts}>Manage accounts</button>{@render hostControls()}<span class="account-state">{security.locked ? 'Credentials locked' : activeAccount?.configured ? 'Configured account' : 'Local sample mail'}</span></div>
  </aside>
  <main class="workspace">
    {#if accountError}<div class="account-error error-box" role="alert">{accountError}<button class="text-button" onclick={() => { accountError = ''; void refreshAccounts().catch(e => accountError = (e as Error).message); }}>Retry account settings</button></div>{/if}
    {#if settingsOpen}<AccountSettings {accounts} {security} onrefresh={refreshAccounts} onback={closeSettings} ondirty={(value) => dirty = value} onbusy={(value) => busy = value} />
    {:else if panelId}{#key `${accountId}:${panelId}`}<ExtensionPanel id={panelId} account={accountId} onback={() => navigate('Inbox')} />{/key}
    {:else}{#if Mailbox}<Mailbox {items} {selected} {folder} {loading} error={error || openError} disabled={busy || opening} onopen={open} onretry={loadList} />{:else}<p class="error-box">Mailbox extension disabled. Open Extensions to enable it.</p>{/if}
    <div class="reading-pane">
      {#key view?.kind === 'message' ? `message:${view.id}` : view}
        {#if view?.kind === 'message' && Reader}<Reader id={view.id} service={services.reader} {accountLabel} onback={back} />
        {:else if view?.kind === 'draft' && Composer}<Composer {connected} draft={view.draft} service={services.composer} {sender} onback={back} ondirty={(value) => dirty = value} onbusy={(value) => busy = value} onsaved={saved} />
        {:else}
          <section class="welcome"><div class="eyebrow">{accountLabel}</div><h2>Your mail, in focus.</h2><p>{opening ? 'Opening your draft…' : 'Select a message to read, or compose a new one.'}</p><button class="secondary" disabled={busy || !initialized} onclick={compose}>New message</button></section>
        {/if}
      {/key}
    </div>{/if}
  </main>
</div>
