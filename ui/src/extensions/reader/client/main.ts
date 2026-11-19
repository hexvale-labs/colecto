// The message reader: tabs of open messages, the body in a sealed frame, labels, attachments,
// and slots other extensions fill (the AI menu and summary).
import { app, bus, call, clip, type Node } from '../../../extension-sdk';
import { when } from '../../../lib/when';
import { replyDraft, type ReplyMode } from '../../../lib/reply';
import type { AttachmentInfo, Labels, Message, MessageContent } from '../../../lib/api';

interface Tab { account: string; id: string; subject: string; folder: string }
let account = '';
let tabs: Tab[] = [];
let view: { id: string; folder: string } | null = null;
let message: Message | null = null;
let content: MessageContent | null = null;
let loading = false;
let contentLoading = false;
let error = '';
let contentError = '';
let openError = '';
let showHtml = true;
let allowRemote = false;
let banners: { text: string; tone: string }[] = [];
let changedBy = '';
let labels: Labels = {};
let labeling = false;
let labelError = '';
let downloading: number | null = null;
let generation = 0;

// Opened mail stays in memory so going back to a message, or opening one warmed ahead of time,
// never waits for the network. A message's content never changes.
const MAX_CACHED = 150;
const contents = new Map<string, Promise<MessageContent>>();
const cacheKey = (owner: string, id: string) => `${owner}\u0000${id}`;
function remember(key: string, value: Promise<MessageContent>) {
  contents.delete(key); contents.set(key, value);
  while (contents.size > MAX_CACHED) contents.delete(contents.keys().next().value!);
}
function cachedContent(id: string, onlyCached = false): Promise<MessageContent> {
  const key = cacheKey(account, id);
  const load = (only: boolean) => {
    const pending = call<MessageContent>('GET', `/messages/${encodeURIComponent(id)}/content`, only ? { query: { cached: 'true' } } : {});
    remember(key, pending);
    pending.catch(() => { if (contents.get(key) === pending) contents.delete(key); });
    return pending;
  };
  const hit = contents.get(key);
  if (!hit) return load(onlyCached);
  remember(key, hit);
  // A warm-up that found nothing cached must not fail a real open: fetch it properly then.
  return onlyCached ? hit : hit.catch(() => load(false));
}
/** Warm the cache for ids the mailbox listed, a few at a time, from what the Host already holds. */
async function prefetch(ids: string[]) {
  const queue = ids.filter(id => !contents.has(cacheKey(account, id))).slice(0, MAX_CACHED / 2);
  const worker = async () => { for (let id = queue.shift(); id !== undefined; id = queue.shift()) await cachedContent(id, true).catch(() => {}); };
  await Promise.all(Array.from({ length: Math.min(3, queue.length) }, worker));
}

const currentTabs = () => tabs.filter(tab => tab.account === account);
const remote = () => app.settings.remote_images === true || allowRemote;
// Mail that paints its own backgrounds or text colours keeps its light sheet.
const designed = () => !!content?.html && /bgcolor|background(-color)?\s*:|(^|[^-\w])color\s*[:=]/i.test(content.html);

function viewing() {
  void bus.emit('v1/viewing', { account, message_id: view?.id ?? null }).catch(() => {});
  app.send({ t: 'state', surface: 'pane', back: !!view });
}

function clear() { ++generation; view = null; message = null; content = null; error = ''; contentError = ''; banners = []; changedBy = ''; showHtml = true; allowRemote = false; }

async function load() {
  if (!view) return;
  const mine = ++generation;
  const id = view.id;
  loading = true; contentLoading = true; error = ''; message = null; content = null; render();
  // Fetch the body alongside the header instead of after it.
  const body = cachedContent(id);
  body.catch(() => {});
  try { message = await call<Message>('GET', `/messages/${encodeURIComponent(id)}`); }
  catch (e) { if (mine === generation) error = (e as Error).message; }
  finally { if (mine === generation) loading = false; }
  if (mine !== generation) return;
  render();
  if (!message) return;
  try { content = await body; contentError = ''; }
  catch (e) { contentError = (e as Error).message; }
  if (mine !== generation) return;
  contentLoading = false;
  render();
  await bus.emit('v1/message_opened', { message_id: id, subject: message.subject, from: message.address || message.sender, html: content?.html ?? null, text: content?.text || message.body }).catch(() => {});
}

