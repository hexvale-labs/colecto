<script lang="ts">
  import ServerSettings from '../../ServerSettings.svelte';
  import CommandRunner from '../../CommandRunner.svelte';
  import type { Command, MessageHit } from '../../CommandRunner.svelte';
  import type { Labels } from '../../lib/api';
  import { initial, hue } from '../../lib/initial';
  import Icon from '../../Icon.svelte';
  import Select from '../../Select.svelte';
  import mark from '../../assets/logo-square.svg';
  import wordmark from '../../assets/wordmark.svg';
  import { setAccessToken } from '../../lib/api';
  import { onMount, tick } from 'svelte';
  import ExtensionPanel from '../../ExtensionPanel.svelte';
  import DockArea, { type PaneInfo } from '../../DockArea.svelte';
  import { activate as showPane, defaultLayout, dock, dockEdge, groupOf, parseLayout, reconcile, setVisible, type Edge, type Layout } from '../../lib/layout';
  import { aiExtension, mailExtensions } from '../registry';
  import ApiKeys from '../../ApiKeys.svelte';
  import SendApprovals from '../../SendApprovals.svelte';
  import ExtensionSettings from './ExtensionSettings.svelte';
  import AccountSettings from '../../AccountSettings.svelte';
  import { applyTypography, DEFAULT_FONT, DEFAULT_TEXT_SIZE } from '../../lib/typography';
  import { setBusAccount } from '../../lib/bus';
  import { createServices, hostService } from '../../lib/api';
  import type { Draft, MailItem, AccountInfo, SecurityInfo } from '../../lib/api';
  import type { Workspace } from '../../App.svelte';
  import NotificationSettings from '../../NotificationSettings.svelte';
  import PrivacySettings from '../../PrivacySettings.svelte';
  import AppearanceSettings from '../../AppearanceSettings.svelte';
  import { notify, dismissKey, notificationPosition, type Toast } from '../notifications/store.svelte';
  import { accountOn, subscribeEvents, type ListDensity, type NotificationPreferences, type PreferencesInput } from '../../lib/api';
  import { destinations, isDestination, resolve, settingsPages, type Destination, type Folder, type SettingsPage } from '../../lib/navigation';
  let { extensionStates, extensionSettings, onsettings }: { extensionSettings: import('svelte').Snippet<[Workspace?]>; onsettings: () => void; extensionStates: import('../../lib/api').ExtensionState[] } = $props();
  let Mailbox = $derived(extensionStates.find(e => e.manifest.id === 'inbox')?.enabled ? mailExtensions.inbox : null);
  let Reader = $derived(extensionStates.find(e => e.manifest.id === 'reader')?.enabled ? mailExtensions.reader : null);
  let Composer = $derived(extensionStates.find(e => e.manifest.id === 'composer')?.enabled ? mailExtensions.composer : null);
  // Optional AI actions in the reader; the reader itself knows nothing about them.
  let aiOn = $derived(!!extensionStates.find(e => e.manifest.id === 'ai')?.enabled);
  let switcher = $derived(!!extensionStates.find(e => e.manifest.id === 'accounts')?.enabled);
  type View = { kind: 'message'; id: string; folder: 'Inbox' | 'Sent' } | { kind: 'draft'; draft: Draft | null } | null;
  let connected = $state(false);
  let initialized = $state(false);
  let accounts = $state<AccountInfo[]>([]);
  let security = $state<SecurityInfo>({ protected: false, locked: false, cleanup_pending: false });
  let accountId = $state('demo');
  // Settings is a card over the workspace: the workspace stays mounted behind it, so opening it never closes a message or draft.
  let settingsOpen = $state(false);
  // Enabled extensions that declare settings each get a page, grouped by bundled or third-party.
  let extensionPages = $derived(extensionStates.filter(e => e.enabled && e.manifest.settings?.length));
  let currentExtension = $derived(extensionPages.find(e => `extension:${e.manifest.id}` === settingsPage));
  let settingsPage = $state<SettingsPage>('accounts');
  let settingsDirty = $state(false);
  let reviewRequest = $state<string | null>(null);
  let notificationPrefs = $state<NotificationPreferences>({ enabled: true, mail: true, approvals: true, system: false, position: 'top', updates: true });
  let listDensity = $state<ListDensity>('full');
  let dateGroups = $state(true);
  let accent = $state<string | null>(null);
  let remoteImages = $state(false);
  let font = $state(DEFAULT_FONT);
  let textSize = $state(DEFAULT_TEXT_SIZE);
  $effect(() => { void applyTypography(font, textSize); });
  $effect(() => setBusAccount(accountId));
  $effect(() => { if (accent) document.documentElement.style.setProperty('--mint', accent); else document.documentElement.style.removeProperty('--mint'); });
  let accountsEnabled = $state<Record<string, boolean>>({});
  const isOn = (id: string) => accountOn(accountsEnabled, id);
  function savePreferences(input: PreferencesInput) {
    if (input.accent !== undefined) accent = input.accent || null;
    if (input.font) font = input.font;
    if (input.font_size) textSize = input.font_size;
    if (input.notifications) notificationPrefs = input.notifications;
    if (input.remote_images !== undefined) remoteImages = input.remote_images;
    if (input.list_density) listDensity = input.list_density;
    if (input.date_groups !== undefined) dateGroups = input.date_groups;
    void hostService.savePreferences(input).catch(e => notify({ title: 'Could not save your preference', body: (e as Error).message }));
  }
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
    if (!leaveSettings()) return;
    if (compact && view) { if (!canLeave()) return; clearView(); }
    activate('mailbox');
    await tick(); document.getElementById(target)?.focus();
  }
  function keyboard(event: KeyboardEvent) {
    // Escape closes Settings wherever focus is, unless a dialog or listbox above it takes the key first.
    if (event.key === 'Escape' && settingsOpen && !event.defaultPrevented && !document.querySelector('dialog[open]')) { event.preventDefault(); closeSettings(); return; }
    if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'k' && !event.isComposing && !event.repeat) {
      if (document.querySelector('dialog[open]:not(.command-runner)')) return;
      event.preventDefault(); runner.open();
    }
    // A bare "c" composes, unless the user is typing or a dialog is open.
    const target = event.target as HTMLElement | null;
    if (event.key === 'c' && !event.ctrlKey && !event.metaKey && !event.altKey && !event.repeat && !target?.closest('input, textarea, select, [contenteditable], dialog, [role="listbox"]')) { event.preventDefault(); compose(); }
  }
  let serverUrl = $state('');
  let serverError = $state('');
  function visitServer(event: SubmitEvent) {
    event.preventDefault(); serverError = '';
    try { const url = new URL(serverUrl); if (url.protocol !== 'https:' && !(url.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname))) throw new Error(); if (url.username || url.password || url.search || url.hash) throw new Error(); window.location.assign(url.origin); }
    catch { serverError = 'Enter an HTTPS server address, or HTTP for a local server. Do not include credentials or an access token.'; }
  }
  const discardSettings = () => !settingsDirty || window.confirm('Discard unsaved settings changes?');
  function settingsTab(next: SettingsPage) { if (next === settingsPage || !discardSettings()) return; settingsDirty = false; settingsPage = next; if (next === 'extensions') onsettings(); }
  function openSettings(page: SettingsPage = 'accounts') {
    if (busy || !initialized) return;
    if (settingsOpen) { settingsTab(page); return; }
    settingsOpen = true; settingsDirty = false; settingsPage = page;
    if (page === 'extensions') onsettings();
    void refreshAccounts().catch(e => accountError = (e as Error).message);
  }
  /** Close Settings before going elsewhere; false when the owner keeps unsaved settings. */
  function leaveSettings() {
    if (!settingsOpen) return true;
    if (busy || !discardSettings()) return false;
    settingsOpen = false; settingsDirty = false;
    return true;
  }
  function closeSettings() { if (leaveSettings()) void tick().then(() => document.querySelector<HTMLElement>('.topbar [aria-label="Settings"]')?.focus()); }
  function focusCard(node: HTMLElement) { node.focus(); }
  /** Open any page by its shared destination name: the command runner and MCP both come through here. */
  function go(target: Destination) {
    const place = resolve(target);
    if ('settings' in place) openSettings(place.settings);
    else if ('compose' in place) compose();
    else navigate(place.folder);
  }
  function activateTab(tab: typeof tabs[number]) { if (!leaveSettings() || !canLeave()) return; clearView(); invalidateList(); view = { kind: 'message', id: tab.id, folder: tab.folder }; activate('reader'); }
  function closeAllTabs() {
    if (view?.kind === 'message' && !canLeave()) return;
    tabs = tabs.filter(tab => tab.account !== accountId);
    if (view?.kind === 'message') clearView();
  }
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

  // Panes are extension surfaces placed in docks. Hidden panes stay mounted, so moving or switching never loses a draft.
  let thirdParty = $derived(extensionStates.filter(extension => !extension.bundled && extension.enabled && !extension.manifest.capabilities.includes('window.overlay')));
  let panes: PaneInfo[] = $derived([
    { id: 'mailbox', title: 'Mail', icon: 'inbox' }, { id: 'reader', title: 'Reader', icon: 'mail' },
    ...(switcher ? [{ id: 'accounts', title: 'Accounts', icon: 'account' as const }] : []),
    ...thirdParty.map(extension => ({ id: `ext:${extension.manifest.id}`, title: extension.manifest.name, icon: 'extension' as const })),
  ]);
  // The layout lives in the Host's Turso preferences, so it follows the owner to every device.
  let savedLayout = $state<Layout>(defaultLayout());
  let layoutReady = $state(false);
  let layout = $derived(reconcile(savedLayout, panes.map(pane => pane.id)));
  let focus = $state('mailbox');
  let compact = $state(false);
  let panelRuns = $state<Record<string, number>>({});
  let unsaved: Layout | null = null;
  let saving = false;
  function commitLayout(next: Layout) {
    if (next === layout) return;
    savedLayout = next; unsaved = next;
    // Resizing commits many times a second: keep one save in flight and always finish on the latest layout.
    if (saving) return;
    saving = true;
    void (async () => {
      while (unsaved) {
        const workspace = unsaved; unsaved = null;
        await hostService.savePreferences({ workspace }).catch(() => { /* The layout still applies for this session. */ });
      }
      saving = false;
    })();
  }
  function activate(pane: string) { focus = pane; commitLayout(showPane(layout, pane)); }
  function openPanel(id: string) {
    if (!leaveSettings()) return;
    // Each explicit open runs the panel again, so it reflects permission changes.
    panelRuns[id] = (panelRuns[id] ?? 0) + 1; activate(`ext:${id}`);
  }
  const shownPane = (pane: string) => !!groupOf(layout, pane);
  const workspace: Workspace = { shown: shownPane, toggle: (pane, on) => commitLayout(setVisible(layout, pane, on)) };
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
  let accountOptions = $derived(accounts.some(account => isOn(account.id)) ? accounts.filter(account => isOn(account.id)).map(account => ({ value: account.id, label: account.settings?.name || account.settings?.address || 'Demo account', hint: !account.settings ? 'Sample mail' : account.settings.name ? account.settings.address : account.settings.protocol === 'jmap' ? 'JMAP' : account.settings.imap_host })) : [{ value: 'demo', label: 'Demo account', hint: 'Sample mail' }]);
  const folders = [{ name: 'Inbox', icon: 'inbox' }, { name: 'Drafts', icon: 'drafts' }, { name: 'Sent', icon: 'sent' }] as const;
  function selectAccount(next: string) {
    if (!accounts.some(account => account.id === next) || !leaveSettings() || !canLeave()) return;
    switchAccount(next);
  }
  function switchAccount(next: string) {
    clearView(); invalidateList(); items = []; fetchedAt = null; labels = {}; labelFilter = ''; labelError = '';
    accountId = next; void hostService.savePreferences({ account: next }).catch(() => { /* Selection still applies for this session. */ });
    connected = accounts.find(a => a.id === next)?.connected ?? false;
    query = ''; appliedQuery = ''; folder = 'Inbox'; void loadList();
  }
  /** Turning an account off hides it and stops its background sync; Settings stays open. */
  function toggleAccount(id: string, on: boolean) {
    accountsEnabled = { ...accountsEnabled, [id]: on };
    savePreferences({ accounts_enabled: { [id]: on } });
    if (!on && id === accountId) { const next = accounts.find(account => isOn(account.id)); if (next && canLeave()) switchAccount(next.id); }
    else if (on && !isOn(accountId) && canLeave()) switchAccount(id);
  }
  let folder = $state<Folder>('Inbox');
  let items = $state<MailItem[]>([]);
  let view = $state<View>(null);
  let loading = $state(true);
  let fetching = $state(false);
  let searching = $state(false);
  let fetchedAt = $state<number | null>(null);
  let error = $state('');
  let openError = $state('');
  let dirty = $state(false);
  let busy = $state(false);
  let opening = $state(false);
  let listGeneration = 0;
  let openGeneration = 0;
  let selected = $derived(view?.kind === 'message' ? view.id : view?.kind === 'draft' ? view.draft?.id ?? null : null);
  const sorts = { newest: 'Newest first', oldest: 'Oldest first', sender: 'Sender A–Z', subject: 'Subject A–Z' } as const;
  type Sort = keyof typeof sorts;
  let sort = $state<Sort>('newest');
  // Providers list newest first, so date order is the list order; parsing dates would misplace short ones.
  let visibleItems = $derived.by(() => {
    // While a search is on its way, narrow the current list at once so typing feels live.
    const typed = query.trim().toLocaleLowerCase();
    const words = typed && typed !== appliedQuery.toLocaleLowerCase() ? typed.split(/\s+/) : [];
    const filtered = items.filter(item => (!labelFilter || labels[labelFilter]?.includes(item.id))
      && words.every(word => `${item.name} ${item.subject} ${item.preview}`.toLocaleLowerCase().includes(word)));
    if (sort === 'newest') return filtered;
    if (sort === 'oldest') return filtered.slice().reverse();
    const key = (item: MailItem) => (sort === 'sender' ? item.name : item.subject).toLocaleLowerCase();
    return filtered.slice().sort((a, b) => key(a).localeCompare(key(b)));
  });
  let hits: MessageHit[] = $derived(items.map(item => ({ id: item.id, name: item.name, subject: item.subject, preview: item.preview })));
  function searchFor(text: string) { query = text; void searchMail().then(() => void runSearch()); }
  // Search follows typing after a short pause, and never refreshes the whole mailbox.
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  function scheduleSearch() { clearTimeout(searchTimer); searchTimer = setTimeout(() => void runSearch(), 300); }
  async function runSearch() {
    clearTimeout(searchTimer);
    if (busy || labeling || !initialized) return;
    if (query.trim() === appliedQuery && !error) return;
    await loadList(true);
  }
  let commands: Command[] = $derived(([
    ...destinations.map(destination => ({ id: destination.target, name: destination.name, run: () => go(destination.target), disabled: destination.target === 'compose' && !Composer })),
    { id: 'search', name: 'Search emails', run: () => void searchMail(), disabled: !Mailbox },
    ...(Object.keys(sorts) as Sort[]).filter(next => next !== sort).map(next => ({ id: `sort-${next}`, name: `Sort: ${sorts[next]}`, run: () => { sort = next; activate('mailbox'); }, disabled: !Mailbox })),
    ...(labelFilter || sort !== 'newest' || appliedQuery ? [{ id: 'clear-filters', name: 'Clear search, sorting and filters', run: () => { sort = 'newest'; labelFilter = ''; if (appliedQuery) { query = ''; void runSearch(); } } }] : []),
    { id: 'new-label', name: 'Create a local label', run: () => void searchMail('new-label'), disabled: !Reader },
    ...Object.keys(labels).filter(() => view?.kind === 'message').map(name => ({ id: `assign-${name}`, name: `${labels[name].includes(view?.kind === 'message' ? view.id : '') ? 'Remove' : 'Add'} label: ${name}`, run: () => { if (view?.kind === 'message') void label(name, view.id, labels[name].includes(view.id)); }, disabled: loading })),
    { id: 'refresh', name: 'Refresh current mailbox', run: () => void loadList(), disabled: settingsOpen },
    { id: 'back', name: 'Back to message list', run: back },
    ...panes.flatMap(pane => [
      ...(['left', 'right', 'top', 'bottom'] as Edge[]).map(edge => ({ id: `edge-${pane.id}-${edge}`, name: `Layout: Dock ${pane.title} along the ${edge} edge`, run: () => { commitLayout(dockEdge(layout, pane.id, edge)); focus = pane.id; }, disabled: settingsOpen || compact })),
      ...panes.filter(other => other.id !== pane.id && shownPane(other.id) && groupOf(layout, other.id) !== groupOf(layout, pane.id)).map(other => ({ id: `tab-${pane.id}-${other.id}`, name: `Layout: Put ${pane.title} in a tab beside ${other.title}`, run: () => { const group = groupOf(layout, other.id); if (group) commitLayout(dock(layout, pane.id, group.id, 'center')); }, disabled: settingsOpen || compact })),
      { id: `show-${pane.id}`, name: `Layout: ${shownPane(pane.id) ? 'Hide' : 'Show'} ${pane.title}`, run: () => commitLayout(setVisible(layout, pane.id, !shownPane(pane.id))), disabled: settingsOpen },
    ]),
    { id: 'layout-reset', name: 'Layout: Reset to default', run: () => commitLayout(defaultLayout()), disabled: settingsOpen || compact },
    ...accounts.filter(account => isOn(account.id)).map(account => ({ id: `account-${account.id}`, name: `Switch account: ${account.settings?.name || account.settings?.address || 'Demo account'}`, run: () => selectAccount(account.id) })),
    ...currentTabs.map(tab => ({ id: `tab-${tab.id}`, name: `Open message: ${tab.subject}`, run: () => activateTab(tab) })),
    ...extensionStates.filter(extension => !extension.bundled && extension.enabled && !extension.manifest.capabilities.includes('window.overlay')).map(extension => ({ id: `extension-${extension.manifest.id}`, name: `Open extension: ${extension.manifest.name}`, run: () => openPanel(extension.manifest.id) })),
    ...Object.keys(labels).map(name => ({ id: `label-${name}`, name: `Filter label: ${name}`, run: () => { navigate('Inbox'); if (!dirty && folder === 'Inbox' && !settingsOpen) labelFilter = name; } })),
    ...(view?.kind === 'draft' ? ['Save draft', ...(connected ? ['Send message'] : [])].map(name => ({ id: name, name, run: () => { document.querySelector<HTMLButtonElement>(`.composer [data-command="${name === 'Save draft' ? 'save' : 'send'}"]`)?.click(); } })) : []),
    ...(view?.kind === 'message' ? [{ id: 'close-tab', name: 'Close current message tab', run: () => { if (view?.kind === 'message') void closeTab(view.id); } }] : []),
    ...(currentTabs.length > 1 ? [{ id: 'close-all-tabs', name: 'Close all message tabs', run: closeAllTabs }] : []),
  ] as Command[]).map(command => ({ ...command, disabled: command.disabled || busy || labeling || !initialized })));
  function invalidateList() { ++listGeneration; loading = false; fetching = false; searching = false; }
  /** Refresh from the provider, or with `searchOnly`, apply the search box without refreshing the folder. */
  async function loadList(searchOnly = false) {
    if (labeling) return;
    const generation = ++listGeneration;
    if (!Mailbox) { items = []; loading = false; return; }
    const requestedFolder = folder;
    const requestedQuery = query.trim();
    const requestedAccount = accountId;
    const inboxService = services.inbox;
    error = ''; openError = '';
    if (searchOnly) searching = true; else { loading = items.length === 0; fetching = true; }
    try {
      if (requestedFolder !== 'Drafts' && !requestedQuery) {
        const snapshot = await inboxService.cached(requestedFolder === 'Sent' ? 'sent' : 'inbox');
        if (generation !== listGeneration || requestedAccount !== accountId) return;
        items = snapshot.messages.map(m => ({ id: m.id, name: m.sender, subject: m.subject, preview: m.body, date: m.date }));
        fetchedAt = snapshot.fetched_at; loading = false;
        // Clearing a search goes back to the saved folder; it does not fetch it again.
        if (searchOnly) { appliedQuery = ''; return; }
      }
      const normalizedQuery = requestedQuery.toLocaleLowerCase();
      const [nextLabels, result] = await Promise.all([
        searchOnly ? labels : inboxService.labels(),
        requestedFolder !== 'Drafts'
          ? inboxService.messages(requestedFolder === 'Sent' ? 'sent' : 'inbox', requestedQuery).then(messages => messages.map(m => ({ id: m.id, name: m.sender, subject: m.subject, preview: m.body, date: m.date })))
          : inboxService.drafts().then(drafts => drafts.filter(draft => [draft.recipient, draft.subject, draft.body].some(text => text.toLocaleLowerCase().includes(normalizedQuery))).slice().reverse().map(d => ({ id: d.id, name: d.recipient || 'No recipient', subject: d.subject, preview: d.body, date: 'Draft' }))),
      ]);
      if (generation === listGeneration && requestedAccount === accountId) { items = result; labels = nextLabels; appliedQuery = requestedQuery; if (!requestedQuery) fetchedAt = Math.floor(Date.now() / 1000); }
    } catch (e) { if (generation === listGeneration) error = (e as Error).message; }
    finally { if (generation === listGeneration) { loading = false; fetching = false; searching = false; } }
  }
  /** The Host already refreshed after new mail arrived, so its saved snapshot is current. */
  async function syncFromCache() {
    if (!Mailbox || folder === 'Drafts' || appliedQuery || query.trim() || fetching || searching) return;
    const generation = listGeneration; const account = accountId; const requested = folder;
    try {
      const snapshot = await services.inbox.cached(requested === 'Sent' ? 'sent' : 'inbox');
      if (generation !== listGeneration || account !== accountId || requested !== folder) return;
      items = snapshot.messages.map(m => ({ id: m.id, name: m.sender, subject: m.subject, preview: m.body, date: m.date }));
      fetchedAt = snapshot.fetched_at;
    } catch { /* The next refresh shows it. */ }
  }
  const nameOf = (account?: AccountInfo) => account?.settings?.name || account?.settings?.address || 'Demo account';
  /** Show an in-app notification, and a desktop one too while Colecto is in the background. */
  $effect(() => { notificationPosition.value = notificationPrefs.position; });
  function alert(category: 'mail' | 'approvals', toast: Omit<Toast, 'id' | 'duration'> & { duration?: number }) {
    if (!notificationPrefs.enabled || !notificationPrefs[category]) return;
    notify(toast);
    if (notificationPrefs.system && !document.hasFocus() && typeof Notification === 'function' && Notification.permission === 'granted') {
      const desktop = new Notification(toast.title, { body: toast.body, tag: toast.key });
      desktop.onclick = () => { window.focus(); desktop.close(); };
    }
  }
  async function onMail(event: { account?: string; ids?: string[] }) {
    if (typeof event.account !== 'string' || !Array.isArray(event.ids) || !event.ids.length) return;
    const account = event.account; const ids = event.ids;
    if (account === accountId && folder === 'Inbox') await syncFromCache();
    const fresh = account === accountId ? items.filter(item => ids.includes(item.id)) : [];
    const single = ids.length === 1 && fresh.length === 1 ? fresh[0] : null;
    alert('mail', {
      title: single ? single.name || 'New message' : `${ids.length} new message${ids.length === 1 ? '' : 's'}`,
      body: single ? single.subject || '(No subject)' : nameOf(accounts.find(item => item.id === account)),
      actions: [{ label: single ? 'Open' : 'Show', primary: true, run: () => {
        if (account !== accountId) selectAccount(account);
        else if (single) void open(single.id);
        else navigate('Inbox');
      } }],
    });
  }
  function onApproval(event: { id?: string; account?: string; from?: string; subject?: string }) {
    if (typeof event.id !== 'string') return;
    const id = event.id; const key = `approval:${id}`;
    const decide = async (decision: 'approve' | 'reject') => {
      try {
        const result = await hostService.decideSend(id, decision);
        notify({ title: result.status === 'sent' ? 'Message sent' : result.status === 'rejected' ? 'Request rejected' : `Request ${result.status}`, body: result.status === 'uncertain' ? 'Delivery may have started. Check Sent before trying again.' : event.subject || undefined });
      } catch (e) { notify({ title: 'Could not record your decision', body: (e as Error).message, actions: [{ label: 'Review', run: () => openSettings('send-approvals') }], duration: 0 }); }
    };
    alert('approvals', {
      key, duration: 0, title: 'Automation wants to send a message',
      body: `“${event.subject || '(No subject)'}” from ${event.from || nameOf(accounts.find(item => item.id === event.account))}`,
      actions: [{ label: 'Review message', primary: true, run: () => { reviewRequest = id; dismissKey(key); openSettings('send-approvals'); } }, { label: 'Reject', run: () => decide('reject') }],
    });
  }
  function onNavigate(event: { target?: unknown; account?: string }) {
    if (!isDestination(event.target)) return;
    const target = event.target;
    if (!target.startsWith('settings/') && event.account && event.account !== accountId && accounts.some(item => item.id === event.account)) selectAccount(event.account);
    go(target);
  }
  function canLeave() { return initialized && !busy && !labeling && (!dirty || window.confirm('Discard unsaved changes? Your last saved draft will be kept.')); }
  function clearView() { ++openGeneration; view = null; dirty = false; openError = ''; opening = false; }
  function navigate(next: Folder) {
    if (!leaveSettings() || !canLeave()) return;
    clearView(); query = ''; labelFilter = '';
    if (next !== folder) { items = []; fetchedAt = null; appliedQuery = ''; }
    folder = next; activate('mailbox'); void loadList();
  }
  function compose(draft: Draft | null = null) { if (!Composer || !leaveSettings() || !canLeave()) return; clearView(); view = { kind: 'draft', draft }; activate('reader'); }
  function back() { if (!canLeave()) return; clearView(); activate('mailbox'); }
  async function open(id: string) {
    if (!canLeave()) return;
    if (folder !== 'Drafts') {
      if (!currentTabs.some(tab => tab.id === id)) {
        if (currentTabs.length >= 30) { openError = 'You have 30 messages open in this account. Close a tab to open another.'; return; }
        tabs = [...tabs, { account: accountId, id, subject: items.find(item => item.id === id)?.subject || '(No subject)', folder }];
      }
      clearView(); view = { kind: 'message', id, folder }; activate('reader'); return;
    }
    clearView();
    const generation = ++openGeneration;
    opening = true;
    try {
      const draft = await services.composer.read(id);
      if (generation === openGeneration) { view = { kind: 'draft', draft }; activate('reader'); }
    } catch (e) { if (generation === openGeneration) openError = (e as Error).message; }
    finally { if (generation === openGeneration) opening = false; }
  }
  function saved(draft: Draft) {
    // Keep the mounted editor intact; refreshing the list must not reset its state.
    if (view?.kind === 'draft') view.draft = draft;
    if (folder === 'Drafts') void loadList();
  }
  function beforeUnload(event: BeforeUnloadEvent) { if (dirty || settingsDirty || busy) { event.preventDefault(); event.returnValue = ''; } }
  const hour = new Date().getHours();
  const greeting = hour < 5 ? 'Up late?' : hour < 12 ? 'Good morning.' : hour < 18 ? 'Good afternoon.' : 'Good evening.';
  onMount(() => {
    const narrow = window.matchMedia('(max-width: 800px)');
    const sync = () => compact = narrow.matches; sync(); narrow.addEventListener('change', sync);
    const preferences = hostService.preferences().catch(() => null);
    void preferences.then(saved => {
      savedLayout = parseLayout(saved?.workspace); layoutReady = true;
      accent = saved?.accent ?? null;
      font = saved?.font || DEFAULT_FONT; textSize = saved?.font_size || DEFAULT_TEXT_SIZE;
      if (saved?.notifications) notificationPrefs = saved.notifications;
      remoteImages = saved?.remote_images ?? false;
      accountsEnabled = saved?.accounts_enabled ?? {};
      listDensity = saved?.list_density ?? 'full'; dateGroups = saved?.date_groups ?? true;
    });
    // Events act on accounts and pages, so they start once both are known.
    let stopEvents = () => {};
    const listen = () => { stopEvents = subscribeEvents((name, data) => {
      const event = (data ?? {}) as Record<string, never>;
      if (name === 'mail') void onMail(event);
      else if (name === 'approval') onApproval(event);
      else if (name === 'navigate') onNavigate(event);
    }); };
    void Promise.all([refreshAccounts(), preferences]).then(([, saved]) => {
      accountsEnabled = saved?.accounts_enabled ?? {};
      if (accounts.some(a => a.id === saved?.account && isOn(a.id))) accountId = saved!.account!;
      else accountId = accounts.find(a => isOn(a.id))?.id ?? accountId;
      connected = accounts.find(a => a.id === accountId)?.connected ?? false;
    }).catch(e => accountError = (e as Error).message).finally(() => { initialized = true; void loadList(); listen(); });
    return () => { narrow.removeEventListener('change', sync); stopEvents(); clearTimeout(searchTimer); };
  });
