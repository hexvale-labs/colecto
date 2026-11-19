// Optional AI help: a menu and a saved summary inside the reader, drafted replies, and the
// settings for the services the owner chose. Automatic labels run in the Host (`ai.rs`).
import { app, bus, call, type Node } from '../../../extension-sdk';
import type { AiSettings, Effort, Field, LabelRule, Preview, SettingsView, Tone } from '../api';

let view: SettingsView | null = null;
let error = '';
let accountLabels: string[] = [];
const tested: Record<'text' | 'decision', { busy: boolean; note: string; ok: boolean }> = { text: { busy: false, note: '', ok: false }, decision: { busy: false, note: '', ok: false } };
const on = () => !!view?.settings.enabled;
const account = () => app.account?.id ?? 'demo';

// ---- Reader contributions ----

interface Summary { text: string | null; busy: boolean; error: string; loaded: boolean }
const summaries = new Map<string, Summary>();
let message: { account: string; id: string } | null = null;
const summaryKey = (owner: string, id: string) => `${owner}\n${id}`;
const summaryOf = () => (message ? summaries.get(summaryKey(message.account, message.id)) : undefined) ?? { text: null, busy: false, error: '', loaded: false };
function setSummary(change: Partial<Summary>) { if (message) summaries.set(summaryKey(message.account, message.id), { ...summaryOf(), ...change }); }

async function loadSummary() {
  if (!message || summaryOf().loaded || !on()) return;
  const id = message.id;
  try { const { summary } = await call<{ summary: string | null }>('GET', `/ai/summaries/${encodeURIComponent(id)}`); if (message?.id === id) setSummary({ text: summary, loaded: true }); }
  catch { if (message?.id === id) setSummary({ loaded: true }); }
  contribute();
}
async function summarize() {
  if (!message || summaryOf().busy) return;
  const id = message.id;
  setSummary({ busy: true, error: '' }); contribute();
  try { const { summary } = await call<{ summary: string }>('POST', `/ai/summaries/${encodeURIComponent(id)}`, { body: {} }); if (message?.id === id) setSummary({ text: summary, loaded: true }); }
  catch (e) { if (message?.id === id) setSummary({ error: (e as Error).message }); }
  finally { if (message?.id === id) setSummary({ busy: false }); contribute(); }
}
async function forget() {
  if (!message) return;
  const previous = summaryOf().text;
  setSummary({ text: null, error: '' }); contribute();
  try { await call('DELETE', `/ai/summaries/${encodeURIComponent(message.id)}`); }
  catch (e) { setSummary({ text: previous, error: (e as Error).message }); contribute(); }
}

function contribute() {
  const current = summaryOf();
  app.send({ t: 'contribute', slot: 'reader/v1/actions', tree: on() && message ? { type: 'menu_button', id: 'ai-menu', label: 'AI', icon: 'sparkles', items: [{ id: 'summarize', label: 'Summarize' }, { id: 'reply', label: 'Reply' }] } : null });
  app.send({ t: 'contribute', slot: 'reader/v1/above_body', tree: on() && message && (current.text !== null || current.busy || current.error) ? { type: 'section', preset: 'summary', labelled_by: 'summary-title', busy: current.busy, children: [
    { type: 'stack', direction: 'row', justify: 'between', align: 'center', children: [
      { type: 'heading', id: 'summary-title', level: 3, text: 'Summary' },
      { type: 'group', label: 'Summary actions', children: [
        { type: 'button', id: 'regenerate', variant: 'text', label: current.busy ? 'Summarizing…' : 'Regenerate', disabled: current.busy },
        { type: 'button', id: 'forget', variant: 'text', label: 'Delete', disabled: current.busy },
      ] },
    ] },
    ...(current.text !== null ? [{ type: 'text', id: 'summary-text', wrap: 'pre', tone: current.busy ? 'muted' : 'default', text: current.text } as Node]
      : current.busy ? [{ type: 'text', role: 'status', hidden_visually: true, text: 'Summarizing…' } as Node, { type: 'skeleton', lines: 2 } as Node] : []),
    ...(current.error ? [{ type: 'text', tone: 'danger', role: 'alert', text: current.error } as Node] : []),
  ] } : null });
}

