// The component vocabulary and message checks for extension client parts
// (docs/extensions/ui-model.md, "Component vocabulary" and "Protocol"). Everything an
// extension sends is checked here before the renderer sees it; anything unknown or over a
// limit rejects the whole message, and the runtime stops the extension.
import type { IconName } from '../Icon.svelte';

export interface Node { type: string; id?: string; children?: Node[]; [prop: string]: unknown }
export interface Row { key: string; kind: 'row' | 'heading'; node: Node }

export const LIMITS = {
  message: 4 * 1024 * 1024,
  buffers: 32 * 1024 * 1024,
  nodes: 5000,
  depth: 32,
  rows: 100_000,
  rowNodes: 32,
  ops: 2000,
  text: 16 * 1024,
  label: 128,
  calls: 64,
  frame: 8 * 1024 * 1024,
} as const;

export class ProtocolError extends Error {}
const fail = (why: string): never => { throw new ProtocolError(why); };

const bytes = (text: string) => new TextEncoder().encode(text).length;
type Check = (value: unknown) => boolean;
const str = (max: number): Check => value => typeof value === 'string' && bytes(value) <= max;
const text = str(LIMITS.text);
const label = str(LIMITS.label);
const bool: Check = value => typeof value === 'boolean';
const oneOf = (...values: unknown[]): Check => value => values.includes(value);
const int = (min: number, max: number): Check => value => Number.isInteger(value) && (value as number) >= min && (value as number) <= max;
const num: Check = value => typeof value === 'number' && Number.isFinite(value);
const nodeId: Check = value => typeof value === 'string' && /^[A-Za-z0-9_.:-]{1,64}$/.test(value);
const list = (check: Check, max = 256): Check => value => Array.isArray(value) && value.length <= max && value.every(check);
const shape = (props: Record<string, Check>, required: string[] = []): Check => value =>
  !!value && typeof value === 'object' && !Array.isArray(value)
  && Object.entries(value).every(([key, item]) => key in props && props[key](item))
  && required.every(key => key in (value as object));

// The closed icon set of API 1; `crates/colecto-package/src/manifest.rs` lists the same names.
export const ICONS = ['account', 'alert', 'archive', 'attach', 'back', 'bell', 'check', 'close', 'compose', 'copy', 'cpu',
  'download', 'drafts', 'extension', 'eye', 'filter', 'folder', 'forward', 'image', 'inbox', 'info', 'key', 'layout', 'mail', 'next',
  'palette', 'pencil', 'plus', 'refresh', 'reply', 'replyAll', 'search', 'sent', 'server', 'settings', 'shield',
  'sidebar', 'sort', 'sparkles', 'star', 'tag', 'text', 'trash'] as const satisfies readonly IconName[];
const icon = oneOf(...ICONS);
const tone = oneOf('default', 'muted', 'danger', 'ok');
const span = shape({ text, style: oneOf('plain', 'strong', 'code', 'kbd', 'muted') }, ['text']);

