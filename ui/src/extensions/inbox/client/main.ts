// The mailbox: the message list of the selected account with search, sorting, labels and
// folders. Opening a message tells the reader over the bus; drafts open in the composer.
import { app, bus, call, clip, type Node, type Row } from '../../../extension-sdk';
import { when } from '../../../lib/when';
import type { AccountSync, Draft, Labels, Message } from '../../../lib/api';

type Folder = 'Inbox' | 'Drafts' | 'Sent';
interface Item { id: string; name: string; subject: string; preview: string; date: string }
const folders: { name: Folder; icon: string }[] = [{ name: 'Inbox', icon: 'inbox' }, { name: 'Drafts', icon: 'drafts' }, { name: 'Sent', icon: 'sent' }];
const sorts = { newest: 'Newest first', oldest: 'Oldest first', sender: 'Sender A–Z', subject: 'Subject A–Z' } as const;
type Sort = keyof typeof sorts;

let account = '';
let folder: Folder = 'Inbox';
let items: Item[] = [];
let labels: Labels = {};
let labelFilter = '';
let labelError = '';
let labeling = false;
let query = '';
let appliedQuery = '';
let searchEpoch = 0;
let labelEpoch = 0;
let sort: Sort = 'newest';
let loading = true;
let fetching = false;
let searching = false;
let fetchedAt: number | null = null;
let error = '';
let selected: string | null = null;
let sync: Record<string, AccountSync> = {};
let arrived: string[] = [];
let generation = 0;
let searchTimer: ReturnType<typeof setTimeout> | undefined;
let newLabel = '';
const cache = new Map<string, { items: Item[]; fetchedAt: number | null }>();
const key = (folderName: Folder) => `${account}\u0000${folderName}`;
const toItems = (messages: Message[]) => messages.map(m => ({ id: m.id, name: m.sender, subject: m.subject, preview: m.body, date: m.date }));
const setting = <T,>(name: string, fallback: T) => (app.settings[name] ?? fallback) as T;

/** The list after the label filter, the search being typed, and sorting. */
function visible() {
  // While a search is on its way, narrow the current list at once so typing feels live.
  const typed = query.trim().toLocaleLowerCase();
  const words = typed && typed !== appliedQuery.toLocaleLowerCase() ? typed.split(/\s+/) : [];
  const filtered = items.filter(item => (!labelFilter || labels[labelFilter]?.includes(item.id))
    && words.every(word => `${item.name} ${item.subject} ${item.preview}`.toLocaleLowerCase().includes(word)));
  if (sort === 'newest') return filtered;
  if (sort === 'oldest') return filtered.slice().reverse();
  const by = (item: Item) => (sort === 'sender' ? item.name : item.subject).toLocaleLowerCase();
  return filtered.slice().sort((a, b) => by(a).localeCompare(by(b)));
}

function show(list: Item[], at: number | null) {
  // Messages that arrive with a refresh slide in, a few at a time; a first load shows all at once.
  const before = new Set(items.map(item => item.id));
  arrived = before.size && !query.trim() && !labelFilter ? list.filter(item => !before.has(item.id)).slice(0, 8).map(item => item.id) : [];
  items = list; fetchedAt = at;
  cache.set(key(folder), { items: list, fetchedAt: at });
}

/** Tell the reader which messages to keep ready, as the owner chose in Settings → Mail. */
function warm(list: Item[]) {
  if (folder === 'Drafts' || !setting('prefetch', true)) return;
  const count = Math.min(setting('prefetch_count', 50), 25);
  if (count) void bus.emit('v1/listed', { account, folder: folder.toLowerCase(), ids: list.slice(0, count).map(item => item.id) }).catch(() => {});
}

