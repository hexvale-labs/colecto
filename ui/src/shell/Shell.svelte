<script lang="ts">
  // The app frame: top bar, docking workspace, account switcher, Settings, command runner and the
  // chrome around extension surfaces. Mail itself is drawn by extensions (inbox, reader, composer)
  // running in their own Workers; see docs/extensions/ui-model.md.
  import ServerSettings from '../ServerSettings.svelte';
  import CommandRunner, { type Command } from '../CommandRunner.svelte';
  import { initial, hue } from '../lib/initial';
  import Icon, { type IconName } from '../Icon.svelte';
  import Select from '../Select.svelte';
  import mark from '../assets/logo-square.svg';
  import wordmark from '../assets/wordmark.svg';
  import { onMount, tick, untrack } from 'svelte';
  import { claimBack, goBack, settled } from '../lib/back';
  import Surface from '../renderer/Surface.svelte';
  import Lifted from '../renderer/Lifted.svelte';
  import ExtensionDialog from './ExtensionDialog.svelte';
  import { runtimes } from '../runtime/runtimes.svelte';
  import DockArea, { type PaneInfo } from '../DockArea.svelte';
  import { activate as showPane, defaultLayout, dock, dockEdge, groupOf, parseLayout, reconcile, setVisible, type Edge, type Layout } from '../lib/layout';
  import ApiKeys from '../ApiKeys.svelte';
  import SendApprovals from '../SendApprovals.svelte';
  import ExtensionSettings from './ExtensionSettings.svelte';
  import AccountSettings from '../AccountSettings.svelte';
  import { applyTypography, DEFAULT_FONT, DEFAULT_TEXT_SIZE } from '../lib/typography';
  import { applySpacing, NO_SPACING, type TextSpacing } from '../lib/text-spacing.svelte';
  import { hostService, accountOn, subscribeEvents, type AccountInfo, type ExtensionState, type NotificationPreferences, type PreferencesInput, type SecurityInfo } from '../lib/api';
  import type { Workspace } from '../App.svelte';
  import ToastSettings from '../ToastSettings.svelte';
  import AppearanceSettings from '../AppearanceSettings.svelte';
  import { notify, notificationPosition } from './toast-store.svelte';
  import { destinations, isDestination, resolve, settingsPages, type Destination, type SettingsPage } from '../lib/navigation';
  let { extensionStates, extensionSettings, onsettings }: { extensionSettings: import('svelte').Snippet<[Workspace?]>; onsettings: () => void; extensionStates: ExtensionState[] } = $props();

  let connected = $state(false);
  let initialized = $state(false);
  let accounts = $state<AccountInfo[]>([]);
  let security = $state<SecurityInfo>({ protected: false, locked: false, cleanup_pending: false });
  let accountId = $state('demo');
  let accountError = $state('');
  let accountsEnabled = $state<Record<string, boolean>>({});
  const isOn = (id: string) => accountOn(accountsEnabled, id);
  let activeAccount = $derived(accounts.find(a => a.id === accountId));
  let accountLabel = $derived(activeAccount?.settings?.name || activeAccount?.settings?.address || 'Demo account');
  // Settings work that must finish first (saving an account) also blocks leaving.
  let settingsBusy = $state(false);
  let busy = $derived(settingsBusy || runtimes.busy);

  // ---- Extensions ----

  // Bundled panes keep their old ids, so saved layouts survive the move to the runtime.
  const paneIds: Record<string, string> = { inbox: 'mailbox', reader: 'reader' };
  const paneOf = (id: string) => paneIds[id] ?? `ext:${id}`;
  const extensionOf = (pane: string) => Object.entries(paneIds).find(([, value]) => value === pane)?.[0] ?? (pane.startsWith('ext:') ? pane.slice(4) : '');
  let withPanes = $derived(extensionStates.filter(extension => extension.enabled && extension.manifest.pane && extension.manifest.client));
  let panes: PaneInfo[] = $derived([
    ...withPanes.filter(extension => extension.manifest.id === 'inbox').map(extension => ({ id: 'mailbox', title: extension.manifest.name, icon: extension.manifest.pane!.icon as IconName })),
    ...withPanes.filter(extension => extension.manifest.id === 'reader').map(extension => ({ id: 'reader', title: extension.manifest.name, icon: extension.manifest.pane!.icon as IconName })),
    { id: 'accounts', title: 'Accounts', icon: 'account' },
    ...withPanes.filter(extension => !paneIds[extension.manifest.id]).map(extension => ({ id: paneOf(extension.manifest.id), title: extension.manifest.name, icon: extension.manifest.pane!.icon as IconName })),
  ]);
  /** Which extension opens a destination such as `inbox` or `compose` (its `navigation`). */
  const owner = (target: string) => extensionStates.find(extension => extension.enabled && extension.manifest.navigation?.includes(target))?.manifest.id;

  // Client parts run while their extension is on, and always know the selected account.
  $effect(() => { const states = extensionStates; untrack(() => runtimes.sync(states)); });
  $effect(() => {
    const summarize = (item: AccountInfo | undefined, id: string) => ({ id, name: item?.settings?.name ?? '', address: item?.settings?.address ?? '', connected: item?.connected ?? false, configured: item?.configured ?? false });
    const summary = { ...summarize(activeAccount, accountId), connected };
    const all = accounts.filter(item => isOn(item.id)).map(item => summarize(item, item.id));
    const ready = initialized;
    untrack(() => runtimes.setAccount(ready ? summary : null, all));
  });
  $effect(() => () => runtimes.stopAll());

  // ---- Preferences ----

  let notificationPrefs = $state<NotificationPreferences>({ enabled: true, mail: true, approvals: true, system: false, position: 'top', updates: true });
  let accent = $state<string | null>(null);
  let font = $state(DEFAULT_FONT);
  let textSize = $state(DEFAULT_TEXT_SIZE);
  let spacing = $state<TextSpacing>({ ...NO_SPACING });
  $effect(() => { void applyTypography(font, textSize).then(() => runtimes.refreshTheme()); });
  $effect(() => { applySpacing(spacing); });
  $effect(() => { if (accent) document.documentElement.style.setProperty('--mint', accent); else document.documentElement.style.removeProperty('--mint'); runtimes.refreshTheme(); });
  $effect(() => { notificationPosition.value = notificationPrefs.position; });
  function savePreferences(input: PreferencesInput) {
    if (input.accent !== undefined) accent = input.accent || null;
    if (input.font) font = input.font;
    if (input.font_size) textSize = input.font_size;
    if (input.text_spacing) spacing = input.text_spacing;
    if (input.notifications) notificationPrefs = input.notifications;
    void hostService.savePreferences(input).catch(e => notify({ title: 'Could not save your preference', body: (e as Error).message }));
  }

  // ---- Workspace ----

  let savedLayout = $state<Layout>(defaultLayout());
  let layoutReady = $state(false);
  let layout = $derived(reconcile(savedLayout, panes.map(pane => pane.id)));
  let focus = $state('mailbox');
  let compact = $state(false);
  $effect(() => { runtimes.compact = compact; });
  $effect(() => { runtimes.ready = initialized; });
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
  const shownPane = (pane: string) => !!groupOf(layout, pane);
  const workspace: Workspace = { shown: shownPane, toggle: (pane, on) => commitLayout(setVisible(layout, pane, on)) };
  /** Leaving a draft with unsaved changes asks first; the composer keeps it in its own pane otherwise. */
  const canLeave = () => initialized && !busy && (!runtimes.dirty || window.confirm('Discard unsaved changes? Your last saved draft will be kept.'));

  // What extensions may ask the shell to do; deciding a send stays here.
  runtimes.shell = {
    navigate(target, account) {
      if (account && account !== accountId && accounts.some(item => item.id === account)) selectAccount(account);
      if (isDestination(target)) go(target);
    },
    async rejectSend(id) {
      try {
        const result = await hostService.decideSend(id, 'reject');
        notify({ title: result.status === 'rejected' ? 'Request rejected' : `Request ${result.status}`, body: result.status === 'uncertain' ? 'Delivery may have started. Check Sent before trying again.' : undefined });
      } catch (e) { notify({ title: 'Could not record your decision', body: (e as Error).message, actions: [{ label: 'Review', run: () => openSettings('send-approvals') }], duration: 0 }); }
    },
    reviewSend(id) { reviewRequest = id; openSettings('send-approvals'); },
    desktop(title, body, tag) {
      if (!notificationPrefs.system || document.hasFocus() || typeof Notification !== 'function' || Notification.permission !== 'granted') return;
      const desktop = new Notification(title, { body, tag });
      desktop.onclick = () => { window.focus(); desktop.close(); };
    },
    show(id, surface) { if (surface === 'pane' && !settingsOpen) activate(paneOf(id)); },
    credentialsChanged() { runtimes.relisten(); },
    leave() { if (compact) activate('mailbox'); },
  };

  /** Open any page by its shared destination name: the command runner, toasts and MCP all come through here. */
  function go(target: Destination) {
    const place = resolve(target);
    if ('settings' in place) { openSettings(place.settings); return; }
    const id = owner(target);
    if (!id || !leaveSettings()) return;
    runtimes.get(id)?.send({ t: 'navigate', target });
    activate(paneOf(id));
  }

  // ---- Accounts ----

  async function refreshAccounts() {
    const [nextAccounts, nextSecurity] = await Promise.all([hostService.accounts(), hostService.security()]);
    accounts = nextAccounts; security = nextSecurity;
    connected = nextAccounts.find(a => a.id === accountId)?.connected ?? false;
  }
  let accountOptions = $derived(accounts.some(account => isOn(account.id)) ? accounts.filter(account => isOn(account.id)).map(account => ({ value: account.id, label: account.settings?.name || account.settings?.address || 'Demo account', hint: !account.settings ? 'Sample mail' : account.settings.name ? account.settings.address : account.settings.protocol === 'jmap' ? 'JMAP' : account.settings.imap_host })) : [{ value: 'demo', label: 'Demo account', hint: 'Sample mail' }]);
  function selectAccount(next: string) {
    if (!accounts.some(account => account.id === next) || !leaveSettings() || !canLeave()) return;
    switchAccount(next);
  }
  function switchAccount(next: string) {
    accountId = next; void hostService.savePreferences({ account: next }).catch(() => { /* Selection still applies for this session. */ });
    connected = accounts.find(a => a.id === next)?.connected ?? false;
  }
  /** Turning an account off hides it and stops its background sync; Settings stays open. */
  function toggleAccount(id: string, on: boolean) {
    accountsEnabled = { ...accountsEnabled, [id]: on };
    savePreferences({ accounts_enabled: { [id]: on } });
    if (!on && id === accountId) { const next = accounts.find(account => isOn(account.id)); if (next && canLeave()) switchAccount(next.id); }
    else if (on && !isOn(accountId) && canLeave()) switchAccount(id);
  }

  // ---- Settings ----

  // Settings is a card over the workspace: the workspace stays mounted behind it, so opening it never closes a message or draft.
  let settingsOpen = $state(false);
  let settingsPage = $state<SettingsPage>('accounts');
  let settingsDirty = $state(false);
  let reviewRequest = $state<string | null>(null);
  // On a phone Settings is a list of sections that opens one page at a time; wider, the list is
  // a sidebar that collapses to icons (remembered on this device only).
  let phone = $state(false);
  let settingsPageShown = $state(false);
  const NAV_KEY = 'colecto.settingsNavCollapsed';
  let navCollapsed = $state(false);
  function toggleNav() {
    navCollapsed = !navCollapsed;
    try { localStorage.setItem(NAV_KEY, navCollapsed ? '1' : '0'); } catch { /* Kept for this session. */ }
  }
  // Enabled extensions get a section of their own for settings that name no core section, or
  // for a page they draw (a settings page and no declared settings).
  let extensionPages = $derived(extensionStates.filter(e => e.enabled && (e.manifest.settings?.some(setting => !setting.section) || (e.manifest.settings_page && e.manifest.client && !e.manifest.settings?.length))));
  /** Extensions placing settings in a core section (API 2), bundled ones first. */
  const contributors = (section: 'appearance' | 'notifications') => extensionStates
    .filter(e => e.enabled && e.manifest.settings?.some(setting => setting.section === section))
    .sort((a, b) => Number(b.bundled) - Number(a.bundled));
  let currentExtension = $derived(extensionPages.find(e => `extension:${e.manifest.id}` === settingsPage));
  // Bundled sections keep the order the owner knows; third-party ones get their own group.
  const builtInOrder = ['accounts', 'extension:inbox', 'appearance', 'notifications', 'extension:reader', 'servers', 'extensions', 'extension:ai', 'api-keys', 'send-approvals'];
  type NavEntry = { id: SettingsPage; name: string; icon?: IconName; extension?: ExtensionState };
  let builtIn = $derived.by((): NavEntry[] => {
    const entries: NavEntry[] = [
      ...settingsPages.map(page => ({ id: page.id as SettingsPage, name: page.name, icon: page.icon as IconName })),
      ...extensionPages.filter(e => e.bundled).map(e => ({ id: `extension:${e.manifest.id}` as SettingsPage, name: e.manifest.settings_page?.title ?? e.manifest.name, icon: e.manifest.settings_page?.icon as IconName | undefined, extension: e })),
    ];
    const rank = (id: string) => { const at = builtInOrder.indexOf(id); return at < 0 ? builtInOrder.length : at; };
    return entries.sort((a, b) => rank(a.id) - rank(b.id));
  });
  const discardSettings = () => !settingsDirty || window.confirm('Discard unsaved settings changes?');
  function settingsTab(next: SettingsPage) {
    if (next === settingsPage) { settingsPageShown = true; return; }
    if (!discardSettings()) return;
    settingsDirty = false; settingsPage = next; settingsPageShown = true; if (next === 'extensions') onsettings();
  }
  /** Open Settings, at `page` when given; without one a phone shows the list of sections. */
  function openSettings(page?: SettingsPage) {
    if (busy || !initialized) return;
    if (settingsOpen) { if (page) settingsTab(page); return; }
    settingsOpen = true; settingsDirty = false; settingsPage = page ?? 'accounts'; settingsPageShown = !!page;
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

  // Back (the system button or gesture, the browser, the in-app arrows) closes the newest of:
  // Settings, a Settings page on a phone, and on narrow windows whatever replaced the list.
  $effect(() => {
    if (!settingsOpen) return;
    return claimBack({ close: closeSettings, isOpen: () => settingsOpen, surface: () => document.querySelector<HTMLElement>('.settings-window'), push: untrack(() => phone) });
  });
  $effect(() => {
    if (!settingsOpen || !phone || !settingsPageShown) return;
    return claimBack({ close: () => settingsPageShown = false, isOpen: () => settingsOpen && phone && settingsPageShown, surface: () => document.querySelector<HTMLElement>('.settings-main'), under: () => document.querySelector<HTMLElement>('.settings-nav'), push: true });
  });
  $effect(() => {
    if (!compact || !layoutReady || focus === 'mailbox') return;
    const pane = focus;
    return claimBack({
      // A pane that said `back: true` handles it (the reader closes its message); otherwise the
      // mailbox comes back.
      close: () => {
        const runtime = runtimes.get(extensionOf(pane));
        if (runtime?.states.pane?.back) runtime.send({ t: 'back', surface: 'pane' });
        activate('mailbox');
      },
      isOpen: () => compact && focus === pane,
      surface: () => document.getElementById(`pane-${pane}`),
      under: () => document.getElementById('pane-mailbox'),
      push: true,
    });
  });

  // ---- Commands ----

  let runner: CommandRunner;
  // App actions (Compose) sit in the top bar unless a visible pane draws them itself.
  let appActions = $derived(runtimes.list.flatMap(runtime => runtime.appActions.map(action => ({ runtime, action }))));
  let topActions = $derived(appActions.filter(({ runtime, action }) => !runtimes.refs[`${runtime.id}/${action.id}`]));
  function runAction(runtime: (typeof appActions)[number]['runtime'], id: string) { if (leaveSettings()) runtime.send({ t: 'app_action', id }); }
  /** A destination's icon: its folder, or the Settings section's own icon in the sidebar. */
  function destinationIcon(target: Destination): IconName {
    const place = resolve(target);
    if ('compose' in place) return 'compose';
    if ('folder' in place) return place.folder.toLowerCase() as IconName;
    return builtIn.find(entry => entry.id === place.settings)?.icon ?? 'settings';
  }
  let commands: Command[] = $derived(([
    ...destinations.filter(destination => !destination.target.startsWith('settings/') ? owner(destination.target) : true)
      .map(destination => ({ id: destination.target, name: destination.name, icon: destinationIcon(destination.target), run: () => go(destination.target) })),
    ...runtimes.list.flatMap(runtime => runtime.commands.map(command => ({ id: `${runtime.id}:${command.id}`, name: command.name, disabled: command.disabled,
      run: () => { if (leaveSettings()) runtime.send({ t: 'command', id: command.id }); } }))),
    ...panes.flatMap(pane => [
      ...(['left', 'right', 'top', 'bottom'] as Edge[]).map(edge => ({ id: `edge-${pane.id}-${edge}`, icon: 'layout', name: `Layout: Dock ${pane.title} along the ${edge} edge`, run: () => { commitLayout(dockEdge(layout, pane.id, edge)); focus = pane.id; }, disabled: settingsOpen || compact })),
      ...panes.filter(other => other.id !== pane.id && shownPane(other.id) && groupOf(layout, other.id) !== groupOf(layout, pane.id)).map(other => ({ id: `tab-${pane.id}-${other.id}`, icon: 'layout', name: `Layout: Put ${pane.title} in a tab beside ${other.title}`, run: () => { const group = groupOf(layout, other.id); if (group) commitLayout(dock(layout, pane.id, group.id, 'center')); }, disabled: settingsOpen || compact })),
      { id: `show-${pane.id}`, icon: 'layout', name: `Layout: ${shownPane(pane.id) ? 'Hide' : 'Show'} ${pane.title}`, run: () => commitLayout(setVisible(layout, pane.id, !shownPane(pane.id))), disabled: settingsOpen },
    ]),
    { id: 'layout-reset', icon: 'layout', name: 'Layout: Reset to default', run: () => commitLayout(defaultLayout()), disabled: settingsOpen || compact },
    ...accounts.filter(account => isOn(account.id)).map(account => ({ id: `account-${account.id}`, icon: 'account', name: `Switch account: ${account.settings?.name || account.settings?.address || 'Demo account'}`, run: () => selectAccount(account.id) })),
    ...withPanes.filter(extension => !extension.bundled).map(extension => ({ id: `extension-${extension.manifest.id}`, icon: 'extension', name: `Open extension: ${extension.manifest.name}`, run: () => { if (leaveSettings()) activate(paneOf(extension.manifest.id)); } })),
  ] as Command[]).map(command => ({ ...command, disabled: command.disabled || busy || !initialized })).concat(
    // Outside the busy check: reloading is the way out when the app is stuck.
    { id: 'reload', name: 'Reload the app', icon: 'refresh', run: reloadApp, disabled: false },
  ));
  // Mail matching what the owner types comes from the extension that searches mail.
  let searcher = $derived(runtimes.list.find(runtime => runtime.searchScope === 'mail'));
  let seq = 0;
  let hits = $derived(searcher?.hits.hits ?? []);
  let reloading = false;
  function reloadApp() {
    if ((runtimes.dirty || settingsDirty || busy) && !window.confirm('Reload Colecto? Unsaved changes will be lost.')) return;
    // Closing the command runner moves history back; reloading during that move is cancelled.
    reloading = true; void settled().then(() => window.location.reload());
  }

  function keyboard(event: KeyboardEvent) {
    // Escape closes Settings wherever focus is, unless a dialog or listbox above it takes the key first.
    if (event.key === 'Escape' && settingsOpen && !event.defaultPrevented && !document.querySelector('dialog[open]')) { event.preventDefault(); closeSettings(); return; }
    if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'k' && !event.isComposing && !event.repeat) {
      if (document.querySelector('dialog[open]:not(.command-runner)')) return;
      event.preventDefault(); runner.open(); return;
    }
    // App action shortcuts (C composes) fire unless the owner is typing or a dialog is open.
    const target = event.target as HTMLElement | null;
    if (event.altKey || event.metaKey || event.repeat || target?.closest('input, textarea, select, [contenteditable], dialog, [role="listbox"]')) return;
    const key = `${event.ctrlKey ? 'ctrl+' : ''}${event.key.toLowerCase()}`;
    const match = appActions.find(({ action }) => action.shortcut === key && !action.disabled);
    if (match && !busy && initialized && !settingsOpen) { event.preventDefault(); runAction(match.runtime, match.action.id); }
  }
  function beforeUnload(event: BeforeUnloadEvent) { if (!reloading && (runtimes.dirty || settingsDirty || busy)) { event.preventDefault(); event.returnValue = ''; } }

  function onNavigate(event: { target?: unknown; account?: string }) {
    if (!isDestination(event.target)) return;
    const target = event.target;
    if (event.account && event.account !== accountId && accounts.some(item => item.id === event.account)) selectAccount(event.account);
    // Folders and the composer are opened by the extension that owns them, which hears the same event.
    if (target.startsWith('settings/')) go(target);
    else { const id = owner(target); if (id) activate(paneOf(id)); }
  }

  onMount(() => {
    // Android window size classes: compact (< 600 dp) and medium (< 840 dp) show one pane at a time.
    const narrow = window.matchMedia('(max-width: 839.98px)');
    const small = window.matchMedia('(max-width: 599.98px)');
    const sync = () => { compact = narrow.matches; phone = small.matches; }; sync();
    narrow.addEventListener('change', sync); small.addEventListener('change', sync);
    try { const saved = localStorage.getItem(NAV_KEY); navCollapsed = saved ? saved === '1' : narrow.matches; } catch { navCollapsed = narrow.matches; }
    const preferences = hostService.preferences().catch(() => null);
    void preferences.then(saved => {
      savedLayout = parseLayout(saved?.workspace); layoutReady = true;
      accent = saved?.accent ?? null;
      font = saved?.font || DEFAULT_FONT; textSize = saved?.font_size || DEFAULT_TEXT_SIZE;
      spacing = { ...NO_SPACING, ...saved?.text_spacing };
      if (saved?.notifications) notificationPrefs = saved.notifications;
      accountsEnabled = saved?.accounts_enabled ?? {};
    });
    let stopEvents = () => {};
    void Promise.all([refreshAccounts(), preferences]).then(([, saved]) => {
      accountsEnabled = saved?.accounts_enabled ?? {};
      if (accounts.some(a => a.id === saved?.account && isOn(a.id))) accountId = saved!.account!;
      else accountId = accounts.find(a => isOn(a.id))?.id ?? accountId;
      connected = accounts.find(a => a.id === accountId)?.connected ?? false;
    }).catch(e => accountError = (e as Error).message).finally(() => {
      initialized = true;
      stopEvents = subscribeEvents((name, data) => { if (name === 'navigate') onNavigate((data ?? {}) as { target?: unknown; account?: string }); });
    });
    return () => { narrow.removeEventListener('change', sync); small.removeEventListener('change', sync); stopEvents(); };
  });