interface Spec { props: Record<string, Check>; container?: boolean; required?: string[]; nodes?: string[] }
const control = { disabled: bool };
const item = (extra: Record<string, Check> = {}) => shape({ id: label, label, icon, title: label, disabled: bool, ...extra }, ['id', 'label']);
const density = oneOf('full', 'medium', 'compact');
export const VOCABULARY: Record<string, Spec> = {
  stack: { container: true, props: { direction: oneOf('column', 'row'), gap: oneOf(0, 2, 4, 6, 8, 12, 16, 24), align: oneOf('start', 'center', 'end', 'stretch', 'baseline'), justify: oneOf('start', 'center', 'end', 'between'), wrap: bool, scroll: oneOf('none', 'y', 'x'),
    // Layouts the app already uses, so extension panes look like the rest of Colecto.
    preset: oneOf('pane', 'pane_header', 'chip_bar', 'chip_group', 'note', 'footer', 'body_controls', 'compose_footer', 'welcome', 'actions', 'attachments_block') } },
  section: { container: true, props: { label, labelled_by: nodeId, busy: bool, preset: oneOf('mailbox', 'reader', 'composer', 'summary'), density } },
  article: { container: true, props: { label } },
  group: { container: true, props: { label, role: oneOf('group', 'toolbar'), preset: oneOf('respond', 'actions') } },
  toolbar: { container: true, props: {} },
  form: { container: true, props: { role: oneOf('search'), label, preset: oneOf('chip_input', 'compose') } },
  fieldset: { container: true, props: { legend: label, legend_hidden: bool, disabled: bool, preset: oneOf('chips', 'compose_fields') } },
  grid: { container: true, props: { columns: oneOf('form') } },
  nav: { props: { label, items: list(item({ current: bool, count: int(0, 1_000_000) }), 32), placement: oneOf('inline', 'bottom') }, required: ['label', 'items'] },
  // Mail text (subjects, senders, file names) can be long; it gets the text limit, not the label one.
  tabs: { props: { label, items: list(shape({ id: label, label: text, title: text, active: bool, closable: bool, disabled: bool }, ['id', 'label']), 64), close_all: bool }, required: ['label', 'items'] },
  heading: { props: { level: int(1, 4), text, style: oneOf('title', 'folder', 'day', 'group', 'toolbar') }, required: ['text'] },
  text: { props: { text, spans: list(span, 64), tone, size: oneOf('sm', 'md'), role: oneOf('status', 'alert'), live: oneOf('polite'), title: str(1024), wrap: oneOf('normal', 'pre'), hidden_visually: bool, preset: oneOf('crumb', 'list_status', 'body', 'note', 'save_state', 'error', 'from', 'body_note'), pending: bool } },
  badge: { props: { text: label, tone }, required: ['text'] },
  banner: { props: { tone: oneOf('info', 'warning', 'error'), text }, required: ['text'], nodes: ['actions'] },
  avatar: { props: { name: label, seed: label }, required: ['name'] },
  icon: { props: { name: icon, size: int(10, 32) }, required: ['name'] },
  message_header: { props: { subject: text, sender: text, address: text, date: label, date_title: text }, required: ['subject'] },
  attachments: { props: { label, items: list(shape({ index: int(0, 10_000), name: text, detail: label, removable: bool }, ['index', 'name']), 100), disabled: bool }, required: ['label', 'items'] },
  button: { props: { ...control, label, icon, icon_only: bool, variant: oneOf('primary', 'secondary', 'tool', 'text', 'icon', 'chip', 'chip_remove', 'plain_remove', 'compose', 'compose_icon'), detail: label, pressed: bool, busy: bool, title: label, submit: bool, copy: str(LIMITS.text * 64), push: bool, live: bool }, required: ['label'] },
  menu_button: { props: { ...control, label, icon, variant: oneOf('tool'), items: list(shape({ id: label, label, disabled: bool }, ['id', 'label']), 16) }, required: ['label', 'items'] },
  action_ref: { props: { action: str(130), labelled: bool }, required: ['action'] },
  input: { props: { ...control, kind: oneOf('text', 'search', 'email', 'url', 'number'), label, label_hidden: bool, placeholder: label, value: text, maxlength: int(1, 100_000), min: num, max: num, step: num, autocomplete: oneOf('off', 'email'), inputmode: oneOf('text', 'email', 'search', 'numeric', 'url'), enterkeyhint: oneOf('enter', 'done', 'go', 'next', 'search', 'send'), spellcheck: bool, autocapitalize: oneOf('none', 'sentences'), report: oneOf('change', 'input'), debounce_ms: int(0, 1000), error: label, preset: oneOf('row', 'chip') }, required: ['label'] },
  textarea: { props: { ...control, label, label_hidden: bool, placeholder: label, value: str(1024 * 1024), maxlength: int(1, 1024 * 1024), rows: int(1, 40), report: oneOf('change', 'input'), debounce_ms: int(0, 1000), preset: oneOf('body') }, required: ['label'] },
  toggle: { props: { ...control, label, label_hidden: bool, checked: bool, style: oneOf('switch', 'checkbox', 'chip'), description: text }, required: ['label'] },
  select: { props: { ...control, label, value: label, options: list(shape({ value: label, label, hint: label }, ['value', 'label']), 64) }, required: ['label', 'options'] },
  segmented: { props: { ...control, label, value: label, options: list(shape({ value: label, label, hint: label }, ['value', 'label']), 8) }, required: ['label', 'options'] },
  range: { props: { ...control, label, value: num, min: num, max: num, step: num, unit: str(16), low: label, high: label, format: oneOf('number', 'percent') }, required: ['label', 'min', 'max'] },
  file_picker: { props: { ...control, label, icon, multiple: bool } , required: ['label'] },
  secret_field: { props: { label, target: oneOf('ai.text', 'ai.decision'), saved: bool }, required: ['label', 'target'] },
  // Children are what a virtual list shows while it has no rows (loading, empty, searching).
  list: { container: true, props: { label, rows: () => true, selected: label, selection: oneOf('none', 'single'), mode: oneOf('static', 'virtual'), arrived: list(label, 64), context_menu: bool }, required: ['rows'] },
  message_row: { props: { sender: text, date: label, date_title: text, subject: text, preview: text, unread: bool, starred: bool, disabled: bool } },
  empty_state: { props: { title: label, text, hint: list(span, 16), image: oneOf('colecto'), preset: oneOf('list', 'welcome', 'error') }, nodes: ['actions'] },
  loading: { props: { text: label } },
  skeleton: { props: { lines: int(1, 6) } },
  frame: { props: { title: label, doc: str(LIMITS.frame), height: (value: unknown) => value === 'fill' || int(1, 10_000)(value), designed: bool, remote_images: bool }, required: ['title', 'doc'] },
  slot: { props: { name: (value: unknown) => typeof value === 'string' && /^v[1-9][0-9]*\/[a-z0-9_-]{1,48}$/.test(value), context: (value: unknown) => JSON.stringify(value ?? null).length <= 4096 }, required: ['name'] },
};