function open(id: string, folder: string, subject: string) {
  openError = '';
  if (!currentTabs().some(tab => tab.id === id)) {
    if (currentTabs().length >= 30) { openError = 'You have 30 messages open in this account. Close a tab to open another.'; render(); return; }
    tabs = [...tabs, { account, id, subject, folder }];
  }
  clear(); view = { id, folder };
  app.send({ t: 'show', surface: 'pane' });
  viewing(); void load(); renderTabs();
}

function closeTab(id: string) {
  const list = currentTabs();
  const index = list.findIndex(tab => tab.id === id);
  const active = view?.id === id;
  tabs = tabs.filter(tab => tab.account !== account || tab.id !== id);
  if (active) {
    const rest = currentTabs();
    const next = rest[Math.min(index, rest.length - 1)];
    clear();
    if (next) { view = { id: next.id, folder: next.folder }; void load(); }
    viewing();
  }
  renderTabs(); render();
}
function closeAll() { tabs = tabs.filter(tab => tab.account !== account); if (view) { clear(); viewing(); } renderTabs(); render(); }
function back() { clear(); viewing(); render(); app.send({ t: 'leave', surface: 'pane' }); }

function reply(mode: ReplyMode, body = '') {
  if (!message) return;
  const draft = replyDraft(mode, message, content, app.account?.address ?? '');
  void bus.request('composer', 'v1/open', { draft: body ? { ...draft, body: body + draft.body } : draft });
}

async function assign(name: string, remove: boolean) {
  if (!view || labeling) return;
  labeling = true; labelError = ''; render();
  try {
    labels = await call<Labels>('POST', '/labels', { body: { name, message_id: view.id, remove } });
    void bus.emit('v1/labels', { account }).catch(() => {});
  } catch (e) { labelError = (e as Error).message; }
  finally { labeling = false; render(); }
}

async function save(file: AttachmentInfo) {
  if (!view) return;
  downloading = file.index; contentError = ''; render();
  try {
    const data = await call<ArrayBuffer>('GET', `/messages/${encodeURIComponent(view.id)}/attachments/${file.index}`, { bytes: true });
    app.send({ t: 'download', name: file.name, data: { $buffer: 0 } }, [data]);
  } catch (e) { contentError = (e as Error).message; }
  finally { downloading = null; render(); }
}

const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : bytes < 1_048_576 ? `${Math.round(bytes / 1024)} KB` : `${(bytes / 1_048_576).toFixed(1)} MB`;
function greeting() {
  const hour = app.now().getHours();
  return hour < 5 ? 'Up late?' : hour < 12 ? 'Good morning.' : hour < 18 ? 'Good afternoon.' : 'Good evening.';
}

// A text node holds at most 16 KiB, and a longer one stops the reader (#87). Plain bodies run
// longer, so they are split at line breaks into blocks that read as one continuous text.
function bodyText(text: string): Node {
  const max = 15 * 1024;
  const parts: string[] = [];
  let rest = text;
  while (rest.length) {
    let end = 0, bytes = 0;
    for (const char of rest) {
      const point = char.codePointAt(0)!;
      bytes += point < 0x80 ? 1 : point < 0x800 ? 2 : point < 0x10000 ? 3 : 4;
      if (bytes > max) break;
      end += char.length;
    }
    if (end === rest.length) { parts.push(rest); break; }
    const newline = rest.lastIndexOf('\n', end - 1);
    parts.push(rest.slice(0, newline > 0 ? newline : end));
    rest = rest.slice(newline > 0 ? newline + 1 : end);
  }
  return parts.length === 1 ? { type: 'text', preset: 'body', text: parts[0] }
    : { type: 'stack', gap: 0, children: parts.map(part => ({ type: 'text', preset: 'body', text: part }) as Node) };
}

function renderTabs() {
  const list = currentTabs();
  app.render('pane.header', list.length ? { type: 'tabs', id: 'tabs', label: 'Open messages', close_all: true,
    items: list.map(tab => ({ id: tab.id, label: clip(tab.subject, 512), title: clip(tab.subject, 1024), active: view?.id === tab.id })) } : null);
}

