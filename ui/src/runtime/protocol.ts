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
export const ICONS = ['account', 'alert', 'attach', 'back', 'bell', 'check', 'close', 'compose', 'copy', 'cpu',
  'download', 'drafts', 'extension', 'forward', 'image', 'inbox', 'info', 'key', 'layout', 'mail', 'next',
  'palette', 'pencil', 'plus', 'refresh', 'reply', 'replyAll', 'search', 'sent', 'server', 'settings', 'shield',
  'sidebar', 'sort', 'text', 'trash'] as const satisfies readonly IconName[];
const icon = oneOf(...ICONS);
const tone = oneOf('default', 'muted', 'danger', 'ok');
const span = shape({ text, style: oneOf('plain', 'strong', 'code', 'kbd', 'muted') }, ['text']);

interface Spec { props: Record<string, Check>; container?: boolean; required?: string[]; nodes?: string[] }
const control = { disabled: bool };
export const VOCABULARY: Record<string, Spec> = {
  stack: { container: true, props: { direction: oneOf('column', 'row'), gap: oneOf(0, 2, 4, 6, 8, 12, 16, 24), align: oneOf('start', 'center', 'end', 'stretch', 'baseline'), justify: oneOf('start', 'center', 'end', 'between'), wrap: bool, scroll: oneOf('none', 'y', 'x') } },
  section: { container: true, props: { label, labelled_by: nodeId, busy: bool } },
  group: { container: true, props: { label, role: oneOf('group', 'toolbar') } },
  toolbar: { container: true, props: {} },
  form: { container: true, props: { role: oneOf('search'), label } },
  heading: { props: { level: int(1, 4), text, style: oneOf('title', 'folder', 'day', 'group') }, required: ['text'] },
  text: { props: { text, spans: list(span, 64), tone, size: oneOf('sm', 'md'), role: oneOf('status', 'alert'), live: oneOf('polite'), title: label, wrap: oneOf('normal', 'pre'), hidden_visually: bool } },
  badge: { props: { text: label, tone }, required: ['text'] },
  banner: { props: { tone: oneOf('info', 'warning', 'error'), text }, required: ['text'], nodes: ['actions'] },
  icon: { props: { name: icon, size: int(10, 32) }, required: ['name'] },
  button: { props: { ...control, label, icon, icon_only: bool, variant: oneOf('primary', 'secondary', 'tool', 'text', 'icon', 'chip'), detail: label, pressed: bool, busy: bool, title: label, type: oneOf('button', 'submit') }, required: ['label'] },
  input: { props: { ...control, kind: oneOf('text', 'search', 'email', 'url', 'number'), label, label_hidden: bool, placeholder: label, value: text, maxlength: int(1, 100_000), autocomplete: oneOf('off', 'email'), inputmode: oneOf('text', 'email', 'search', 'numeric', 'url'), enterkeyhint: oneOf('enter', 'done', 'go', 'next', 'search', 'send'), spellcheck: bool, autocapitalize: oneOf('none', 'sentences'), report: oneOf('change', 'input'), debounce_ms: int(0, 1000), error: label }, required: ['label'] },
  textarea: { props: { ...control, label, label_hidden: bool, placeholder: label, value: str(1024 * 1024), maxlength: int(1, 1024 * 1024), rows: int(1, 40), report: oneOf('change', 'input'), debounce_ms: int(0, 1000) }, required: ['label'] },
  toggle: { props: { ...control, label, label_hidden: bool, checked: bool, style: oneOf('switch', 'checkbox') }, required: ['label'] },
  select: { props: { ...control, label, value: label, options: list(shape({ value: label, label, hint: label }, ['value', 'label']), 64) }, required: ['label', 'options'] },
  list: { props: { label, rows: () => true, selected: label, selection: oneOf('none', 'single') }, required: ['rows'] },
  empty_state: { props: { title: label, text, hint: list(span, 16) } },
  loading: { props: { text: label } },
  frame: { props: { title: label, doc: str(LIMITS.frame), height: (value: unknown) => value === 'fill' || int(1, 10_000)(value), designed: bool }, required: ['title', 'doc'] },
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
  | { t: 'call'; id: number; method: string; path: string; query?: Record<string, string>; body?: unknown }
  | { t: 'pong'; n: number };

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
    default:
      return fail(`Unknown message ${String(m.t)}`);
  }
}