/** Check a tree, count its nodes and ids, and return it unchanged. */
export function checkTree(tree: unknown, budget = { nodes: 0, ids: new Set<string>() }, depth = 0): Node {
  if (depth > LIMITS.depth) fail('Tree too deep');
  if (!tree || typeof tree !== 'object' || Array.isArray(tree)) fail('A node must be an object');
  const node = tree as Node;
  const spec = VOCABULARY[node.type] ?? fail(`Unknown component ${String(node.type)}`);
  if (++budget.nodes > LIMITS.nodes) fail('Too many nodes');
  if (node.id !== undefined) {
    if (!nodeId(node.id)) fail('Invalid node id');
    if (budget.ids.has(node.id)) fail(`Duplicate node id ${node.id}`);
    budget.ids.add(node.id);
  }
  for (const [key, value] of Object.entries(node)) {
    if (key === 'type' || key === 'id') continue;
    if (key === 'children') {
      if (!spec.container || !Array.isArray(value) || value.length > LIMITS.nodes) fail(`${node.type} has no children`);
      for (const child of value as unknown[]) checkTree(child, budget, depth + 1);
    } else if (spec.nodes?.includes(key)) {
      if (!Array.isArray(value) || value.length > 4) fail(`Invalid ${key}`);
      for (const child of value as unknown[]) {
        if ((child as Node)?.type !== 'button') fail(`${key} holds buttons only`);
        checkTree(child, budget, depth + 1);
      }
    } else if (key === 'rows' && node.type === 'list') {
      checkRows(value, budget, depth);
    } else if (!(key in spec.props) || !spec.props[key](value)) {
      fail(`Invalid ${node.type}.${key}`);
    }
  }
  for (const key of spec.required ?? []) if (!(key in node)) fail(`${node.type} needs ${key}`);
  return node;
}