function render() {
  if (!view) {
    app.render('pane', { type: 'stack', preset: 'welcome', children: [
      { type: 'empty_state', preset: 'welcome', image: 'colecto', title: greeting(), text: openError || 'Pick a message and it will open here.',
        hint: [{ text: 'Press ' }, { text: 'Ctrl', style: 'kbd' }, { text: ' ' }, { text: 'K', style: 'kbd' }, { text: ' to jump anywhere, or drag a pane’s tab to rearrange.' }] },
    ] });
    commands();
    return;
  }
  const date = message ? when(message.date, app.now()) : null;
  const names = Object.keys(labels);
  const toolbar: Node = { type: 'toolbar', children: [
    { type: 'button', id: 'back', variant: 'icon', icon: 'back', label: 'Back to list' },
    { type: 'text', preset: 'crumb', text: view.folder },
    ...(message ? [{ type: 'group', preset: 'respond', label: 'Respond', children: [
      { type: 'button', id: 'reply', variant: 'tool', icon: 'reply', label: 'Reply', title: 'Reply' },
      { type: 'button', id: 'reply-all', variant: 'tool', icon: 'replyAll', label: 'Reply all', title: 'Reply all' },
      { type: 'button', id: 'forward', variant: 'tool', icon: 'forward', label: 'Forward', title: 'Forward' },
    ] } as Node, { type: 'slot', id: 'actions-slot', name: 'v1/actions', context: { account, message_id: view.id } } as Node,
    { type: 'button', id: 'copy', variant: 'tool', icon: 'copy', label: 'Copy message text', title: 'Copy message text', live: true, copy: clip(content?.text || message.body, 1024 * 1024) } as Node] : []),
  ] };
  const body: Node[] = !message ? [] : [
    ...banners.map(banner => ({ type: 'banner', tone: banner.tone, text: banner.text }) as Node),
    ...(changedBy ? [{ type: 'banner', tone: 'info', text: `${changedBy} changed how this message is shown.` } as Node] : []),
    { type: 'message_header', subject: clip(message.subject, 1024), sender: clip(message.sender, 512), address: clip(message.address, 512), ...(date?.full ? { date: clip(date.full, 128), date_title: clip(message.date, 512) } : {}) },
    ...(labelError ? [{ type: 'text', preset: 'error', text: labelError } as Node] : []),
    ...(names.length ? [{ type: 'fieldset', preset: 'chips', legend: 'Labels', legend_hidden: true, disabled: labeling, children: names.map((name, index) => ({ type: 'toggle', id: `label.${index}`, style: 'chip', label: `Label ${name}`, label_hidden: true, checked: labels[name].includes(view!.id) })) } as Node] : []),
    ...(content?.attachments.length ? [{ type: 'attachments', id: 'attachments', label: 'Attachments', disabled: downloading !== null, items: content.attachments.map(file => ({ index: file.index, name: file.name, detail: downloading === file.index ? 'Downloading…' : size(file.size) })) } as Node] : []),
    { type: 'slot', id: 'summary-slot', name: 'v1/above_body', context: { account, message_id: view.id } },
    ...(contentError ? [{ type: 'text', preset: 'note', role: 'status', text: `Showing saved text. ${contentError}` } as Node] : []),
    ...(content?.html ? [{ type: 'stack', preset: 'body_controls', children: [
      ...(showHtml && !remote() ? [{ type: 'text', preset: 'body_note', text: 'Remote images are blocked.' } as Node, { type: 'button', id: 'load-images', variant: 'text', label: 'Load remote images' } as Node] : []),
      { type: 'button', id: 'toggle-html', variant: 'text', icon: 'text', push: true, pressed: !showHtml, label: showHtml ? 'Show plain text' : 'Show formatted' },
    ] } as Node] : []),
    contentLoading ? { type: 'skeleton', lines: 3 }
      : content?.html && showHtml ? { type: 'frame', title: 'Message content', doc: content.html, designed: designed(), remote_images: remote() }
      : bodyText(content?.text || message.body),
  ];
  app.render('pane', { type: 'section', preset: 'reader', label: 'Message reader', busy: loading, children: [
    toolbar,
    loading ? { type: 'loading', text: 'Opening message…' }
      : error ? { type: 'empty_state', text: error, actions: [{ type: 'button', id: 'retry', variant: 'secondary', label: 'Try again' }] }
      : { type: 'article', children: body },
  ] });
  commands();
}