async function load(searchOnly = false) {
  if (labeling || !account) return;
  const mine = ++generation;
  const requested = { folder, query: query.trim(), account };
  const current = () => mine === generation && account === requested.account;
  error = '';
  if (searchOnly) searching = true; else { loading = items.length === 0; fetching = true; }
  render();
  try {
    const remote = requested.folder === 'Sent' ? 'sent' : 'inbox';
    const lower = requested.query.toLocaleLowerCase();
    // Start everything at once: the saved snapshot paints first, labels and the fresh list
    // arrive in parallel instead of one after another.
    const snapshot = requested.folder !== 'Drafts' && !requested.query ? call<{ messages: Message[]; fetched_at: number | null }>('GET', '/messages/cache', { query: { folder: remote } }) : null;
    let arrivedFirst = false;
    // Clearing a search goes back to the saved folder; it does not fetch it again.
    const fresh = (searchOnly && !requested.query ? null : requested.folder !== 'Drafts'
      ? call<Message[]>('POST', '/messages/refresh', { query: { ...(remote === 'sent' ? { folder: 'sent' } : {}), ...(requested.query ? { q: requested.query } : {}) }, body: {} }).then(toItems)
      : call<Draft[]>('GET', '/drafts').then(drafts => drafts.filter(d => [d.recipient, d.subject, d.body].some(text => text.toLocaleLowerCase().includes(lower))).slice().reverse()
        .map(d => ({ id: d.id, name: d.recipient || 'No recipient', subject: d.subject, preview: d.body, date: 'Draft' }))))
      ?.then(list => { arrivedFirst = true; return list; });
    const nextLabels = searchOnly ? Promise.resolve(labels) : call<Labels>('GET', '/labels');
    fresh?.catch(() => {}); nextLabels.catch(() => {});
    if (snapshot) {
      const saved = await snapshot;
      if (!current()) return;
      // A fresh list that already arrived wins over the older snapshot.
      if (!arrivedFirst) { show(toItems(saved.messages), saved.fetched_at); loading = false; warm(items); render(); }
      if (!fresh) { appliedQuery = ''; return; }
    }
    const [labelResult, result] = await Promise.all([nextLabels, fresh!]);
    if (!current()) return;
    labels = labelResult; appliedQuery = requested.query;
    if (requested.query) items = result;
    else show(result, requested.folder === 'Drafts' ? null : Math.floor(app.now().getTime() / 1000));
    warm(result);
  } catch (e) { if (mine === generation) error = (e as Error).message; }
  finally { if (mine === generation) { loading = false; fetching = false; searching = false; render(); } }
}

/** The Host already refreshed after new mail arrived, so its saved snapshot is current. */
async function syncFromCache() {
  if (folder === 'Drafts' || appliedQuery || query.trim() || fetching || searching) return;
  const mine = generation; const requested = folder;
  try {
    const snapshot = await call<{ messages: Message[]; fetched_at: number | null }>('GET', '/messages/cache', { query: { folder: requested === 'Sent' ? 'sent' : 'inbox' } });
    if (mine !== generation || requested !== folder) return;
    show(toItems(snapshot.messages), snapshot.fetched_at); warm(items); render();
  } catch { /* The next refresh shows it. */ }
}

function navigate(next: Folder) {
  query = ''; labelFilter = ''; searchEpoch++;
  if (next !== folder) { const saved = cache.get(key(next)); items = saved?.items ?? []; fetchedAt = saved?.fetchedAt ?? null; appliedQuery = ''; arrived = []; }
  folder = next; app.send({ t: 'show', surface: 'pane' }); void load();
}

function switchAccount(next: string) {
  ++generation; account = next; labels = {}; labelFilter = ''; labelError = ''; selected = null;
  query = ''; appliedQuery = ''; searchEpoch++; folder = 'Inbox'; arrived = [];
  const saved = cache.get(key('Inbox')); items = saved?.items ?? []; fetchedAt = saved?.fetchedAt ?? null;
  void load();
}