app.on('slot', slot => {
  const context = slot.context as { account?: string; message_id?: string } | null;
  if (slot.owner !== 'reader') return;
  message = context && typeof context.message_id === 'string' ? { account: context.account ?? account(), id: context.message_id } : null;
  replying = false; draft = ''; notes = '';
  contribute(); void loadSummary();
});
app.when('ai-menu', 'select', ({ item }) => { if (item === 'summarize') void summarize(); else if (item === 'reply') { replying = true; renderDialog(); } });
app.when('regenerate', 'press', () => void summarize());
app.when('forget', 'press', () => void forget());

// ---- Drafting a reply ----

const tones: { value: Tone; label: string; hint: string }[] = [
  { value: 'casual', label: 'Casual', hint: 'Relaxed and friendly.' },
  { value: 'lazy', label: 'Lazy', hint: 'all lowercase, short, the odd “lol”.' },
  { value: 'professional', label: 'Professional', hint: 'Formal and polished, no slang.' },
  { value: 'roast', label: 'Roast', hint: 'Playful comedian jabs, never cruel.' },
  { value: 'troll', label: 'Troll', hint: 'Chaotic replies for scams and ragebait.' },
];
let replying = false;
let notes = '';
let tone: Tone = 'casual';
let draft = '';
let writing = false;
let replyError = '';
let replyEpoch = 0;

function renderDialog() {
  if (!replying) { app.send({ t: 'dialog', tree: null }); return; }
  if (editing !== null) return;
  app.send({ t: 'dialog', tree: { type: 'form', id: `reply-form-${replyEpoch}`, label: 'Draft a reply', children: [
    { type: 'heading', level: 2, text: 'Draft a reply' },
    { type: 'text', tone: 'muted', text: 'Write the gist in any language or wording. The AI turns it into a reply to this message.' },
    { type: 'textarea', id: 'notes', label: 'What do you want to say?', rows: 3, maxlength: 4000, placeholder: 'e.g. thanks, but I can\'t make it on friday', value: notes, report: 'input', disabled: writing },
    { type: 'segmented', id: 'tone', label: 'Tone', value: tone, options: tones },
    ...(draft || writing ? [{ type: 'textarea', id: 'draft', label: 'Reply', rows: 8, value: draft, placeholder: writing ? 'Writing…' : '', report: 'input', disabled: writing } as Node] : []),
    ...(replyError ? [{ type: 'text', preset: 'error', text: replyError } as Node] : []),
    { type: 'stack', direction: 'row', gap: 8, justify: 'end', children: draft && !writing
      ? [{ type: 'button', id: 'generate', variant: 'secondary', submit: true, label: 'Regenerate' }, { type: 'button', id: 'use', variant: 'primary', label: 'Use as reply' }]
      : [{ type: 'button', id: 'cancel-reply', variant: 'secondary', label: 'Cancel' }, { type: 'button', id: 'generate', variant: 'primary', submit: true, label: writing ? 'Writing…' : 'Generate', disabled: writing || !notes.trim() }] },
  ] } });
}
async function generate() {
  if (writing || !notes.trim() || !message) return;
  writing = true; replyError = ''; renderDialog();
  try { draft = (await call<{ text: string }>('POST', '/ai/reply', { body: { message_id: message.id, input: notes, tone } })).text; replyEpoch++; }
  catch (e) { replyError = (e as Error).message; }
  finally { writing = false; renderDialog(); }
}
app.when('notes', 'input', ({ value }) => { const before = !notes.trim(); notes = value; if (before !== !notes.trim()) renderDialog(); }, 'dialog');
app.when('draft', 'input', ({ value }) => { draft = value; }, 'dialog');
app.when('tone', 'change', ({ value }) => { tone = value; }, 'dialog');
app.when('cancel-reply', 'press', () => { replying = false; renderDialog(); }, 'dialog');
app.when('use', 'press', () => {
  if (message) void bus.request('reader', 'v1/reply', { message_id: message.id, body: draft.trim() + '\n' });
  replying = false; renderDialog();
}, 'dialog');
app.on('event', event => {
  const node = String(event.node);
  if (event.surface === 'dialog' && event.kind === 'submit' && node.startsWith('reply-form-')) void generate();
  if (event.surface === 'dialog' && event.kind === 'close') { replying = false; editing = null; }
  if (event.surface === 'dialog' && event.kind === 'submit' && node.startsWith('rule-form-')) void saveEditor();
});