function commands() {
  const id = view?.id;
  app.send({ t: 'commands', items: [
    ...(id ? Object.keys(labels).map(name => ({ id: `assign:${name}`, name: clip(`${labels[name].includes(id) ? 'Remove' : 'Add'} label: ${name}`, 96), disabled: labeling })) : []),
    { id: 'back', name: 'Back to message list' },
    ...currentTabs().map(tab => ({ id: `tab:${tab.id}`, name: clip(`Open message: ${tab.subject}`, 96) })),
    ...(id ? [{ id: 'close-tab', name: 'Close current message tab' }] : []),
    ...(currentTabs().length > 1 ? [{ id: 'close-all', name: 'Close all message tabs' }] : []),
  ].slice(0, 64) });
}

app.when('back', 'press', back);
app.when('retry', 'press', () => void load());
app.when('reply', 'press', () => reply('reply'));
app.when('reply-all', 'press', () => reply('all'));
app.when('forward', 'press', () => reply('forward'));
app.when('load-images', 'press', () => { allowRemote = true; render(); });
app.when('toggle-html', 'press', () => { showHtml = !showHtml; render(); });
app.when('attachments', 'download', ({ index }) => { const file = content?.attachments.find(item => item.index === index); if (file) void save(file); });
app.when('tabs', 'select', ({ item }) => { const tab = currentTabs().find(entry => entry.id === item); if (tab) open(tab.id, tab.folder, tab.subject); });
app.when('tabs', 'close', ({ item }) => closeTab(item));
app.when('tabs', 'close_all', closeAll);
app.on('event', message => {
  const node = String(message.node);
  const name = node.startsWith('label.') ? Object.keys(labels)[Number(node.slice(6))] : undefined;
  if (message.kind === 'toggle' && name !== undefined) void assign(name, !(message.value as { checked: boolean }).checked);
});
app.on('back', back);
app.on('command', message => {
  const id = String(message.id);
  if (id.startsWith('assign:') && view) { const name = id.slice(7); void assign(name, labels[name]?.includes(view.id) ?? false); }
  else if (id === 'back') back();
  else if (id.startsWith('tab:')) { const tab = currentTabs().find(entry => entry.id === id.slice(4)); if (tab) open(tab.id, tab.folder, tab.subject); }
  else if (id === 'close-tab' && view) closeTab(view.id);
  else if (id === 'close-all') closeAll();
});

app.on('start', () => { account = app.account?.id ?? 'demo'; void refreshLabels(); render(); renderTabs(); });
app.on('account', () => {
  const next = app.account?.id ?? 'demo';
  if (next === account) return;
  account = next; clear(); labels = {}; viewing(); void refreshLabels(); render(); renderTabs();
});
app.on('settings', () => render());
async function refreshLabels() { try { labels = await call<Labels>('GET', '/labels'); render(); } catch { /* Labels stay as they were. */ } }

bus.listen('inbox/v1/selected', payload => { if (payload?.account === account && typeof payload.message_id === 'string') open(payload.message_id, payload.folder ?? 'Inbox', payload.subject ?? '(No subject)'); });
bus.listen('inbox/v1/listed', payload => { if (payload?.account === account && Array.isArray(payload.ids)) void prefetch(payload.ids.filter((id: unknown) => typeof id === 'string')); });
bus.listen('inbox/v1/labels', payload => { if (payload?.account === account) void refreshLabels(); });
bus.handle('v1/get_html', payload => payload?.message_id === view?.id ? content?.html ?? null : null);
bus.handle('v1/set_html', (payload, from) => {
  if (payload?.message_id === view?.id && content && typeof payload.html === 'string') { content = { ...content, html: payload.html }; changedBy = from; render(); }
  return null;
});
bus.handle('v1/add_banner', payload => {
  if (payload?.message_id === view?.id && typeof payload.text === 'string' && ['info', 'warning'].includes(payload.tone)) { banners = [...banners.slice(-2), { text: payload.text.slice(0, 280), tone: payload.tone }]; render(); }
  return null;
});
// Another extension (AI) starts a reply with its own text above the quoted message.
bus.handle('v1/reply', payload => {
  if (payload?.message_id !== view?.id || typeof payload.body !== 'string') return null;
  reply('reply', payload.body);
  return null;
});
