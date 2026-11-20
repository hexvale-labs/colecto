// The mailbox: the message list of the selected account with search, sorting, labels and
// folders. Opening a message tells the reader over the bus; drafts open in the composer.
import { app, bus, call, clip, type Node, type Row } from '../../../extension-sdk';
import { when } from '../../../lib/when';
import type { AccountSync, Draft, Labels, Message } from '../../../lib/api';

/** The three folders the mailbox always shows, or `folder:<id>` for another provider folder. */
type Folder = 'Inbox' | 'Drafts' | 'Sent' | `folder:${string}`;
interface RemoteFolder { id: string; name: string; role: string }
interface Item { id: string; name: string; address?: string; subject: string; preview: string; date: string; seen?: boolean; flagged?: boolean }
/** An entry another extension added to every message's menu (`v1/add_row_action`). */
interface RowAction { extension: string; id: string; label: string; icon?: string }
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
const toItems = (messages: Message[]) => messages.map(m => ({ id: m.id, name: m.sender, address: m.address, subject: m.subject, preview: m.body, date: m.date, seen: m.seen, flagged: m.flagged }));
let rowActions: RowAction[] = [];
let folderList: RemoteFolder[] = [];
let foldersLoading: Promise<void> | null = null;
/** Older mail below the newest page, loaded as the list is scrolled to its end (#136). */
let older: Item[] = [];
let olderOf = '';
/** Whether this listing asked for older mail yet: a list shorter than a page just ends. */
let paged = false;
let more: 'available' | 'loading' | 'end' | 'error' = 'end';
const PAGE = 100;
// ---- Provider folders (#132) ----
/** Provider folders besides Inbox, Drafts and Sent, as the folder bar and message menu show them.
 *  Ids beyond the renderer's 128-character limit could not be named, so they are left out. */