// ---- Settings ----

const efforts: { value: string; label: string }[] = [{ value: 'default', label: 'Default' }, { value: 'none', label: 'None' }, { value: 'low', label: 'Low' }, { value: 'medium', label: 'Medium' }, { value: 'high', label: 'High' }];
const empty: LabelRule = { enabled: false, model: false, description: '', rules: [] };
const rules = () => view?.settings.labels[account()] ?? {};

async function load() {
  try { view = await call<SettingsView>('GET', '/ai/settings'); error = ''; }
  catch (e) { error = (e as Error).message; }
  await loadLabels();
  renderSettings(); contribute(); void loadSummary();
}
async function loadLabels() {
  try { accountLabels = Object.keys(await call<Record<string, string[]>>('GET', '/labels')); } catch { accountLabels = []; }
}
// Settings save as they change. A failed save reloads what the Host kept.
async function save(change: (draft: AiSettings) => void) {
  if (!view) return;
  const next = structuredClone(view.settings);
  change(next);
  view.settings = next; renderSettings();
  try { view = await call<SettingsView>('POST', '/ai/settings', { body: next }); error = ''; }
  catch (e) { error = (e as Error).message; void load(); }
  renderSettings(); contribute();
}
async function test(kind: 'text' | 'decision') {
  tested[kind] = { busy: true, note: '', ok: false }; renderSettings();
  try { await call('POST', '/ai/test', { body: { kind } }); tested[kind] = { busy: false, note: 'Connected.', ok: true }; }
  catch (e) { tested[kind] = { busy: false, note: (e as Error).message, ok: false }; }
  renderSettings();
}
function describe(rule: LabelRule | undefined) {
  if (!rule?.enabled) return 'Off';
  const parts = [];
  if (rule.rules.length) parts.push(`${rule.rules.length} ${rule.rules.length === 1 ? 'rule' : 'rules'}`);
  if (rule.model) parts.push('model on');
  return parts.join(' · ') || 'Nothing set up yet';
}