async function label(name: string, remove = false) {
  if (labeling || loading) return;
  labeling = true; labelError = ''; render();
  try {
    labels = await call<Labels>('POST', '/labels', { body: { name, message_id: null, remove } });
    if (!remove) labelEpoch++;
    if (!labels[labelFilter]) labelFilter = '';
    void bus.emit('v1/labels', { account }).catch(() => {});
  } catch (e) { labelError = (e as Error).message; }
  finally { labeling = false; render(); }
}

function open(id: string) {
  selected = id;
  if (folder === 'Drafts') void bus.request('composer', 'v1/open', { draft_id: id });
  else {
    const item = items.find(entry => entry.id === id);
    void bus.emit('v1/selected', { account, message_id: id, folder, subject: item?.subject || '(No subject)' }).catch(() => {});
  }
  render();
}

// ---- Drawing ----

const every = (seconds: number) => seconds < 60 ? `${seconds} s` : seconds % 60 ? `${Math.round(seconds / 60)} min` : `${seconds / 60} min`;
function status() {
  const state = sync[account];
  const fetched = fetchedAt ? new Date(fetchedAt * 1000).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }) : '';
  const mode = !state ? '' : ({ local: 'Sample mail', jmap_push: 'JMAP push', imap_idle: 'IMAP IDLE', offline: 'Offline', polling: `${state.protocol.toUpperCase()} · checks every ${every(state.interval)}` } as Record<string, string>)[state.mode];
  const count = state?.progress && state.progress[1] > 0 ? ` ${Math.min(state.progress[0], state.progress[1])}/${state.progress[1]}` : '';
  const busy = fetching || searching || state?.activity === 'syncing';
  const value = searching ? { text: 'Searching…', detail: '' }
    : busy ? { text: `Syncing${count || '…'}`, detail: '' }
    : state?.activity === 'prefetching' ? { text: `Downloading${count || '…'}`, detail: mode }
    : state?.mode === 'offline' ? { text: fetched ? `Offline · updated ${fetched}` : 'Offline', detail: state.error ?? '' }
    : { text: fetched ? `Updated ${fetched}` : 'Saved mail may be out of date.', detail: mode };
  return { ...value, busy, offline: !busy && state?.mode === 'offline' };
}

function rows(list: Item[]): Row[] {
  const now = app.now();
  const grouped = setting('date_groups', true) && (sort === 'newest' || sort === 'oldest');
  const out: Row[] = [];
  let group = '';
  for (const item of list) {
    const date = folder === 'Drafts' ? { label: '', group: '', full: '' } : when(item.date, now);
    if (grouped && date.group && date.group !== group) out.push({ key: `day:${date.group}`, kind: 'heading', node: { type: 'heading', level: 3, style: 'day', text: date.group } });
    group = date.group;
    // A row shows one line of each field, while bodies run to 100 KB and the renderer rejects
    // text over 16 KiB, stopping the whole extension (#87). Clip everything that comes from mail.
    out.push({ key: item.id, kind: 'row', node: { type: 'message_row', sender: clip(item.name, 512), date: clip(date.label, 128), date_title: clip(date.full, 512), subject: clip(item.subject, 1024), preview: clip(item.preview, 512) } });
  }
  return out;
}

