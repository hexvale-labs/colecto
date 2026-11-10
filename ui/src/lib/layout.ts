// The workspace is a tree, like a game engine's editor: splits divide space into rows or columns,
// and every leaf is a group of tabbed panes. Dropping a pane on a group's edge splits it again.
export type Edge = 'left' | 'right' | 'top' | 'bottom';
export type Zone = Edge | 'center';
export interface Tabs { kind: 'tabs'; id: string; panes: string[]; active: string }
export interface Split { kind: 'split'; dir: 'row' | 'column'; children: Node[]; sizes: number[] }
export type Node = Tabs | Split;
export interface Layout { version: 2; root: Node | null; hidden: string[] }
export interface Rect { x: number; y: number; w: number; h: number }
export interface Gutter { path: number[]; index: number; dir: 'row' | 'column'; rect: Rect; span: number }

export const GUTTER = 1;
export const HEAD = 34;
export const MIN = { row: 180, column: 76 };
const MAX_DEPTH = 8;

export function defaultLayout(): Layout {
  return {
    version: 2, hidden: [],
    root: { kind: 'split', dir: 'row', sizes: [0.27, 0.73], children: [
      { kind: 'split', dir: 'column', sizes: [0.89, 0.11], children: [tabs('t1', ['mailbox']), tabs('t2', ['accounts'])] },
      tabs('t3', ['reader']),
    ] },
  };
}
function tabs(id: string, panes: string[]): Tabs { return { kind: 'tabs', id, panes, active: panes[0] ?? '' }; }

export function leaves(node: Node | null): Tabs[] {
  return !node ? [] : node.kind === 'tabs' ? [node] : node.children.flatMap(leaves);
}
export function groupOf(layout: Layout, pane: string): Tabs | undefined {
  return leaves(layout.root).find(group => group.panes.includes(pane));
}
function nextId(root: Node | null) {
  return `t${Math.max(0, ...leaves(root).map(group => Number(group.id.slice(1)) || 0)) + 1}`;
}
// Layouts are plain JSON; this also unwraps Svelte state proxies, which structuredClone rejects.
const clone = (layout: Layout): Layout => JSON.parse(JSON.stringify(layout));

/** Remove empty groups, merge splits into same-direction parents and keep sizes summing to 1. */
function tidy(node: Node | null): Node | null {
  if (!node || node.kind === 'tabs') {
    if (node && !node.panes.includes(node.active)) node.active = node.panes[0] ?? '';
    return node?.panes.length ? node : null;
  }
  const children: Node[] = []; const sizes: number[] = [];
  node.children.forEach((child, i) => {
    const kept = tidy(child);
    if (!kept) return;
    if (kept.kind === 'split' && kept.dir === node.dir) kept.children.forEach((c, j) => { children.push(c); sizes.push(kept.sizes[j] * node.sizes[i]); });
    else { children.push(kept); sizes.push(node.sizes[i]); }
  });
  if (!children.length) return null;
  if (children.length === 1) return children[0];
  const total = sizes.reduce((a, b) => a + b, 0) || 1;
  return { ...node, children, sizes: sizes.map(size => size / total) };
}

function without(layout: Layout, pane: string): Layout {
  const next = clone(layout);
  for (const group of leaves(next.root)) group.panes = group.panes.filter(item => item !== pane);
  next.root = tidy(next.root);
  return next;
}

function replace(node: Node, id: string, make: (group: Tabs) => Node): Node {
  if (node.kind === 'tabs') return node.id === id ? make(node) : node;
  return { ...node, children: node.children.map(child => replace(child, id, make)) };
}

/** Drop `pane` on a group: in its tabs (center) or as a new group beside it (an edge). */
export function dock(layout: Layout, pane: string, target: string, zone: Zone, index?: number): Layout {
  const source = groupOf(layout, pane);
  if (zone !== 'center' && source?.id === target && source.panes.length === 1) return layout;
  const next = without(layout, pane);
  if (!next.root || !leaves(next.root).some(group => group.id === target)) return layout;
  if (zone === 'center') {
    const group = leaves(next.root).find(item => item.id === target)!;
    group.panes.splice(index === undefined ? group.panes.length : Math.max(0, Math.min(index, group.panes.length)), 0, pane);
    group.active = pane;
    return next;
  }
  const fresh = tabs(nextId(next.root), [pane]);
  const after = zone === 'right' || zone === 'bottom';
  next.root = tidy(replace(next.root, target, group => ({ kind: 'split', dir: zone === 'left' || zone === 'right' ? 'row' : 'column', sizes: [0.5, 0.5], children: after ? [group, fresh] : [fresh, group] })));
  return next;
}

/** Dock `pane` along an edge of the whole workspace. */
export function dockEdge(layout: Layout, pane: string, edge: Edge): Layout {
  const next = without(layout, pane);
  const fresh = tabs(nextId(next.root), [pane]);
  if (!next.root) { next.root = fresh; return next; }
  const after = edge === 'right' || edge === 'bottom';
  next.root = tidy({ kind: 'split', dir: edge === 'left' || edge === 'right' ? 'row' : 'column', sizes: after ? [0.75, 0.25] : [0.25, 0.75], children: after ? [next.root, fresh] : [fresh, next.root] });
  return next;
}

export function activate(layout: Layout, pane: string): Layout {
  const group = groupOf(layout, pane);
  if (!group || group.active === pane) return layout;
  const next = clone(layout);
  groupOf(next, pane)!.active = pane;
  return next;
}