function renderSettings() {
  const settings = view?.settings;
  const testRow = (kind: 'text' | 'decision'): Node => ({ type: 'stack', direction: 'row', gap: 12, align: 'center', children: [
    { type: 'button', id: `test-${kind}`, variant: 'secondary', label: tested[kind].busy ? 'Testing…' : 'Test connection', disabled: tested[kind].busy },
    ...(tested[kind].note ? [{ type: 'text', role: 'status', tone: tested[kind].ok ? 'muted' : 'danger', size: 'sm', text: tested[kind].note } as Node] : []),
  ] });
  app.render('settings', { type: 'stack', gap: 24, children: [
    { type: 'text', tone: 'muted', text: 'Optional help from AI services you choose. While this is off, Colecto never contacts them. When it is on, summaries and replies send the message you are reading, and automatic labels send newly arrived mail, to the service you set below.' },
    ...(error ? [{ type: 'text', preset: 'error', text: error } as Node] : []),
    ...(!settings ? (error ? [] : [{ type: 'text', preset: 'note', role: 'status', text: 'Loading…' } as Node]) : [
      { type: 'fieldset', children: [{ type: 'toggle', id: 'enabled', label: 'Use AI features', description: 'Turns on summaries, drafted replies and automatic labels. Off by default.', checked: settings.enabled }] },
      { type: 'fieldset', legend: 'Text model', disabled: !settings.enabled, children: [
        { type: 'text', tone: 'muted', size: 'sm', text: 'Writes summaries and replies. Works with any service that offers an OpenAI-compatible chat completions API, including ones on your own computer.' },
        { type: 'grid', columns: 'form', children: [
          { type: 'input', id: 'text-url', kind: 'url', label: 'Base URL', placeholder: 'https://api.openai.com/v1', value: settings.text.base_url },
          { type: 'input', id: 'text-model', label: 'Model', placeholder: 'Model name', autocomplete: 'off', spellcheck: false, value: settings.text.model },
          { type: 'input', id: 'text-tokens', kind: 'number', label: 'Max tokens', min: 16, max: 32000, step: 1, value: String(settings.text.max_tokens) },
        ] },
        { type: 'secret_field', id: 'text-key', label: 'API key', target: 'ai.text', saved: view!.keys.text },
        { type: 'segmented', id: 'effort', label: 'Thinking effort', value: settings.text.effort ?? 'default', options: efforts },
        { type: 'text', tone: 'muted', size: 'sm', text: 'Only sent when not Default. Not every model supports it.' },
        testRow('text'),
      ] },
      { type: 'fieldset', legend: 'Decision model', disabled: !settings.enabled, children: [
        { type: 'text', tone: 'muted', size: 'sm', text: 'Answers yes-or-no questions for automatic labels, such as “does this belong under Receipts?”. Uses the OpenRouter Decisions API.' },
        { type: 'grid', columns: 'form', children: [
          { type: 'input', id: 'decision-url', kind: 'url', label: 'Base URL', value: settings.decision.base_url },
          { type: 'input', id: 'decision-model', label: 'Model', autocomplete: 'off', spellcheck: false, value: settings.decision.model },
        ] },
        { type: 'secret_field', id: 'decision-key', label: 'API key', target: 'ai.decision', saved: view!.keys.decision },
        { type: 'range', id: 'threshold', label: 'Confidence needed', min: 0.5, max: 0.99, step: 0.01, value: settings.decision.threshold, format: 'percent', low: 'More labels', high: 'Fewer, surer labels' },
        testRow('decision'),
      ] },
      { type: 'fieldset', legend: 'Automatic labels', disabled: !settings.enabled, children: [
        { type: 'toggle', id: 'autolabel', label: 'Label new mail automatically', description: 'Sorts newly arrived mail into your labels. Rules match text and need no AI service; a label can also ask the decision model. A message is only checked once, so a label you remove stays removed.', checked: settings.autolabel },
        { type: 'heading', level: 4, style: 'group', text: `Labels in ${app.account?.name || app.account?.address || 'Demo account'}` },
        ...(!accountLabels.length ? [{ type: 'text', tone: 'muted', size: 'sm', text: 'No labels yet. Create one from a message or with “Create a local label” in the command palette.' } as Node] : []),
        { type: 'list', id: 'labels', label: 'Labels', rows: accountLabels.map((name, index) => ({ key: name, kind: 'row' as const, node: { type: 'stack', direction: 'row', justify: 'between', align: 'center', gap: 12, children: [
          { type: 'button', id: `edit.${index}`, variant: 'text', label: name, detail: describe(rules()[name]), disabled: !settings.autolabel },
          { type: 'toggle', id: `rule.${index}`, label: `Label ${name} automatically`, label_hidden: true, checked: !!rules()[name]?.enabled, disabled: !settings.autolabel },
        ] } })) },
      ] },
    ]),
  ] });
}

const saveText = (apply: (draft: AiSettings, value: string) => void) => ({ value }: { value: string }) => void save(draft => apply(draft, value.trim()));
app.when('enabled', 'toggle', ({ checked }) => void save(draft => { draft.enabled = checked; }));
app.when('text-url', 'change', saveText((draft, value) => { draft.text.base_url = value; }));
app.when('text-model', 'change', saveText((draft, value) => { draft.text.model = value; }));
app.when('text-tokens', 'change', saveText((draft, value) => { draft.text.max_tokens = Math.round(Number(value)); }));
app.when('decision-url', 'change', saveText((draft, value) => { draft.decision.base_url = value; }));
app.when('decision-model', 'change', saveText((draft, value) => { draft.decision.model = value; }));
app.when('effort', 'change', ({ value }) => void save(draft => { draft.text.effort = value === 'default' ? null : value as Effort; }));
app.when('threshold', 'change', ({ value }) => void save(draft => { draft.decision.threshold = value; }));
app.when('autolabel', 'toggle', ({ checked }) => void save(draft => { draft.autolabel = checked; }));
app.when('test-text', 'press', () => void test('text'));
app.when('test-decision', 'press', () => void test('decision'));
app.when('text-key', 'saved', ({ saved }) => { if (view) { view.keys.text = saved; renderSettings(); } });
app.when('decision-key', 'saved', ({ saved }) => { if (view) { view.keys.decision = saved; renderSettings(); } });
app.on('event', event => {
  const node = String(event.node);
  if (event.surface !== 'settings') return;
  const labelAt = (prefix: string) => node.startsWith(prefix) ? accountLabels[Number(node.slice(prefix.length))] : undefined;
  if (event.kind === 'toggle' && labelAt('rule.') !== undefined) { const name = labelAt('rule.')!; const checked = (event.value as { checked: boolean }).checked; void save(draft => { (draft.labels[account()] ??= {})[name] = { ...(rules()[name] ?? empty), enabled: checked }; }); }
  if (event.kind === 'press' && labelAt('edit.') !== undefined) openEditor(labelAt('edit.')!);
});

