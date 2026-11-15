// The composer: one draft at a time in its own pane, with attachments, sending and buttons other
// extensions contribute. The Compose action in the top bar (and the C key) comes from here.
import { app, bus, call, type Node } from '../../../extension-sdk';
import type { Draft, DraftInput } from '../../../lib/api';

interface Fields { recipient: string; cc: string; subject: string; body: string }
interface File { name: string; content_type: string; data: string; size: number }
const MAX_FILES = 25 * 1024 * 1024;
let open = false;
let id: string | undefined;
let thread: Pick<Draft, 'in_reply_to' | 'references' | 'forward_of'> = {};
let heading = 'New message';
let fields: Fields = { recipient: '', cc: '', subject: '', body: '' };
let baseline = '';
let files: File[] = [];
let saving = false, sending = false, sent = false, saved = false;
let error = '';
let epoch = 0;
let pendingOpen: Draft | null | undefined;
let openError = '';
const snapshot = () => JSON.stringify([fields.recipient, fields.cc, fields.subject, fields.body]);
const dirty = () => open && (snapshot() !== baseline || files.length > 0);
const connected = () => app.account?.connected ?? false;

function state() { app.send({ t: 'state', surface: 'pane', dirty: dirty(), busy: saving, back: open }); }

function start(draft: Draft | null) {
  open = true; epoch++; openError = '';
  id = draft?.id || undefined;
  fields = { recipient: draft?.recipient ?? '', cc: draft?.cc ?? '', subject: draft?.subject ?? '', body: draft?.body ?? '' };
  baseline = snapshot();
  // Threading and forward references come from the original message and are not edited.
  thread = { ...(draft?.in_reply_to ? { in_reply_to: draft.in_reply_to, references: draft.references ?? '' } : {}), ...(draft?.forward_of ? { forward_of: draft.forward_of } : {}) };
  heading = id ? 'Your draft' : draft?.in_reply_to ? 'Reply' : draft?.forward_of || draft?.subject?.startsWith('Fwd:') ? 'Forward' : 'New message';
  files = []; saving = sending = sent = saved = false; error = '';
  app.send({ t: 'show', surface: 'pane' });
  app.send({ t: 'focus', surface: 'pane', node: draft?.recipient ? 'body' : 'recipient' });
  void bus.emit('v1/opened', {}).catch(() => {});
  void bus.emit('v1/editing', { draft_id: id ?? null }).catch(() => {});
  render();
}

/** Open a draft, asking first when unsaved changes would be lost. */
function request(draft: Draft | null) {
  if (!dirty()) { start(draft); return; }
  pendingOpen = draft;
  app.send({ t: 'dialog', tree: { type: 'stack', gap: 12, children: [
    { type: 'heading', level: 2, text: 'Discard unsaved changes?' },
    { type: 'text', tone: 'muted', text: 'Your last saved draft will be kept.' },
    { type: 'stack', direction: 'row', gap: 8, justify: 'end', children: [{ type: 'button', id: 'keep', variant: 'secondary', label: 'Keep editing' }, { type: 'button', id: 'discard', variant: 'primary', label: 'Discard changes' }] },
  ] } });
}
app.when('keep', 'press', () => { pendingOpen = undefined; app.send({ t: 'dialog', tree: null }); }, 'dialog');
app.when('discard', 'press', () => { app.send({ t: 'dialog', tree: null }); if (pendingOpen !== undefined) start(pendingOpen); else close(); pendingOpen = undefined; }, 'dialog');

function close() { open = false; openError = ''; files = []; error = ''; void bus.emit('v1/editing', { draft_id: null }).catch(() => {}); render(); app.send({ t: 'leave', surface: 'pane' }); }

async function save() {
  if (saving) return;
  saving = true; error = ''; saved = false; render();
  try {
    const result = await call<Draft>('POST', '/drafts', { body: { id, ...fields, ...(fields.cc ? {} : { cc: undefined }), ...thread } satisfies Partial<DraftInput> });
    id = result.id; baseline = snapshot(); saved = true; heading = 'Your draft';
    void bus.emit('v1/saved', { draft_id: id }).catch(() => {});
    void bus.emit('v1/editing', { draft_id: id }).catch(() => {});
  } catch (e) { error = (e as Error).message; }
  finally { saving = false; render(); }
}