</script>

<svelte:window onbeforeunload={beforeUnload} onkeydown={keyboard} />
<CommandRunner bind:this={runner} {commands} messages={Mailbox ? hits : []} scope={folder} onopen={(id) => void open(id)} onsearch={Mailbox ? searchFor : undefined} />
{#snippet accountSwitcher(placement: 'above' | 'below')}
  <Select label="Current account" value={accountId} options={accountOptions} disabled={busy || !initialized} {placement} onchange={selectAccount}>
    {#snippet trigger(current)}
      <span class="avatar" style:--hue={hue(current?.label ?? accountLabel)} aria-hidden="true">{initial(current?.label ?? accountLabel)}</span>
      <span class="account-name"><span class="account-title">{current?.label ?? accountLabel}</span><small><span class="status-dot" class:on={connected && !security.locked} aria-hidden="true"></span><span class="status-text">{security.locked ? 'Credentials locked' : activeAccount?.configured ? connected ? 'Connected' : 'Not connected' : 'Local sample mail'}</span></small></span>
    {/snippet}
  </Select>
{/snippet}
{#snippet composeButton(labelled: boolean)}
  <button class={labelled ? 'compose-button' : 'compose-icon'} aria-label="Compose" title="Compose  C" disabled={busy || !initialized || !Composer} onclick={() => compose()}><Icon name="compose" />{#if labelled}<span>Compose</span>{/if}</button>
{/snippet}
<div class="app-shell" class:compact inert={settingsOpen}>
  <header class="topbar">
    <a class="brand" href="/" aria-label="Colecto, go to Inbox" onclick={(event) => { event.preventDefault(); navigate('Inbox'); }}><img class="brand-mark" src={mark} alt="" width="24" height="24" /><img class="brand-word" src={wordmark} alt="" width="69" height="16" /></a>
    <button class="command-trigger" aria-label="Commands" title="Commands  Ctrl K" onclick={() => runner.open()}><Icon name="search" size={15} /><span class="command-hint">Search mail or run a command</span><span class="command-keys" aria-hidden="true"><kbd>Ctrl</kbd><kbd>K</kbd></span></button>
    <div class="topbar-end">
      {#if !Mailbox || !shownPane('mailbox')}{@render composeButton(true)}{/if}
      {#if compact && switcher}{@render accountSwitcher('below')}{/if}
      <button class="icon-button" class:active={settingsOpen} aria-label="Settings" title="Settings" disabled={busy || !initialized} onclick={() => openSettings()}><Icon name="settings" size={18} /></button>
    </div>
  </header>
  {#if accountError}<div class="account-error error-box" role="alert">{accountError}<button class="text-button" onclick={() => { accountError = ''; void refreshAccounts().catch(e => accountError = (e as Error).message); }}>Retry account settings</button></div>{/if}
  {#if layoutReady}<DockArea {layout} {panes} focus={panes.some(pane => pane.id === focus) ? focus : 'mailbox'} {compact} onchange={commitLayout} onactivate={activate}>
    {#snippet header(pane)}{#if pane === 'reader'}{@render messageTabs()}{/if}{/snippet}
    {#snippet content(pane)}
      {#if pane === 'mailbox'}{@render mailboxPane()}
      {:else if pane === 'reader'}{@render readerPane()}
      {:else if pane === 'accounts'}<div class="account-pane">{@render accountSwitcher('above')}</div>
      {:else}
        {@const id = pane.slice(4)}
        {#if panelRuns[id] || groupOf(layout, pane)?.active === pane}{#key `${accountId}:${id}:${panelRuns[id] ?? 0}`}<ExtensionPanel {id} account={accountId} onback={() => activate('mailbox')} />{/key}{/if}
      {/if}
    {/snippet}
  </DockArea>{/if}
  {#if compact}{@render folderNav()}{/if}
</div>

{#if settingsOpen}
  {@const current = settingsPages.find(page => page.id === settingsPage)}
  <!-- Pressing the dimmed backdrop closes Settings, like Escape; the card itself is the dialog. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="settings-overlay" onpointerdown={(event) => { if (event.target === event.currentTarget) closeSettings(); }}>
    <div class="settings-window" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1" use:focusCard>
      <nav class="settings-nav" aria-label="Settings sections">
        <h1 id="settings-title">Settings</h1>
        <div class="nav-group" role="group" aria-labelledby="settings-built-in">
          <span class="nav-group-label" id="settings-built-in">Built-in</span>
          {#each settingsPages as page (page.id)}<button class="nav-item" class:active={settingsPage === page.id} aria-current={settingsPage === page.id ? 'page' : undefined} disabled={busy} onclick={() => settingsTab(page.id)}><Icon name={page.icon} /><span class="nav-label">{page.name}</span></button>{/each}
          {#each extensionPages.filter(e => e.bundled) as extension (extension.manifest.id)}{@render extensionNav(extension)}{/each}
        </div>
        {#if extensionPages.some(e => !e.bundled)}
          <div class="nav-group" role="group" aria-labelledby="settings-extensions">
            <span class="nav-group-label" id="settings-extensions">Extensions</span>
            {#each extensionPages.filter(e => !e.bundled) as extension (extension.manifest.id)}{@render extensionNav(extension)}{/each}
          </div>
        {/if}
      </nav>
      <div class="settings-main">
        <header class="settings-head"><p class="settings-crumb"><span>Settings</span><span aria-hidden="true">/</span><strong>{current?.name ?? currentExtension?.manifest.name ?? 'Extension'}</strong></p><button class="icon-button" aria-label="Close settings" title="Close  Esc" disabled={busy} onclick={closeSettings}><Icon name="close" size={16} /></button></header>
        <div class="settings-page">
          {#if settingsPage.startsWith('extension:')}{#if currentExtension}{#key settingsPage}<ExtensionSettings manifest={currentExtension.manifest} />{/key}{:else}<p class="content-note">This extension is off or was removed.</p>{/if}
          {:else if settingsPage === 'accounts'}<AccountSettings {accounts} {security} enabled={isOn} ontoggle={toggleAccount} onrefresh={refreshAccounts} ondirty={(value) => settingsDirty = value} onbusy={(value) => busy = value} />
          {:else if settingsPage === 'appearance'}<AppearanceSettings value={accent} {font} {textSize} density={listDensity} {dateGroups} onchange={(next) => savePreferences({ accent: next })} ontypography={savePreferences} onlist={savePreferences} />
          {:else if settingsPage === 'notifications'}<NotificationSettings value={notificationPrefs} onchange={(next) => savePreferences({ notifications: next })} />
          {:else if settingsPage === 'privacy'}<PrivacySettings {remoteImages} onchange={(next) => savePreferences({ remote_images: next })} />
          {:else if settingsPage === 'servers'}<ServerSettings />
          {:else if settingsPage === 'send-approvals'}<SendApprovals requestId={reviewRequest} />
          {:else if settingsPage === 'ai'}<aiExtension.settings account={accountId} accountName={accountLabel} extensionOn={aiOn} />
          {:else if settingsPage === 'api-keys'}<ApiKeys {accounts} />
          {:else}{@render extensionSettings(workspace)}{/if}
        </div>
      </div>
    </div>
  </div>
{/if}


{#snippet aiActions(slot: import('../../lib/reader-slots').ReaderSlot)}<aiExtension.actions reader={slot} account={accountId} />{/snippet}
{#snippet aiSummary(slot: import('../../lib/reader-slots').ReaderSlot)}<aiExtension.summary reader={slot} account={accountId} />{/snippet}
{#snippet extensionNav(extension: import('../../lib/api').ExtensionState)}
  {@const id = `extension:${extension.manifest.id}` as const}
  <button class="nav-item" class:active={settingsPage === id} aria-current={settingsPage === id ? 'page' : undefined} disabled={busy} onclick={() => settingsTab(id)}><span class="nav-badge" aria-hidden="true">{extension.manifest.name.slice(0, 1).toUpperCase()}</span><span class="nav-label">{extension.manifest.name}</span></button>
{/snippet}
{#snippet folderNav()}
<nav class="folder-nav" aria-label="Mail folders">
  {#each folders as item (item.name)}<button class:active={folder === item.name} aria-current={folder === item.name ? 'page' : undefined} disabled={busy || !initialized} onclick={() => navigate(item.name)}><Icon name={item.icon} /><span>{item.name}</span>{#if folder === item.name && !loading && items.length}<span class="count" aria-hidden="true">{items.length}</span>{/if}</button>{/each}
  {#each thirdParty as extension (extension.manifest.id)}
    <button disabled={busy || !initialized} class:active={focus === `ext:${extension.manifest.id}`} onclick={() => openPanel(extension.manifest.id)}><Icon name="extension" /><span>{extension.manifest.name}</span></button>
  {/each}
</nav>
{/snippet}

{#snippet mailboxPane()}
  <div class="mail-pane">
    {#if Mailbox}<div class="mail-head"><form class="mail-search" role="search" onsubmit={(event) => { event.preventDefault(); void runSearch(); }}>
      <Icon name="search" />
      <label class="sr-only" for="mail-search">Search {folder}</label>
      <input id="mail-search" type="search" enterkeyhint="search" autocapitalize="none" maxlength="1024" bind:value={query} oninput={scheduleSearch} placeholder={`Search ${folder.toLowerCase()}`} />
    </form>{@render composeButton(false)}</div>{/if}
    {#if !compact}{@render folderNav()}{/if}
    {#if Mailbox && initialized && !isOn(accountId)}
      <p class="pane-note">All your accounts are turned off. <button class="text-button inline" onclick={() => openSettings('accounts')}>Turn one on in Settings → Accounts</button></p>
    {:else if Mailbox}
      {#if sort !== 'newest'}<div class="sort-note"><Icon name="sort" size={14} />Sorted {sorts[sort].toLocaleLowerCase()}<button class="chip-remove plain" aria-label="Clear sorting" onclick={() => sort = 'newest'}><Icon name="close" size={12} /></button></div>{/if}
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
      <Mailbox grouped={dateGroups && (sort === 'newest' || sort === 'oldest')} density={listDensity} items={visibleItems} filtered={!!appliedQuery || !!query.trim() || !!labelFilter} {searching} {selected} {folder} {loading} {fetching} {fetchedAt} error={error || openError} disabled={busy || opening} onopen={open} onretry={() => void loadList()} />
    {:else}<p class="pane-note">The mailbox extension is turned off. Turn it on in Settings → Extensions.</p>{/if}
  </div>
{/snippet}

{#snippet messageTabs()}
    {#if currentTabs.length}<nav class="tab-strip" aria-label="Open messages">
      {#each currentTabs as tab (tab.id)}<div class="message-tab" class:active={view?.kind === 'message' && view.id === tab.id}>
        <button class="tab-title" aria-pressed={view?.kind === 'message' && view.id === tab.id} disabled={busy} onclick={() => activateTab(tab)} title={tab.subject}>{tab.subject}</button>
        <button class="tab-close" aria-label={`Close ${tab.subject}`} disabled={busy} onclick={() => closeTab(tab.id)}><Icon name="close" size={13} /></button>
      </div>{/each}
      {#if currentTabs.length > 1}<button class="tab-close-all" aria-label="Close all message tabs" title="Close all tabs" disabled={busy} onclick={closeAllTabs}><Icon name="close" size={13} /><span>Close all</span></button>{/if}
    </nav>{/if}
{/snippet}

{#snippet readerPane()}
  <div class="stage">
    {#if compact}{@render messageTabs()}{/if}
    <div class="reading-pane">
      {#key view?.kind === 'message' ? `message:${accountId}:${view.id}` : view}
        {#if view?.kind === 'message' && Reader}<Reader id={view.id} service={services.reader} {remoteImages} folder={view.folder} {labels} labeling={labeling || loading || busy} {labelError} self={activeAccount?.settings?.address ?? ''} onreply={Composer ? compose : undefined} onlabel={(name, remove) => void label(name, view?.kind === 'message' ? view.id : undefined, remove)} onback={back} actions={aiOn ? aiActions : undefined} aboveBody={aiOn ? aiSummary : undefined} />
        {:else if view?.kind === 'draft' && Composer}<Composer {connected} draft={view.draft} service={services.composer} {sender} onback={back} ondirty={(value) => dirty = value} onbusy={(value) => busy = value} onsaved={saved} />
        {:else}
          <section class="welcome">
            <img src={mark} alt="" width="48" height="48" />
            {#if opening}<p>Opening your draft…</p>
            {:else if !Reader}<p>The reader extension is turned off.</p>
            {:else}<h2>{greeting}</h2><p>Pick a message and it will open here.</p>{/if}
            <p class="hint">Press <kbd>Ctrl</kbd> <kbd>K</kbd> to jump anywhere, or drag a pane’s tab to rearrange.</p>
          </section>
        {/if}
      {/key}
    </div>
  </div>
{/snippet}