function render() {
  const list = visible();
  const filtered = !!appliedQuery || !!query.trim() || !!labelFilter;
  const state = status();
  const off = !app.accounts.length;
  const names = Object.keys(labels);
  const empty: Node[] = loading && !list.length ? [{ type: 'empty_state', preset: 'list', text: `Loading your ${folder.toLowerCase()}…` }]
    : searching && !list.length ? [{ type: 'empty_state', preset: 'list', text: 'Searching…' }]
    : !list.length && !error && !fetching ? [{ type: 'empty_state', preset: 'list', title: filtered ? 'No matching messages' : folder === 'Drafts' ? 'No drafts yet' : 'Nothing here yet',
      hint: [{ text: filtered ? 'Try another search or clear the label filter.' : folder === 'Drafts' ? 'Start one with Compose and save it when you’re ready.' : folder === 'Sent' ? 'Messages you send will appear here.' : 'New mail will appear here when it arrives.' }] }]
    : [];
  app.render('pane', { type: 'stack', preset: 'pane', children: [
    { type: 'stack', preset: 'pane_header', children: [
      { type: 'form', id: `search-${searchEpoch}`, role: 'search', children: [
        { type: 'input', id: 'mail-search', kind: 'search', label: `Search ${folder}`, label_hidden: true, placeholder: `Search ${folder.toLowerCase()}`, value: query, maxlength: 1024, enterkeyhint: 'search', autocapitalize: 'none', report: 'input', debounce_ms: 0 },
      ] },
      { type: 'action_ref', action: 'composer/compose' },
    ] },
    { type: 'nav', id: 'folders', label: 'Mail folders', placement: 'bottom', items: folders.map(item => ({ id: item.name, label: item.name, icon: item.icon, current: folder === item.name, count: folder === item.name && !loading ? items.length : 0 })) },
    ...(off ? [{ type: 'text', preset: 'note', text: 'All your accounts are turned off. Turn one on in Settings → Accounts.' } as Node] : [
      ...(sort !== 'newest' ? [{ type: 'stack', preset: 'note', children: [{ type: 'icon', name: 'sort', size: 14 }, { type: 'text', text: `Sorted ${sorts[sort].toLocaleLowerCase()}` }, { type: 'button', id: 'clear-sort', variant: 'plain_remove', icon: 'close', label: 'Clear sorting' }] } as Node] : []),
      ...(folder !== 'Drafts' ? [{ type: 'group', preset: 'actions', label: 'Filter by label', children: [
        ...(names.length ? [{ type: 'button', id: 'label-all', variant: 'chip', label: 'All', pressed: !labelFilter }] : []),
        // Label names can hold any character, so nodes are named by position.
        ...names.map((name, index) => ({ type: 'stack', preset: 'chip_group', children: [
          { type: 'button', id: `label.${index}`, variant: 'chip', label: name, pressed: labelFilter === name },
          ...(labelFilter === name ? [{ type: 'button', id: `delete-label.${index}`, variant: 'chip_remove', icon: 'close', label: `Delete label ${name}`, disabled: labeling || loading }] : []),
        ] })),
        { type: 'form', id: `new-label-form-${labelEpoch}`, preset: 'chip_input', children: [
          { type: 'input', id: 'new-label', preset: 'chip', label: 'New label', placeholder: 'New label', maxlength: 80, report: 'input', debounce_ms: 0 },
          { type: 'button', id: 'create-label', variant: 'icon', icon: 'plus', label: 'Create label', submit: true, disabled: !newLabel.trim() || labeling || loading },
        ] },
      ] } as Node] : []),
      ...(labelError ? [{ type: 'text', preset: 'error', text: labelError } as Node] : []),
      { type: 'section', preset: 'mailbox', density: setting('density', 'full'), label: `${folder} messages`, busy: loading, children: [
        { type: 'heading', level: 1, style: 'folder', text: folder },
        ...(error ? [{ type: 'empty_state', preset: 'error', text: error, ...(items.length ? { hint: [{ text: 'Showing saved mail; it may be out of date.' }] } : {}), actions: [{ type: 'button', id: 'retry', variant: 'secondary', label: 'Try again' }] } as Node] : []),
        { type: 'list', id: 'messages', mode: 'virtual', label: `${folder} messages`, rows: rows(list), ...(selected ? { selected } : {}), arrived, children: empty },
        { type: 'stack', preset: 'footer', children: folder === 'Drafts' ? [{ type: 'text', text: 'Drafts are saved by your mail service.' }] : [
          { type: 'text', preset: 'list_status', tone: state.offline ? 'danger' : 'default', title: [state.text, state.detail].filter(Boolean).join(' · '), spans: [{ text: state.text }, ...(state.detail ? [{ text: state.detail }] : [])] },
          { type: 'button', id: 'refresh', variant: 'icon', icon: 'refresh', label: 'Refresh', title: 'Refresh', busy: state.busy, disabled: fetching },
        ] },
      ] },
    ]),
  ] });
  commands();
}