// ---- Label rules ----

const fields: { value: Field; label: string }[] = [{ value: 'any', label: 'Anywhere' }, { value: 'sender', label: 'From' }, { value: 'subject', label: 'Subject' }, { value: 'body', label: 'Body' }];
let editing: string | null = null;
let rule: LabelRule = empty;
let preview: Preview | null = null;
let previewError = '';
let editorError = '';
let editorEpoch = 0;
let previewTimer: ReturnType<typeof setTimeout> | undefined;

function openEditor(name: string) { editing = name; rule = structuredClone(rules()[name] ?? { ...empty, enabled: true }); preview = null; previewError = ''; editorError = ''; editorEpoch++; renderEditor(); schedulePreview(); }
function schedulePreview() {
  clearTimeout(previewTimer);
  if (!rule.rules.length) { preview = null; renderEditor(); return; }
  previewTimer = setTimeout(async () => {
    try { preview = await call<Preview>('POST', '/ai/labels/preview', { body: { rule } }); previewError = ''; }
    catch (e) { previewError = (e as Error).message; }
    renderEditor();
  }, 250);
}
function renderEditor() {
  if (editing === null) return;
  const invalid = !!preview?.errors.some(Boolean);
  app.send({ t: 'dialog', tree: { type: 'form', id: `rule-form-${editorEpoch}`, label: `“${editing}”`, children: [
    { type: 'heading', level: 2, text: `“${editing}”` },
    { type: 'text', tone: 'muted', text: 'Choose which new mail gets this label.' },
    { type: 'toggle', id: 'rule-enabled', label: 'Label new mail automatically', checked: rule.enabled },
    { type: 'fieldset', legend: 'Rules', children: [
      { type: 'text', tone: 'muted', size: 'sm', spans: [{ text: 'A message gets the label when any rule matches. Words work as they are, ignoring case: ' }, { text: 'invoice', style: 'code' }, { text: ' matches “Invoice due”. Use ' }, { text: '|', style: 'code' }, { text: ' for “or”, like ' }, { text: 'receipt|invoice', style: 'code' }, { text: '.' }] },
      ...rule.rules.flatMap((current, index): Node[] => [
        { type: 'stack', direction: 'row', gap: 8, align: 'center', children: [
          { type: 'select', id: `field:${index}`, label: `Rule ${index + 1}: where to look`, value: current.field, options: fields },
          { type: 'input', id: `pattern:${index}`, label: `Rule ${index + 1}: text to find`, label_hidden: true, placeholder: 'Text to find', autocomplete: 'off', spellcheck: false, maxlength: 300, value: current.pattern, report: 'input', debounce_ms: 0, ...(preview?.errors[index] ? { error: preview.errors[index]! } : {}) },
          { type: 'button', id: `remove:${index}`, variant: 'icon', icon: 'trash', label: `Remove rule ${index + 1}` },
        ] },
      ]),
      ...(rule.rules.length < 10 ? [{ type: 'button', id: 'add-rule', variant: 'text', icon: 'plus', label: 'Add a rule' } as Node] : []),
    ] },
    { type: 'fieldset', legend: 'Decision model', children: [
      { type: 'toggle', id: 'rule-model', style: 'checkbox', label: 'Also ask the decision model about mail no rule matched', checked: rule.model },
      ...(rule.model && !view?.settings.decision.model ? [{ type: 'text', tone: 'muted', size: 'sm', text: 'Set up the decision model on the AI page first.' } as Node] : []),
      { type: 'textarea', id: 'rule-description', label: 'What belongs under this label? Optional, helps the model', rows: 2, maxlength: 500, placeholder: 'e.g. receipts and order confirmations from shops', value: rule.description, report: 'input', disabled: !rule.model },
    ] },
    { type: 'stack', gap: 6, children: [
      { type: 'heading', level: 3, text: 'Test with your Inbox' },
      { type: 'text', tone: previewError ? 'danger' : 'muted', size: 'sm', live: 'polite', text: previewError || (!preview ? 'Add a rule to see which saved messages it matches. The decision model is not asked here.' : invalid ? 'Fix the rule marked above to test it.'
        : preview.checked ? `Matches ${preview.count} of the last ${preview.checked} ${preview.checked === 1 ? 'message' : 'messages'}.` : 'Your Inbox has no saved messages to test with yet.') },
      ...(preview && !invalid && preview.matches.length ? [{ type: 'list', label: 'Matching messages', rows: preview.matches.map((match, index) => ({ key: String(index), kind: 'row' as const, node: { type: 'text', size: 'sm', spans: [{ text: `${match.sender}  `, style: 'muted' }, { text: match.subject || '(No subject)' }] } })) } as Node] : []),
    ] },
    ...(editorError ? [{ type: 'text', preset: 'error', text: editorError } as Node] : []),
    { type: 'stack', direction: 'row', gap: 8, justify: 'end', children: [{ type: 'button', id: 'cancel-rule', variant: 'secondary', label: 'Cancel' }, { type: 'button', id: 'save-rule', variant: 'primary', submit: true, label: 'Save', disabled: invalid }] },
  ] } });
}
async function saveEditor() {
  if (editing === null || preview?.errors.some(Boolean)) return;
  const name = editing;
  const next = { ...rule, rules: rule.rules.filter(r => r.pattern.trim()) };
  try { await save(draft => { (draft.labels[account()] ??= {})[name] = next; }); editing = null; app.send({ t: 'dialog', tree: null }); }
  catch (e) { editorError = (e as Error).message; renderEditor(); }
}
const edit = (change: () => void, redraw = true) => { change(); if (redraw) { editorEpoch++; renderEditor(); } schedulePreview(); };
app.when('rule-enabled', 'toggle', ({ checked }) => edit(() => { rule.enabled = checked; }, false), 'dialog');
// Redrawn in place (same form id), so the checkbox keeps focus while the description follows it.
app.when('rule-model', 'toggle', ({ checked }) => { edit(() => { rule.model = checked; }, false); renderEditor(); }, 'dialog');
app.when('rule-description', 'input', ({ value }) => { rule.description = value; }, 'dialog');
app.when('add-rule', 'press', () => edit(() => { rule.rules.push({ field: 'any', pattern: '' }); }), 'dialog');
app.when('cancel-rule', 'press', () => { editing = null; app.send({ t: 'dialog', tree: null }); }, 'dialog');
app.on('event', event => {
  if (event.surface !== 'dialog' || editing === null) return;
  const [kind, index] = String(event.node).split(':');
  const at = Number(index);
  if (kind === 'field' && event.kind === 'change') edit(() => { rule.rules[at].field = (event.value as { value: Field }).value; }, false);
  if (kind === 'pattern' && event.kind === 'input') edit(() => { rule.rules[at].pattern = (event.value as { value: string }).value; }, false);
  if (kind === 'remove' && event.kind === 'press') edit(() => { rule.rules.splice(at, 1); });
});

app.on('start', () => void load());
app.on('account', () => void load());
// Labels made after Settings → AI loaded would otherwise be missing from it (#101).
for (const source of ['inbox/v1/labels', 'reader/v1/labels']) {
  bus.listen(source, payload => { if (payload?.account === account()) void loadLabels().then(renderSettings); });
}