export function checkRows(rows: unknown, budget = { nodes: 0, ids: new Set<string>() }, depth = 0): Row[] {
  if (!Array.isArray(rows) || rows.length > LIMITS.rows) fail('Invalid rows');
  const keys = new Set<string>();
  for (const row of rows as Row[]) {
    if (!shape({ key: label, kind: oneOf('row', 'heading'), node: () => true }, ['key', 'kind', 'node'])(row)) fail('Invalid row');
    if (keys.has(row.key)) fail('Duplicate row key');
    keys.add(row.key);
    // Rows are budgeted on their own: up to 32 nodes each, and not part of the surface count.
    const own = { nodes: 0, ids: budget.ids };
    checkTree(row.node, own, depth + 1);
    if (own.nodes > LIMITS.rowNodes) fail('Row too large');
  }
  return rows as Row[];
}

export interface Patch { op: 'set' | 'replace' | 'insert' | 'remove'; node?: string; parent?: string; index?: number; props?: Record<string, unknown>; tree?: unknown }

/** Messages a client part may send (docs/extensions/ui-model.md, "Extension to shell"). */
export type Outgoing =
  | { t: 'render'; surface: string; tree: Node | null }
  | { t: 'patch'; surface: string; ops: Patch[] }
  | { t: 'call'; id: number; method: string; path: string; query?: Record<string, string>; body?: unknown; bytes?: boolean }
  | { t: 'pong'; n: number }
  | { t: 'toast'; id: string; title: string; body?: string; icon?: IconName; duration?: number; system?: boolean; actions?: ToastAction[] }
  | { t: 'toast_dismiss'; id: string }
  | { t: 'state'; surface: string; dirty?: boolean; busy?: boolean; back?: boolean }
  | { t: 'show'; surface: string }
  | { t: 'leave'; surface: string }
  | { t: 'focus'; surface: string; node: string }
  | { t: 'dialog'; tree: Node | null }
  | { t: 'menu'; surface: string; node: string; key: string; items: MenuItem[] }
  | { t: 'contribute'; slot: string; tree: Node | null }
  | { t: 'commands'; items: { id: string; name: string; disabled?: boolean }[] }
  | { t: 'command_search'; scope: string }
  | { t: 'command_hits'; seq: number; hits: { id: string; title: string; hint?: string }[] }
  | { t: 'app_actions'; items: { id: string; label: string; icon?: IconName; shortcut?: string; disabled?: boolean }[] }
  | { t: 'download'; name: string; data: { $buffer: number } }
  | { t: 'frame'; bitmap: { $buffer: number }; x?: number; y?: number };

/** One entry of a row's context menu (API 2); `separator` draws a line before it. */
export interface MenuItem { id: string; label: string; icon?: IconName; danger?: boolean; disabled?: boolean; checked?: boolean; separator?: boolean }
const menuItem = shape({ id: label, label, icon, danger: bool, disabled: bool, checked: bool, separator: bool }, ['id', 'label']);