function commands() {
  app.send({ t: 'commands', items: [
    { id: 'search', name: 'Search emails' },
    ...(Object.keys(sorts) as Sort[]).filter(next => next !== sort).map(next => ({ id: `sort:${next}`, name: `Sort: ${sorts[next]}` })),
    ...(labelFilter || sort !== 'newest' || appliedQuery ? [{ id: 'clear-filters', name: 'Clear search, sorting and filters' }] : []),
    { id: 'new-label', name: 'Create a local label' },
    { id: 'refresh', name: 'Refresh current mailbox' },
    ...Object.keys(labels).map(name => ({ id: `label:${name}`, name: clip(`Filter label: ${name}`, 96) })),
  ].slice(0, 64) });
}

// ---- Interaction ----

app.when('mail-search', 'input', ({ value }) => {
  query = value; render();
  // Search follows typing after a short pause, and never refreshes the whole mailbox.
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => void runSearch(), 300);
});
async function runSearch() {
  clearTimeout(searchTimer);
  if (labeling) return;
  if (query.trim() === appliedQuery && !error) return;
  await load(true);
}
app.on('event', message => {
  const node = String(message.node);
  if (message.kind === 'submit' && node.startsWith('search-')) void runSearch();
  if (message.kind === 'submit' && node.startsWith('new-label-form-')) void label(String((message.value as { values: Record<string, string> }).values['new-label'] ?? '').trim());
  const named = (prefix: string) => node.startsWith(prefix) ? Object.keys(labels)[Number(node.slice(prefix.length))] : undefined;
  if (message.kind === 'press' && node === 'label-all') { labelFilter = ''; render(); }
  const chip = named('label.');
  if (message.kind === 'press' && chip !== undefined) { labelFilter = labelFilter === chip ? '' : chip; render(); }
  const doomed = named('delete-label.');
  if (message.kind === 'press' && doomed !== undefined) void confirmDelete(doomed);
});
app.when('new-label', 'input', ({ value }) => { const before = !newLabel.trim(); newLabel = value; if (before !== !newLabel.trim()) render(); });
app.when('folders', 'select', ({ item }) => navigate(item as Folder));
app.when('messages', 'select', ({ key }) => open(key));
app.when('refresh', 'press', () => void load());
app.when('retry', 'press', () => void load());
app.when('clear-sort', 'press', () => { sort = 'newest'; render(); });

// Deleting a label asks first, through a dialog the shell draws with this extension's name.
let deleting = '';
async function confirmDelete(name: string) {
  deleting = name;
  app.send({ t: 'dialog', tree: { type: 'stack', gap: 12, children: [
    { type: 'heading', level: 2, text: `Delete label “${name}”?` },
    { type: 'text', tone: 'muted', text: 'Messages will be kept.' },
    { type: 'stack', direction: 'row', gap: 8, justify: 'end', children: [{ type: 'button', id: 'cancel-delete', variant: 'secondary', label: 'Cancel' }, { type: 'button', id: 'confirm-delete', variant: 'primary', label: 'Delete label' }] },
  ] } });
}
app.when('cancel-delete', 'press', () => app.send({ t: 'dialog', tree: null }), 'dialog');
app.when('confirm-delete', 'press', () => { app.send({ t: 'dialog', tree: null }); void label(deleting, true); }, 'dialog');