export function setVisible(layout: Layout, pane: string, visible: boolean): Layout {
  const next = visible ? clone(layout) : without(layout, pane);
  next.hidden = visible ? next.hidden.filter(item => item !== pane) : [...new Set([...next.hidden, pane])];
  return next;
}

export function resize(layout: Layout, path: number[], sizes: number[]): Layout {
  const next = clone(layout);
  let node = next.root;
  for (const index of path) node = node?.kind === 'split' ? node.children[index] : null;
  if (node?.kind === 'split' && sizes.length === node.sizes.length) node.sizes = sizes;
  return next;
}

/** Keep exactly the known, visible panes. New ones join the reader's group, or the first group. */
export function reconcile(layout: Layout, panes: string[]): Layout {
  const next = clone(layout);
  next.hidden = next.hidden.filter(pane => panes.includes(pane));
  const seen = new Set<string>();
  for (const group of leaves(next.root)) group.panes = group.panes.filter(pane => panes.includes(pane) && !next.hidden.includes(pane) && !seen.has(pane) && !!seen.add(pane));
  next.root = tidy(next.root);
  const missing = panes.filter(pane => !seen.has(pane) && !next.hidden.includes(pane));
  if (!missing.length) return next;
  if (!next.root) { next.root = tabs('t1', missing); return next; }
  const home = groupOf(next, 'reader') ?? leaves(next.root)[0];
  home.panes.push(...missing);
  return next;
}

/** Lay out every group and gutter inside `area`, in pixels. */
export function measure(root: Node | null, area: Rect) {
  const groups = new Map<string, Rect>(); const gutters: Gutter[] = [];
  const walk = (node: Node, rect: Rect, path: number[]) => {
    if (node.kind === 'tabs') { groups.set(node.id, rect); return; }
    const row = node.dir === 'row';
    const span = (row ? rect.w : rect.h) - GUTTER * (node.children.length - 1);
    let offset = row ? rect.x : rect.y;
    node.children.forEach((child, i) => {
      const size = i === node.children.length - 1 ? (row ? rect.x + rect.w : rect.y + rect.h) - offset : Math.round(span * node.sizes[i]);
      walk(child, row ? { x: offset, y: rect.y, w: size, h: rect.h } : { x: rect.x, y: offset, w: rect.w, h: size }, [...path, i]);
      offset += size;
      if (i < node.children.length - 1) {
        gutters.push({ path, index: i, dir: node.dir, span, rect: row ? { x: offset, y: rect.y, w: GUTTER, h: rect.h } : { x: rect.x, y: offset, w: rect.w, h: GUTTER } });
        offset += GUTTER;
      }
    });
  };
  if (root) walk(root, area, []);
  return { groups, gutters };
}

/** Which zone of `group` a pointer is over: the tab strip and the middle add a tab, the outer quarters split. */
export function zoneIn(rect: Rect, x: number, y: number): Zone {
  if (y < rect.y + HEAD) return 'center';
  const dx = (x - rect.x) / rect.w, dy = (y - rect.y - HEAD) / Math.max(1, rect.h - HEAD);
  const edges: [Edge, number][] = [['left', dx], ['right', 1 - dx], ['top', dy], ['bottom', 1 - dy]];
  const [edge, distance] = edges.reduce((a, b) => (b[1] < a[1] ? b : a));
  return distance < 0.26 ? edge : 'center';
}

/** Where a drop would land, for the preview overlay. */
export function preview(rect: Rect, zone: Zone): Rect {
  const half = { left: { ...rect, w: rect.w / 2 }, right: { ...rect, x: rect.x + rect.w / 2, w: rect.w / 2 }, top: { ...rect, h: rect.h / 2 }, bottom: { ...rect, y: rect.y + rect.h / 2, h: rect.h / 2 } };
  return zone === 'center' ? { ...rect, y: rect.y + HEAD, h: rect.h - HEAD } : half[zone];
}

function valid(node: unknown, depth: number, ids: Set<string>): node is Node {
  if (!node || typeof node !== 'object' || depth > MAX_DEPTH) return false;
  const value = node as Record<string, unknown>;
  if (value.kind === 'tabs') return typeof value.id === 'string' && /^t\d{1,6}$/.test(value.id) && !ids.has(value.id) && !!ids.add(value.id)
    && Array.isArray(value.panes) && value.panes.length > 0 && value.panes.every(pane => typeof pane === 'string') && typeof value.active === 'string';
  return value.kind === 'split' && (value.dir === 'row' || value.dir === 'column') && Array.isArray(value.children) && Array.isArray(value.sizes)
    && value.children.length >= 2 && value.children.length === value.sizes.length
    && value.sizes.every(size => typeof size === 'number' && size > 0 && size < 1) && value.children.every(child => valid(child, depth + 1, ids));
}

/** Accept a stored layout only if its shape is sound; anything else falls back to the default. */
export function parseLayout(value: unknown): Layout {
  const saved = value as Partial<Layout> | null;
  if (saved?.version === 2 && Array.isArray(saved.hidden) && saved.hidden.every(pane => typeof pane === 'string') && (saved.root === null || valid(saved.root, 0, new Set()))) {
    return { version: 2, hidden: saved.hidden, root: tidy(JSON.parse(JSON.stringify(saved.root))) };
  }
  return defaultLayout();
}