export interface ToastAction { id: string; label: string; primary?: boolean; navigate?: string; account?: string; reject_send?: string; review_send?: string; url?: string }
const toastAction = shape({ id: label, label, primary: bool, navigate: str(64), account: str(128), reject_send: str(128), review_send: str(128), url: (value: unknown) => typeof value === 'string' && value.length <= 2048 && /^https:\/\//.test(value) }, ['id', 'label']);

export function checkOutgoing(message: unknown, surfaces: readonly string[]): Outgoing {
  const m = message as Record<string, unknown>;
  if (!m || typeof m !== 'object') fail('A message must be an object');
  switch (m.t) {
    case 'render':
      if (!surfaces.includes(m.surface as string)) fail('Unknown surface');
      if (m.tree !== null) checkTree(m.tree);
      return m as Outgoing;
    case 'patch':
      if (!surfaces.includes(m.surface as string) || !Array.isArray(m.ops) || m.ops.length > LIMITS.ops) fail('Invalid patch');
      return m as Outgoing;
    case 'call':
      if (!Number.isInteger(m.id) || !['GET', 'POST', 'PUT', 'DELETE'].includes(m.method as string)
        || typeof m.path !== 'string' || !m.path.startsWith('/') || m.path.includes('..') || m.path.length > 2048
        || (m.query !== undefined && !shape(Object.fromEntries(Object.keys(m.query as object).map(key => [key, str(4096)])))(m.query))) fail('Invalid call');
      return m as Outgoing;
    case 'pong':
      if (!Number.isInteger(m.n)) fail('Invalid pong');
      return m as Outgoing;
    case 'toast':
      if (!shape({ t: () => true, id: label, title: label, body: str(512), icon, duration: int(0, 60_000), system: bool, actions: list(toastAction, 3) }, ['id', 'title'])(m)) fail('Invalid toast');
      return m as Outgoing;
    case 'toast_dismiss':
      if (!label(m.id)) fail('Invalid toast');
      return m as Outgoing;
    case 'state':
      if (!shape({ t: () => true, surface: label, dirty: bool, busy: bool, back: bool }, ['surface'])(m)) fail('Invalid state');
      return m as Outgoing;
    case 'show': case 'leave':
      if (!surfaces.includes(m.surface as string)) fail('Unknown surface');
      return m as Outgoing;
    case 'focus':
      if (!shape({ t: () => true, surface: label, node: nodeId }, ['surface', 'node'])(m)) fail('Invalid focus');
      return m as Outgoing;
    case 'dialog':
      if (m.tree !== null) checkTree(m.tree);
      return m as Outgoing;
    case 'menu':
      if (!shape({ t: () => true, surface: label, node: nodeId, key: label, items: list(menuItem, 24) }, ['surface', 'node', 'key', 'items'])(m) || !surfaces.includes(m.surface as string)) fail('Invalid menu');
      return m as Outgoing;
    case 'contribute':
      if (typeof m.slot !== 'string' || !/^[a-z0-9-]+\/v[1-9][0-9]*\/[a-z0-9_-]{1,48}$/.test(m.slot)) fail('Invalid slot');
      if (m.tree !== null) checkTree(m.tree);
      return m as Outgoing;
    case 'commands':
      if (!list(shape({ id: label, name: str(96), disabled: bool }, ['id', 'name']), 64)(m.items)) fail('Invalid commands');
      return m as Outgoing;
    case 'command_search':
      if (!label(m.scope)) fail('Invalid command search');
      return m as Outgoing;
    case 'command_hits':
      if (!Number.isInteger(m.seq) || !list(shape({ id: label, title: label, hint: label }, ['id', 'title']), 6)(m.hits)) fail('Invalid command hits');
      return m as Outgoing;
    case 'app_actions':
      if (!list(shape({ id: label, label, icon, shortcut: (v: unknown) => typeof v === 'string' && /^(ctrl\+)?[a-z]$/.test(v) && v !== 'ctrl+k', disabled: bool }, ['id', 'label']), 2)(m.items)) fail('Invalid app actions');
      return m as Outgoing;
    case 'frame':
      if (!shape({ t: () => true, bitmap: shape({ $buffer: int(0, 0) }, ['$buffer']), x: num, y: num }, ['bitmap'])(m)) fail('Invalid frame');
      return m as Outgoing;
    case 'download':
      if (!label(m.name) || !shape({ $buffer: int(0, 64) }, ['$buffer'])(m.data)) fail('Invalid download');
      return m as Outgoing;
    default:
      return fail(`Unknown message ${String(m.t)}`);
  }
}