app.on('command', message => {
  const id = String(message.id);
  if (id === 'search') { app.send({ t: 'show', surface: 'pane' }); app.send({ t: 'focus', surface: 'pane', node: 'mail-search' }); }
  else if (id.startsWith('sort:')) { sort = id.slice(5) as Sort; app.send({ t: 'show', surface: 'pane' }); render(); }
  else if (id === 'clear-filters') { sort = 'newest'; labelFilter = ''; if (appliedQuery) { query = ''; searchEpoch++; void runSearch(); } render(); }
  else if (id === 'new-label') { if (folder === 'Drafts') navigate('Inbox'); app.send({ t: 'show', surface: 'pane' }); app.send({ t: 'focus', surface: 'pane', node: 'new-label' }); }
  else if (id === 'refresh') void load();
  else if (id.startsWith('label:')) { if (folder !== 'Inbox') navigate('Inbox'); labelFilter = id.slice(6); render(); }
  else if (id.startsWith('open:')) open(id.slice(5));
});

// The command runner asks this extension for mail matching what the owner types.
app.send({ t: 'command_search', scope: 'mail' });
app.on('command_query', message => {
  const words = String(message.query).toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const hits = words.length ? items.filter(item => words.every(word => `${item.name} ${item.subject} ${item.preview}`.toLocaleLowerCase().includes(word))).slice(0, 6)
    .map(item => ({ id: `open:${item.id}`, title: item.subject || '(No subject)', hint: item.name })) : [];
  app.send({ t: 'command_hits', seq: message.seq, hits });
});
app.on('command_submit', message => { query = String(message.query); searchEpoch++; if (folder === 'Drafts') navigate('Inbox'); void load(true); });

app.on('start', () => {
  account = app.account?.id ?? 'demo';
  void call<AccountSync[]>('GET', '/sync').then(list => { sync = Object.fromEntries(list.map(status => [status.account, status])); render(); }).catch(() => {});
  void load();
});
app.on('account', () => { const next = app.account?.id ?? 'demo'; if (next !== account) switchAccount(next); else render(); });
app.on('settings', () => render());
app.on('navigate', message => { const target = String(message.target); navigate((target[0].toUpperCase() + target.slice(1)) as Folder); });
app.on('host:navigate', message => {
  const target = (message.data as { target?: string }).target;
  if (target === 'inbox' || target === 'drafts' || target === 'sent') navigate((target[0].toUpperCase() + target.slice(1)) as Folder);
});
app.on('host:mail', message => { if ((message.data as { account?: string }).account === account && folder === 'Inbox') void syncFromCache(); });
app.on('host:sync', message => {
  const event = message.data as Partial<AccountSync>;
  if (typeof event.account !== 'string' || typeof event.mode !== 'string') return;
  const previous = sync[event.account];
  sync[event.account] = event as AccountSync;
  // Background downloads just finished: the reader can warm its cache from what the Host holds.
  if (event.account === account && previous && previous.activity !== 'idle' && event.activity === 'idle') warm(items);
  render();
});
for (const name of ['host:ready', 'host:resync']) app.on(name, () => void call<AccountSync[]>('GET', '/sync').then(list => { sync = Object.fromEntries(list.map(status => [status.account, status])); render(); }).catch(() => {}));
// Other extensions open a message here: notifications, for example.
bus.handle('v1/open', payload => {
  if (typeof payload?.message_id !== 'string') return null;
  if (folder !== 'Inbox') navigate('Inbox');
  open(payload.message_id);
  return null;
});
// Keep the highlight on what the reader or composer actually shows.
bus.listen('reader/v1/viewing', payload => { if (payload?.account === account && folder !== 'Drafts') { selected = payload.message_id ?? null; render(); } });
bus.listen('composer/v1/editing', payload => { if (folder === 'Drafts') { selected = payload?.draft_id ?? null; render(); } });
bus.listen('composer/v1/saved', () => { if (folder === 'Drafts') void load(); });
bus.listen('reader/v1/labels', payload => { if (payload?.account === account) void call<Labels>('GET', '/labels').then(next => { labels = next; render(); }).catch(() => {}); });