const otherFolders = () => folderList.filter(entry => !['inbox', 'sent', 'drafts'].includes(entry.role) && entry.id.length <= 100);
/** Gmail files its own folders under `[Gmail]/`; the prefix says nothing here. */
const folderLabel = (entry: RemoteFolder) => entry.name.replace(/^\[(Gmail|Google Mail)\]\//, '') || entry.name;
const folderIcon = (role: string) => ({ archive: 'archive', trash: 'trash', junk: 'alert' } as Record<string, string>)[role] ?? 'folder';
/** The folder's name on screen. */
function nameOf(name: Folder) {
  if (!name.startsWith('folder:')) return name;
  const entry = folderList.find(item => item.id === name.slice(7));
  return entry ? folderLabel(entry) : name.slice(7);
}
/** The `folder` the Host lists: `inbox`, `sent` or a provider folder id; `null` for Drafts. */
const remoteOf = (name: Folder) => name === 'Inbox' ? 'inbox' : name === 'Sent' ? 'sent' : name === 'Drafts' ? null : name.slice(7);
/** Folders load in the background: listing them can take seconds over IMAP, and nothing,
 *  least of all a message's menu, waits for them (#133). */
function loadFolders() {
  const owner = account;
  foldersLoading ??= call<RemoteFolder[]>('GET', '/folders').then(list => {
    if (owner !== account) return;
    folderList = list;
    render();
  }).catch(() => {}).finally(() => { if (owner === account) foldersLoading = null; });
  return foldersLoading;
}
const setting = <T,>(name: string, fallback: T) => (app.settings[name] ?? fallback) as T;

/** The list after the label filter, the search being typed, and sorting. */
// ---- Search filters (#103) ----
// The Host turns these into the provider's own search (colecto-core `search.rs`); here they
// only narrow the list while a search is on its way, and show as chips.
interface Filter { token: string; label: string; test?: (item: Item) => boolean }
const lower = (text = '') => text.toLocaleLowerCase();
function parse(text: string): { words: string[]; filters: Filter[] } {
  const words: string[] = []; const filters: Filter[] = [];
  for (const token of text.match(/(?:[^\s"]+|"[^"]*")+/g) ?? []) {
    const [key, ...rest] = token.split(':');
    const value = rest.join(':').replace(/^"|"$/g, '');
    const date = /^\d{4}-\d{2}-\d{2}$/.test(value);
    const kind = lower(key);
    if ((kind === 'from' || kind === 'to') && value) filters.push({ token, label: `${kind === 'from' ? 'From' : 'To'} ${value}`, test: kind === 'from' ? item => lower(`${item.name} ${item.address}`).includes(lower(value)) : undefined });
    else if (kind === 'is' && ['starred', 'flagged'].includes(lower(value))) filters.push({ token, label: 'Starred', test: item => !!item.flagged });
    else if (kind === 'is' && lower(value) === 'unstarred') filters.push({ token, label: 'Not starred', test: item => !item.flagged });
    else if (kind === 'is' && lower(value) === 'unread') filters.push({ token, label: 'Unread', test: item => !item.seen });
    else if (kind === 'is' && lower(value) === 'read') filters.push({ token, label: 'Read', test: item => !!item.seen });
    else if ((kind === 'after' || kind === 'before') && date) filters.push({ token, label: `${kind === 'after' ? 'Since' : 'Before'} ${value}` });
    else words.push(lower(token));
  }
  return { words, filters };
}
/** Whether an item could match: dates and recipients are left to the server. */
const matches = (item: Item, { words, filters }: ReturnType<typeof parse>) =>
  words.every(word => lower(`${item.name} ${item.address} ${item.subject} ${item.preview}`).includes(word)) && filters.every(filter => !filter.test || filter.test(item));
const day = (offset: number) => new Date(app.now().getTime() + offset * 86_400_000).toISOString().slice(0, 10);
const filterChoices: { id: string; label: string; token: () => string; complete: boolean }[] = [
  { id: 'from', label: 'From a sender…', token: () => 'from:', complete: false },
  { id: 'to', label: 'To a recipient…', token: () => 'to:', complete: false },
  { id: 'starred', label: 'Starred', token: () => 'is:starred', complete: true },
  { id: 'unread', label: 'Unread', token: () => 'is:unread', complete: true },
  { id: 'read', label: 'Read', token: () => 'is:read', complete: true },
  { id: 'after', label: 'In the last 30 days', token: () => `after:${day(-30)}`, complete: true },
  { id: 'before', label: 'Older than 30 days', token: () => `before:${day(-30)}`, complete: true },
];

function visible() {
  // While a search is on its way, narrow the current list at once so typing feels live.
  const typed = query.trim();
  const pending = typed && lower(typed) !== lower(appliedQuery) ? parse(typed) : { words: [], filters: [] };
  const filtered = items.filter(item => (!labelFilter || labels[labelFilter]?.includes(item.id)) && matches(item, pending));
  if (sort === 'newest') return filtered;
  if (sort === 'oldest') return filtered.slice().reverse();
  const by = (item: Item) => (sort === 'sender' ? item.name : item.subject).toLocaleLowerCase();
  return filtered.slice().sort((a, b) => by(a).localeCompare(by(b)));
}

function show(list: Item[], at: number | null) {
  // Messages that arrive with a refresh slide in, a few at a time; a first load shows all at once.
  const before = new Set(items.map(item => item.id));
  arrived = before.size && !query.trim() && !labelFilter ? list.filter(item => !before.has(item.id)).slice(0, 8).map(item => item.id) : [];
  items = withOlder(list); fetchedAt = at;
  cache.set(key(folder), { items: list, fetchedAt: at });
}

/** The newest page followed by the older pages already loaded for this listing. */
function withOlder(list: Item[]) {
  const listed = new Set(list.map(item => item.id));
  return [...list, ...older.filter(item => !listed.has(item.id))];
}
/** Whether older mail may exist after a newest page of `count` messages. */
const moreAfter = (count: number) => older.length ? more : count >= PAGE ? 'available' : 'end';

/** Tell the reader which messages to keep ready, as the owner chose in Settings → Mail. */
function warm(list: Item[]) {
  if (folder === 'Drafts' || !setting('prefetch', true)) return;
  const count = Math.min(setting('prefetch_count', 50), 25);
  if (count) void bus.emit('v1/listed', { account, folder: remoteOf(folder), ids: list.slice(0, count).map(item => item.id) }).catch(() => {});
}

async function load(searchOnly = false) {
  if (labeling || !account) return;
  const mine = ++generation;
  const requested = { folder, query: query.trim(), account };
  const current = () => mine === generation && account === requested.account;
  error = '';
  // Older pages belong to one folder and search; anything else starts from the newest page.
  const listing = `${requested.folder}\u0000${requested.query}`;
  if (listing !== olderOf) { older = []; olderOf = listing; more = 'end'; paged = false; }
  if (searchOnly) searching = true; else { loading = items.length === 0; fetching = true; }
  render();
  try {
    const remote = remoteOf(requested.folder);
    const lower = requested.query.toLocaleLowerCase();
    // Start everything at once: the saved snapshot paints first, labels and the fresh list
    // arrive in parallel instead of one after another. Only Inbox and Sent keep one.
    const snapshot = (remote === 'inbox' || remote === 'sent') && !requested.query ? call<{ messages: Message[]; fetched_at: number | null }>('GET', '/messages/cache', { query: { folder: remote } }) : null;
    let arrivedFirst = false;
    // Clearing a search goes back to the saved folder; it does not fetch it again.
    const fresh = (searchOnly && !requested.query ? null : remote
      ? call<Message[]>('POST', '/messages/refresh', { query: { ...(remote !== 'inbox' ? { folder: remote } : {}), ...(requested.query ? { q: requested.query } : {}) }, body: {} }).then(toItems)
      : call<Draft[]>('GET', '/drafts').then(drafts => drafts.filter(d => [d.recipient, d.subject, d.body].some(text => text.toLocaleLowerCase().includes(lower))).slice().reverse()
        .map(d => ({ id: d.id, name: d.recipient || 'No recipient', subject: d.subject, preview: d.body, date: 'Draft' }))))
      ?.then(list => { arrivedFirst = true; return list; });
    const nextLabels = searchOnly ? Promise.resolve(labels) : call<Labels>('GET', '/labels');
    fresh?.catch(() => {}); nextLabels.catch(() => {});
    if (snapshot) {
      const saved = await snapshot;
      if (!current()) return;
      // A fresh list that already arrived wins over the older snapshot.
      if (!arrivedFirst) { show(toItems(saved.messages), saved.fetched_at); more = moreAfter(saved.messages.length); loading = false; warm(items); render(); }
      if (!fresh) { appliedQuery = ''; return; }
    }
    const [labelResult, result] = await Promise.all([nextLabels, fresh!]);
    if (!current()) return;
    labels = labelResult; appliedQuery = requested.query;
    if (requested.query) items = withOlder(result);
    else show(result, requested.folder === 'Drafts' ? null : Math.floor(app.now().getTime() / 1000));
    more = requested.folder === 'Drafts' ? 'end' : moreAfter(result.length);
    warm(result);
  } catch (e) { if (mine === generation) error = (e as Error).message; }
  finally { if (mine === generation) { loading = false; fetching = false; searching = false; render(); } }
}

/** Load the next older page when the list is scrolled to its end (#136). Pages overlap when
 *  mail arrives in between, so messages already listed are dropped. */
async function loadMore() {
  const remote = remoteOf(folder);
  if (more !== 'available' && more !== 'error' || !remote || labeling || loading || !items.length) return;
  const mine = generation; const requested = { folder, query: appliedQuery };
  more = 'loading'; paged = true; render();
  try {
    const page = toItems(await call<Message[]>('POST', '/messages/refresh', { query: { ...(remote !== 'inbox' ? { folder: remote } : {}), ...(requested.query ? { q: requested.query } : {}), offset: String(items.length) }, body: {} }));
    if (mine !== generation || requested.folder !== folder || requested.query !== appliedQuery) return;
    const listed = new Set(items.map(item => item.id));
    const fresh = page.filter(item => !listed.has(item.id));
    older = [...older, ...fresh];
    items = [...items, ...fresh];
    if (!requested.query) cache.set(key(folder), { items, fetchedAt });
    more = page.length < PAGE || !fresh.length ? 'end' : 'available';
  } catch { if (mine === generation) more = 'error'; }
  render();
}

/** The Host already refreshed after new mail arrived, so its saved snapshot is current. */
async function syncFromCache() {
  const remote = remoteOf(folder);
  if ((remote !== 'inbox' && remote !== 'sent') || appliedQuery || query.trim() || fetching || searching) return;
  const mine = generation; const requested = folder;
  try {
    const snapshot = await call<{ messages: Message[]; fetched_at: number | null }>('GET', '/messages/cache', { query: { folder: remote } });
    if (mine !== generation || requested !== folder) return;
    show(toItems(snapshot.messages), snapshot.fetched_at); warm(items); render();
  } catch { /* The next refresh shows it. */ }
}

function navigate(next: Folder) {
  query = ''; labelFilter = ''; searchEpoch++;
  if (next !== folder) { const saved = cache.get(key(next)); items = saved?.items ?? []; fetchedAt = saved?.fetchedAt ?? null; appliedQuery = ''; arrived = []; older = []; olderOf = ''; paged = false; }
  folder = next; app.send({ t: 'show', surface: 'pane' }); void load();
}

function switchAccount(next: string) {
  ++generation; account = next; labels = {}; labelFilter = ''; labelError = ''; selected = null; folderList = []; foldersLoading = null;
  query = ''; appliedQuery = ''; searchEpoch++; folder = 'Inbox'; arrived = []; older = []; olderOf = ''; paged = false;
  const saved = cache.get(key('Inbox')); items = saved?.items ?? []; fetchedAt = saved?.fetchedAt ?? null;
  void load(); void loadFolders();
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
    void bus.emit('v1/selected', { account, message_id: id, folder: nameOf(folder), subject: item?.subject || '(No subject)' }).catch(() => {});
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
    out.push({ key: item.id, kind: 'row', node: { type: 'message_row', sender: clip(item.name, 512), date: clip(date.label, 128), date_title: clip(date.full, 512), subject: clip(item.subject, 1024), preview: clip(item.preview, 512), ...(item.flagged ? { starred: true } : {}) } });
  }
  return out;
}

function render() {
  const list = visible();
  const title = nameOf(folder);
  // A label filter or a search still being typed narrows the list here, so scrolling it to
  // its end would page through the whole folder for a few matches.
  const narrowed = !!labelFilter || (!!query.trim() && lower(query.trim()) !== lower(appliedQuery));
  const filtered = !!appliedQuery || !!query.trim() || !!labelFilter;
  const state = status();
  const off = !app.accounts.length;
  const names = Object.keys(labels);
  const empty: Node[] = loading && !list.length ? [{ type: 'empty_state', preset: 'list', text: folder.startsWith('folder:') ? `Loading ${title}…` : `Loading your ${title.toLowerCase()}…` }]
    : searching && !list.length ? [{ type: 'empty_state', preset: 'list', text: 'Searching…' }]
    : !list.length && !error && !fetching ? [{ type: 'empty_state', preset: 'list', title: filtered ? 'No matching messages' : folder === 'Drafts' ? 'No drafts yet' : 'Nothing here yet',
      hint: [{ text: filtered ? 'Try another search, or remove a filter.' : folder === 'Drafts' ? 'Start one with Compose and save it when you’re ready.' : folder === 'Sent' ? 'Messages you send will appear here.' : 'New mail will appear here when it arrives.' }] }]
    : [];
  app.render('pane', { type: 'stack', preset: 'pane', children: [
    { type: 'stack', preset: 'pane_header', children: [
      { type: 'form', id: `search-${searchEpoch}`, role: 'search', children: [
        { type: 'input', id: 'mail-search', kind: 'search', label: clip(`Search ${title}`, 128), label_hidden: true, placeholder: clip(folder.startsWith('folder:') ? `Search ${title}` : `Search ${title.toLowerCase()}`, 128), value: query, maxlength: 1024, enterkeyhint: 'search', autocapitalize: 'none', report: 'input', debounce_ms: 0 },
      ] },
      { type: 'action_ref', action: 'composer/compose' },
    ] },
    { type: 'nav', id: 'folders', label: 'Mail folders', placement: 'bottom', items: [
      ...folders.map(item => ({ id: item.name, label: item.name, icon: item.icon, current: folder === item.name, count: folder === item.name && !loading ? items.length : 0 })),
      ...otherFolders().slice(0, 32 - folders.length).map(entry => {
        const id: Folder = `folder:${entry.id}`;
        return { id, label: clip(folderLabel(entry), 128), icon: folderIcon(entry.role), current: folder === id, count: folder === id && !loading ? items.length : 0 };
      }),
    ] },
    ...(off ? [{ type: 'text', preset: 'note', text: 'All your accounts are turned off. Turn one on in Settings → Accounts.' } as Node] : [
      ...(sort !== 'newest' ? [{ type: 'stack', preset: 'note', children: [{ type: 'icon', name: 'sort', size: 14 }, { type: 'text', text: `Sorted ${sorts[sort].toLocaleLowerCase()}` }, { type: 'button', id: 'clear-sort', variant: 'plain_remove', icon: 'close', label: 'Clear sorting' }] } as Node] : []),
      ...(folder !== 'Drafts' ? [{ type: 'group', preset: 'actions', label: 'Filters and labels', children: [
        { type: 'menu_button', id: 'add-filter', label: 'Filter', icon: 'filter', items: filterChoices.map(choice => ({ id: choice.id, label: choice.label })) },
        ...activeFilters().map((filter, index) => ({ type: 'stack', preset: 'chip_group', children: [
          { type: 'button', id: `filter.${index}`, variant: 'chip', label: clip(filter.label, 128), pressed: true },
          { type: 'button', id: `remove-filter.${index}`, variant: 'chip_remove', icon: 'close', label: clip(`Remove filter ${filter.label}`, 128) },
        ] } as Node)),
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
      { type: 'section', preset: 'mailbox', density: setting('density', 'full'), label: clip(`${title} messages`, 128), busy: loading, children: [
        { type: 'heading', level: 1, style: 'folder', text: title },
        ...(error ? [{ type: 'empty_state', preset: 'error', text: error, ...(items.length ? { hint: [{ text: 'Showing saved mail; it may be out of date.' }] } : {}), actions: [{ type: 'button', id: 'retry', variant: 'secondary', label: 'Try again' }] } as Node] : []),
        { type: 'list', id: 'messages', mode: 'virtual', label: clip(`${title} messages`, 128), rows: rows(list), ...(selected ? { selected } : {}), arrived, context_menu: folder !== 'Drafts', ...(list.length && !narrowed && (more !== 'end' || paged) ? { more } : {}), children: empty },
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
  const removed = message.kind === 'press' && node.startsWith('remove-filter.') ? activeFilters()[Number(node.slice('remove-filter.'.length))] : undefined;
  if (removed) {
    const index = query.indexOf(removed.token);
    query = `${query.slice(0, index)}${query.slice(index + removed.token.length)}`.replace(/\s+/g, ' ').trim();
    searchEpoch++; void runSearch();
  }
  const chip = named('label.');
  if (message.kind === 'press' && chip !== undefined) { labelFilter = labelFilter === chip ? '' : chip; render(); }
  const doomed = named('delete-label.');
  if (message.kind === 'press' && doomed !== undefined) void confirmDelete(doomed);
});
app.when('new-label', 'input', ({ value }) => { const before = !newLabel.trim(); newLabel = value; if (before !== !newLabel.trim()) render(); });
app.when('folders', 'select', ({ item }) => navigate(item as Folder));
app.when('messages', 'end', () => void loadMore());
app.when('messages', 'select', ({ key }) => open(key));
app.when('add-filter', 'select', ({ item }) => {
  const choice = filterChoices.find(entry => entry.id === item);
  if (!choice) return;
  if (folder === 'Drafts') navigate('Inbox');
  query = `${query.trim()} ${choice.token()}`.trim();
  searchEpoch++; render();
  // A filter that needs a value waits for it; the rest search at once.
  if (choice.complete) void runSearch();
  else app.send({ t: 'focus', surface: 'pane', node: 'mail-search' });
});
function activeFilters() { return parse(query.trim()).filters; }
// ---- A message's context menu (#95) ----
app.when('messages', 'context', ({ key }) => showMenu(key));
app.when('messages', 'menu', ({ key, item }) => void choose(key, item));
function showMenu(id: string) {
  const item = items.find(entry => entry.id === id);
  // Always answer, even with nothing, so the shell never waits on a menu (#133).
  if (!item || folder === 'Drafts') { app.send({ t: 'menu', surface: 'pane', node: 'messages', key: id, items: [] }); return; }
  // Answer at once with the folders known so far; a first menu fetches them for the next.
  if (!folderList.length) void loadFolders();
  const here = remoteOf(folder);
  const archive = folderList.find(entry => entry.role === 'archive');
  const names = Object.keys(labels).slice(0, 8);
  const head = [
    { id: 'read', label: item.seen ? 'Mark as unread' : 'Mark as read', icon: item.seen ? 'mail' : 'eye' },
    { id: 'star', label: item.flagged ? 'Remove star' : 'Star', icon: 'star' },
    ...rowActions.map((action, index) => ({ id: `ext:${index}`, label: clip(action.label, 64), icon: action.icon, ...(index === 0 ? { separator: true } : {}) })),
    ...names.map((name, index) => ({ id: `label:${index}`, label: clip(`Label: ${name}`, 64), checked: !!labels[name]?.includes(id), ...(index === 0 ? { separator: true } : {}) })),
    ...(archive && folder !== 'Sent' && here !== archive.id ? [{ id: 'archive', label: 'Archive', icon: 'archive', separator: true }] : []),
  ];
  // The same folders the folder bar shows, as many as the menu's 24 entries allow.
  const others = otherFolders().filter(entry => entry.role !== 'archive' && entry.role !== 'trash' && entry.id !== here).slice(0, 24 - head.length - 1);
  app.send({ t: 'menu', surface: 'pane', node: 'messages', key: id, items: [
    ...head,
    ...others.map((entry, index) => ({ id: `move:${entry.id}`, label: clip(`Move to ${folderLabel(entry)}`, 64), icon: folderIcon(entry.role), ...(index === 0 && !head.some(entry => entry.id === 'archive') ? { separator: true } : {}) })),
    { id: 'delete', label: 'Move to Trash', icon: 'trash', danger: true, separator: true },
  ] });
}
async function organize(action: string, ids: string[], body: Record<string, unknown> = {}) {
  await call('POST', `/messages/${action}`, { body: { ids, ...body } });
}
async function choose(id: string, choice: string) {
  const item = items.find(entry => entry.id === id);
  if (!item) return;
  error = '';
  try {
    if (choice === 'read') { await organize('flags', [id], { seen: !item.seen }); item.seen = !item.seen; }
    else if (choice === 'star') { await organize('flags', [id], { flagged: !item.flagged }); item.flagged = !item.flagged; }
    else if (choice.startsWith('label:')) {
      const name = Object.keys(labels)[Number(choice.slice(6))];
      if (name) { labels = await call<Labels>('POST', '/labels', { body: { name, message_id: id, remove: !!labels[name]?.includes(id) } }); void bus.emit('v1/labels', { account }).catch(() => {}); }
    } else if (choice.startsWith('ext:')) {
      const action = rowActions[Number(choice.slice(4))];
      if (action) void bus.emit('v1/row_action', { extension: action.extension, action: action.id, account, message_id: id, folder: remoteOf(folder) ?? 'drafts' }).catch(() => {});
    } else if (choice === 'archive' || choice === 'delete' || choice.startsWith('move:')) {
      if (choice.startsWith('move:')) await organize('move', [id], { folder: choice.slice(5) });
      else await organize(choice, [id]);
      items = items.filter(entry => entry.id !== id);
      older = older.filter(entry => entry.id !== id);
      if (selected === id) selected = null;
      cache.set(key(folder), { items, fetchedAt });
    }
  } catch (e) { error = (e as Error).message; }
  render();
}
// Other extensions add entries to every message's menu (at most four each); choosing one
// emits `v1/row_action` for them. A second call with the same id replaces the entry.
bus.handle('v1/add_row_action', (payload, from) => {
  if (typeof payload?.id !== 'string' || typeof payload.label !== 'string' || !payload.id || !payload.label) return false;
  const action: RowAction = { extension: from, id: payload.id.slice(0, 64), label: payload.label.slice(0, 64), ...(typeof payload.icon === 'string' ? { icon: payload.icon } : {}) };
  const others = rowActions.filter(entry => !(entry.extension === from && entry.id === action.id));
  if (others.filter(entry => entry.extension === from).length >= 4) return false;
  rowActions = [...others, action];
  return true;
});
bus.handle('v1/remove_row_action', (payload, from) => {
  rowActions = rowActions.filter(entry => !(entry.extension === from && (payload?.id === undefined || entry.id === payload.id)));
  return true;
});
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
  // The command runner takes the same filters as the search field.
  const parsed = parse(String(message.query).trim());
  const hits = parsed.words.length || parsed.filters.length ? items.filter(item => matches(item, parsed)).slice(0, 6)
    .map(item => ({ id: `open:${item.id}`, title: item.subject || '(No subject)', hint: item.name })) : [];
  app.send({ t: 'command_hits', seq: message.seq, hits });
});
app.on('command_submit', message => { query = String(message.query); searchEpoch++; if (folder === 'Drafts') navigate('Inbox'); void load(true); });

app.on('start', () => {
  account = app.account?.id ?? 'demo';
  // Extensions that add message menu entries register (again) when Mail starts.
  void bus.emit('v1/ready', {}).catch(() => {});
  void call<AccountSync[]>('GET', '/sync').then(list => { sync = Object.fromEntries(list.map(status => [status.account, status])); render(); }).catch(() => {});
  void load(); void loadFolders();
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
bus.listen('ai/v1/labeled', payload => { if (payload?.account === account) void call<Labels>('GET', '/labels').then(next => { labels = next; render(); }).catch(() => {}); });
bus.listen('reader/v1/labels', payload => { if (payload?.account === account) void call<Labels>('GET', '/labels').then(next => { labels = next; render(); }).catch(() => {}); });