async function send() {
  if (saving || sent) return;
  saving = true; sending = true; error = ''; render();
  try {
    await bus.emit('v1/sending', {}).catch(() => {});
    await call('POST', '/send', { body: { ...fields, ...(fields.cc ? {} : { cc: undefined }), ...thread, ...(files.length ? { attachments: files.map(({ name, content_type, data }) => ({ name, content_type, data })) } : {}) } });
    sent = true; baseline = snapshot(); files = [];
  } catch (e) { error = (e as Error).message; }
  finally { saving = false; sending = false; render(); }
}

function base64(buffer: ArrayBuffer) {
  const bytes = new Uint8Array(buffer);
  let text = '';
  for (let index = 0; index < bytes.length; index += 0x8000) text += String.fromCharCode(...bytes.subarray(index, index + 0x8000));
  return btoa(text);
}
function attach(picked: { name: string; type: string; size: number; data: ArrayBuffer; failed?: boolean }[]) {
  error = '';
  const unread = picked.find(file => file.failed);
  if (unread) { error = `Could not read ${unread.name}. Choose it again.`; render(); return; }
  if (files.length + picked.length > 20) { error = 'Attach at most 20 files.'; render(); return; }
  if ([...files, ...picked].reduce((total, file) => total + file.size, 0) > MAX_FILES) { error = 'Attachments must total at most 25 MB.'; render(); return; }
  for (const file of picked) files.push({ name: file.name, content_type: file.type || 'application/octet-stream', data: base64(file.data), size: file.size });
  render();
}

function status() {
  return sent ? 'Accepted by mail server' : sending ? 'Sending…' : saving ? 'Saving draft…' : dirty() ? 'Unsaved changes' : saved ? 'Draft saved' : id ? 'Saved by your mail service' : connected() ? 'Not saved yet' : 'Sending is unavailable for this account';
}

function render() {
  state();
  app.send({ t: 'app_actions', items: [{ id: 'compose', label: 'Compose', icon: 'compose', shortcut: 'c', disabled: saving }] });
  app.send({ t: 'commands', items: open ? [{ id: 'save', name: 'Save draft' }, ...(connected() ? [{ id: 'send', name: 'Send message' }] : [])] : [] });
  if (!open) {
    app.render('pane', openError
      ? { type: 'empty_state', preset: 'error', text: openError, actions: [{ type: 'button', id: 'back', variant: 'secondary', label: 'Back to list' }] }
      : { type: 'stack', preset: 'welcome', children: [{ type: 'empty_state', preset: 'welcome', title: 'No draft open', text: 'Press Compose or C to start a message, or open a draft from the Drafts folder.' }] });
    return;
  }
  const locked = saving || sent;
  const sender = app.account?.address || 'Demo account';
  app.render('pane', { type: 'section', preset: 'composer', label: 'Draft editor', children: [
    { type: 'form', id: `draft-${epoch}`, preset: 'compose', children: [
      { type: 'toolbar', children: [
        { type: 'button', id: 'back', variant: 'icon', icon: 'back', label: 'Back to list', disabled: saving },
        { type: 'heading', level: 2, style: 'toolbar', text: heading },
        { type: 'stack', preset: 'actions', children: [{ type: 'slot', name: 'v1/actions', context: { draft_id: id ?? null } }] },
      ] },
      { type: 'fieldset', preset: 'compose_fields', disabled: locked, children: [
        { type: 'text', preset: 'from', spans: [{ text: 'From' }, { text: sender, style: 'strong' }] },
        { type: 'input', id: 'recipient', preset: 'row', label: 'To', kind: 'email', inputmode: 'email', autocomplete: 'email', autocapitalize: 'none', spellcheck: false, enterkeyhint: 'next', placeholder: 'name@example.com', maxlength: 4000, value: fields.recipient, report: 'input' },
        { type: 'input', id: 'cc', preset: 'row', label: 'Cc', kind: 'email', inputmode: 'email', autocomplete: 'email', autocapitalize: 'none', spellcheck: false, maxlength: 4000, value: fields.cc, report: 'input' },
        { type: 'input', id: 'subject', preset: 'row', label: 'Subject', maxlength: 998, value: fields.subject, report: 'input' },
        { type: 'textarea', id: 'body', preset: 'body', label: 'Message', placeholder: 'Write your message', maxlength: 100_000, value: fields.body, report: 'input' },
        ...(connected() && (files.length || thread.forward_of) ? [{ type: 'stack', preset: 'attachments_block', children: [
          ...(thread.forward_of ? [{ type: 'text', preset: 'note', text: 'The original message’s attachments are included when you send.' } as Node] : []),
          ...(files.length ? [{ type: 'attachments', id: 'files', label: 'Files to send', items: files.map((file, index) => ({ index, name: file.name, removable: true })) } as Node,
            { type: 'text', preset: 'note', text: 'Attached files are sent but not saved with drafts.' } as Node] : []),
        ] } as Node] : []),
      ] },
      ...(error ? [{ type: 'text', preset: 'error', text: error } as Node] : []),
      { type: 'stack', preset: 'compose_footer', children: [
        ...(connected() ? [{ type: 'file_picker', id: 'attach', label: 'Attach files', icon: 'attach', multiple: true, disabled: locked } as Node] : []),
        { type: 'button', id: 'copy', variant: 'tool', icon: 'copy', label: 'Copy message text', title: 'Copy message text', live: true, copy: fields.body, disabled: !fields.body },
        { type: 'text', preset: 'save_state', pending: dirty() && !saving && !sent, text: status() },
        { type: 'button', id: 'save', variant: 'secondary', submit: true, label: saving && !sending ? 'Saving…' : 'Save draft', disabled: locked || (!!id && !dirty()) },
        ...(connected() ? [{ type: 'button', id: 'send', variant: 'primary', icon: 'sent', label: sent ? 'Sent' : sending ? 'Sending…' : 'Send', disabled: locked } as Node] : []),
      ] },
    ] },
  ] });
}