</script>

<svelte:window onbeforeunload={beforeUnload} onkeydown={keyboard} />
<CommandRunner bind:this={runner} {commands} {hits} scope="mail"
  onquery={searcher ? (query) => runtimes.query('mail', ++seq, query) : undefined}
  onhit={(id) => { if (searcher && leaveSettings()) { searcher.send({ t: 'command', id }); activate(paneOf(searcher.id)); } }}
  onsearch={searcher ? (query) => { if (leaveSettings()) { searcher!.send({ t: 'command_submit', scope: 'mail', query }); activate(paneOf(searcher!.id)); } } : undefined} />
{#snippet accountSwitcher(placement: 'above' | 'below')}
  <Select label="Current account" value={accountId} options={accountOptions} disabled={busy || !initialized} {placement} onchange={selectAccount}>
    {#snippet trigger(current)}
      <span class="avatar" style:--hue={hue(current?.label ?? accountLabel)} aria-hidden="true">{initial(current?.label ?? accountLabel)}</span>
      <span class="account-name"><span class="account-title">{current?.label ?? accountLabel}</span><small><span class="status-dot" class:on={connected && !security.locked} aria-hidden="true"></span><span class="status-text">{security.locked ? 'Credentials locked' : activeAccount?.configured ? connected ? 'Connected' : 'Not connected' : 'Local sample mail'}</span></small></span>
    {/snippet}
  </Select>
{/snippet}
<div class="app-shell" class:compact inert={settingsOpen}>
  <header class="topbar">
    <a class="brand" href="/" aria-label="Colecto, go to Inbox" onclick={(event) => { event.preventDefault(); go('inbox'); }}><img class="brand-mark" src={mark} alt="" width="24" height="24" /><img class="brand-word" src={wordmark} alt="" width="69" height="16" /></a>
    <button class="command-trigger" aria-label="Commands" title="Commands  Ctrl K" onclick={() => runner.open()}><Icon name="search" size={15} /><span class="command-hint">Search mail or run a command</span><span class="command-keys" aria-hidden="true"><kbd>Ctrl</kbd><kbd>K</kbd></span></button>
    <div class="topbar-end">
      {#each topActions as { runtime, action } (`${runtime.id}/${action.id}`)}
        <button class="compose-button" aria-label={action.label} title={action.shortcut ? `${action.label}  ${action.shortcut.toUpperCase()}` : action.label} disabled={action.disabled || busy || !initialized} onclick={() => runAction(runtime, action.id)}><Icon name={(action.icon ?? 'extension') as IconName} /><span>{action.label}</span></button>
      {/each}
      {#if compact}{@render accountSwitcher('below')}{/if}
      <button class="icon-button" class:active={settingsOpen} aria-label="Settings" title="Settings" disabled={busy || !initialized} onclick={() => openSettings()}><Icon name="settings" size={18} /></button>
    </div>
  </header>
  {#if accountError}<div class="account-error error-box" role="alert">{accountError}<button class="text-button" onclick={() => { accountError = ''; void refreshAccounts().catch(e => accountError = (e as Error).message); }}>Retry account settings</button></div>{/if}
  {#if layoutReady}<DockArea {layout} {panes} focus={panes.some(pane => pane.id === focus) ? focus : 'mailbox'} {compact} onchange={commitLayout} onactivate={activate}>
    {#snippet header(pane)}
      {@const runtime = runtimes.get(extensionOf(pane))}
      {#if runtime && !compact && runtime.trees['pane.header']}<Surface {runtime} surface="pane.header" bare />{/if}
    {/snippet}
    {#snippet content(pane)}
      {#if pane === 'accounts'}<div class="account-pane">{@render accountSwitcher('above')}</div>
      {:else}
        {@const runtime = runtimes.get(extensionOf(pane))}
        {#if runtime?.bundled && pane === 'mailbox'}<Surface {runtime} surface="pane" bare />
        {:else if runtime?.bundled}
          <div class="stage">
            {#if compact && runtime.trees['pane.header']}<Surface {runtime} surface="pane.header" bare />{/if}
            <div class="reading-pane"><Surface {runtime} surface="pane" bare /></div>
          </div>
        {:else if runtime}<div class="extension-pane" role="region" aria-label={runtime.name}><Surface {runtime} surface="pane" /></div>{/if}
      {/if}
    {/snippet}
  </DockArea>{/if}
  {#if compact}<Lifted />{/if}
</div>

{#each runtimes.list.filter(runtime => runtime.dialog) as runtime (runtime.id)}<ExtensionDialog {runtime} />{/each}

{#if settingsOpen}
  {@const current = settingsPages.find(page => page.id === settingsPage)}
  {@const pageRuntime = currentExtension ? runtimes.get(currentExtension.manifest.id) : undefined}
  <!-- Pressing the dimmed backdrop closes Settings, like Escape; the card itself is the dialog. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="settings-overlay" onpointerdown={(event) => { if (event.target === event.currentTarget) closeSettings(); }}>
    <div class="settings-window" class:collapsed={navCollapsed && !phone} data-view={phone ? settingsPageShown ? 'page' : 'list' : undefined} role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1" use:focusCard>
      <nav class="settings-nav" aria-label="Settings sections" inert={phone && settingsPageShown}>
        <header class="settings-nav-head">
          {#if phone}<button class="icon-button" aria-label="Close settings" disabled={busy} onclick={() => goBack() || closeSettings()}><Icon name="back" size={20} /></button>{/if}
          <h1 id="settings-title">Settings</h1>
          {#if !phone}<button class="icon-button nav-toggle" aria-label={navCollapsed ? 'Expand the section list' : 'Collapse the section list'} aria-expanded={!navCollapsed} title={navCollapsed ? 'Expand' : 'Collapse'} onclick={toggleNav}><Icon name="sidebar" size={17} /></button>{/if}
        </header>
        <div class="nav-group" role="group" aria-labelledby="settings-built-in">
          <span class="nav-group-label" id="settings-built-in">Built-in</span>
          {#each builtIn as entry (entry.id)}{@render navItem(entry)}{/each}
        </div>
        {#if extensionPages.some(e => !e.bundled)}
          <div class="nav-group" role="group" aria-labelledby="settings-extensions">
            <span class="nav-group-label" id="settings-extensions">Extensions</span>
            {#each extensionPages.filter(e => !e.bundled) as extension (extension.manifest.id)}{@render navItem({ id: `extension:${extension.manifest.id}`, name: extension.manifest.settings_page?.title ?? extension.manifest.name, extension })}{/each}
          </div>
        {/if}
      </nav>
      <div class="settings-main" inert={phone && !settingsPageShown}>
        <header class="settings-head">
          {#if phone}<button class="icon-button" aria-label="Back to Settings" disabled={busy} onclick={() => goBack() || (settingsPageShown = false)}><Icon name="back" size={20} /></button>{/if}
          <p class="settings-crumb"><span>Settings</span><span aria-hidden="true">/</span><strong>{current?.name ?? currentExtension?.manifest.settings_page?.title ?? currentExtension?.manifest.name ?? 'Extension'}</strong></p>
          {#if !phone}<button class="icon-button" aria-label="Close settings" title="Close  Esc" disabled={busy} onclick={closeSettings}><Icon name="close" size={16} /></button>{/if}
        </header>
        <div class="settings-page">
          {#if settingsPage.startsWith('extension:')}
            {#if pageRuntime && currentExtension?.manifest.settings_page && (!currentExtension.manifest.settings?.length || pageRuntime.trees.settings !== undefined)}
              <!-- An extension's own section, always labelled with who drew it. -->
              <section class="settings-section" aria-labelledby="extension-page-title"><header class="section-head"><h2 id="extension-page-title">{currentExtension.manifest.settings_page.title}</h2><p class="extension-from">From {pageRuntime.name}</p></header><Surface runtime={pageRuntime} surface="settings" /></section>
            {:else if currentExtension}{#key settingsPage}<ExtensionSettings manifest={currentExtension.manifest} />{/key}{:else}<p class="content-note">This extension is off or was removed.</p>{/if}
          {:else if settingsPage === 'accounts'}<AccountSettings {accounts} {security} pane={{ shown: shownPane('accounts'), toggle: (on) => workspace.toggle('accounts', on) }} enabled={isOn} ontoggle={toggleAccount} onrefresh={refreshAccounts} ondirty={(value) => settingsDirty = value} onbusy={(value) => settingsBusy = value} />
          {:else if settingsPage === 'appearance'}<AppearanceSettings value={accent} {font} {textSize} {spacing} onchange={(next) => savePreferences({ accent: next })} ontypography={savePreferences}>{#each contributors('appearance') as extension (extension.manifest.id)}<ExtensionSettings manifest={extension.manifest} section="appearance" bundled={extension.bundled} />{/each}</AppearanceSettings>
          {:else if settingsPage === 'notifications'}
            <section class="settings-section" aria-labelledby="notifications-title">
              <header class="section-head"><h2 id="notifications-title">Notifications</h2><p>Choose what Colecto tells you about, where it shows notifications and whether they reach your desktop.</p></header>
              {#each contributors('notifications') as extension (extension.manifest.id)}<ExtensionSettings manifest={extension.manifest} section="notifications" bundled={extension.bundled} />{/each}
              <ToastSettings value={notificationPrefs} onchange={(next) => savePreferences({ notifications: next })} />
            </section>
          {:else if settingsPage === 'servers'}<ServerSettings />
          {:else if settingsPage === 'send-approvals'}<SendApprovals requestId={reviewRequest} />
          {:else if settingsPage === 'api-keys'}<ApiKeys {accounts} />
          {:else}{@render extensionSettings(workspace)}{/if}
        </div>
      </div>
    </div>
  </div>
{/if}

{#snippet navItem(entry: NavEntry)}
  <button class="nav-item" class:active={settingsPage === entry.id} aria-current={settingsPage === entry.id ? 'page' : undefined} title={navCollapsed && !phone ? entry.name : undefined} disabled={busy} onclick={() => settingsTab(entry.id)}>
    {#if entry.icon}<Icon name={entry.icon} />{:else}<span class="nav-badge" aria-hidden="true">{entry.name.slice(0, 1).toUpperCase()}</span>{/if}<span class="nav-label">{entry.name}</span><Icon name="next" />
  </button>
{/snippet}