const typed = (key: keyof Fields) => (value: { value: string }) => {
  const before = dirty(); const hadBody = !!fields.body;
  fields = { ...fields, [key]: value.value }; saved = false;
  // Only redraw when something visible depends on it; the renderer keeps the text itself.
  if (before !== dirty() || (key === 'body' && hadBody !== !!fields.body)) render(); else state();
};
for (const key of ['recipient', 'cc', 'subject', 'body'] as const) app.when(key, 'input', typed(key));
app.on('event', message => { if (message.kind === 'submit' && String(message.node).startsWith('draft-')) void save(); });
app.when('send', 'press', () => void send());
app.when('back', 'press', () => { if (dirty()) { pendingOpen = undefined; request(null); } else close(); });
app.when('attach', 'files', value => attach(value));
app.when('files', 'remove', ({ index }) => { files = files.filter((_, other) => other !== index); render(); });
app.on('back', () => { if (dirty()) request(null); else close(); });
app.on('app_action', message => { if (message.id === 'compose') request(null); });
app.on('navigate', message => { if (message.target === 'compose') request(null); });
app.on('host:navigate', message => { if ((message.data as { target?: string }).target === 'compose') request(null); });
app.on('command', message => { if (message.id === 'save') void save(); else if (message.id === 'send') void send(); });
app.on('start', render);
app.on('account', render);

bus.handle('v1/open', async payload => {
  if (typeof payload?.draft_id === 'string') {
    try { request(await call<Draft>('GET', `/drafts/${encodeURIComponent(payload.draft_id)}`)); }
    catch (e) { openError = (e as Error).message; open = false; render(); app.send({ t: 'show', surface: 'pane' }); }
    return null;
  }
  if (payload?.draft && typeof payload.draft === 'object') { request(payload.draft as Draft); return null; }
  request(null);
  return null;
});
bus.handle('v1/get_draft', () => open ? { recipient: fields.recipient, subject: fields.subject, body: fields.body } : null);
bus.handle('v1/set_draft', value => {
  if (!open) return null;
  for (const key of ['recipient', 'subject', 'body'] as const) if (typeof value?.[key] === 'string') fields[key] = value[key];
  epoch++; render();
  return null;
});
