// Where each pane lives in the workspace. Panes are extension surfaces; docks are the slots they fill.
export type Dock = 'left' | 'center' | 'right' | 'bottom';
export const docks: Dock[] = ['left', 'center', 'right', 'bottom'];
export interface Layout {
  docks: Record<Dock, string[]>;
  active: Record<Dock, string>;
  /** Pixel widths for side docks and the bottom dock's height. */
  size: { left: number; right: number; bottom: number };
}
export const minSize = { left: 240, right: 240, bottom: 140 };
const key = 'colecto.layout.v1';

export function defaultLayout(): Layout {
  return { docks: { left: ['mailbox'], center: ['reader'], right: [], bottom: [] }, active: { left: 'mailbox', center: 'reader', right: '', bottom: '' }, size: { left: 344, right: 360, bottom: 260 } };
}

export function dockOf(layout: Layout, pane: string): Dock | undefined {
  return docks.find(dock => layout.docks[dock].includes(pane));
}

/** Keep exactly the known panes: drop stale ones, add new ones to the center, and repair active tabs. */
export function reconcile(layout: Layout, panes: string[]): Layout {
  const seen = new Set<string>();
  const next: Layout = { docks: { left: [], center: [], right: [], bottom: [] }, active: { ...layout.active }, size: { ...layout.size } };
  for (const dock of docks) next.docks[dock] = (layout.docks[dock] ?? []).filter(pane => panes.includes(pane) && !seen.has(pane) && seen.add(pane));
  for (const pane of panes) if (!seen.has(pane)) next.docks.center.push(pane);
  for (const dock of docks) if (!next.docks[dock].includes(next.active[dock])) next.active[dock] = next.docks[dock][0] ?? '';
  return next;
}

/** Move a pane into a dock (optionally at a tab index) and make it that dock's visible tab. */
export function move(layout: Layout, pane: string, to: Dock, index?: number): Layout {
  const next = reconcile(layout, docks.flatMap(dock => layout.docks[dock]));
  for (const dock of docks) next.docks[dock] = next.docks[dock].filter(item => item !== pane);
  const target = next.docks[to];
  target.splice(index === undefined ? target.length : Math.min(Math.max(index, 0), target.length), 0, pane);
  next.active[to] = pane;
  for (const dock of docks) if (!next.docks[dock].includes(next.active[dock])) next.active[dock] = next.docks[dock][0] ?? '';
  return next;
}

export function loadLayout(): Layout {
  try {
    const saved = JSON.parse(localStorage.getItem(key) ?? 'null');
    if (saved && typeof saved === 'object' && saved.docks && saved.size) {
      const base = defaultLayout();
      const docksValue = Object.fromEntries(docks.map(dock => [dock, Array.isArray(saved.docks[dock]) ? saved.docks[dock].filter((pane: unknown) => typeof pane === 'string') : []])) as Layout['docks'];
      const size = Object.fromEntries((['left', 'right', 'bottom'] as const).map(name => [name, Number.isFinite(saved.size[name]) ? Math.max(minSize[name], saved.size[name]) : base.size[name]])) as Layout['size'];
      return { docks: docksValue, active: { ...base.active, ...saved.active }, size };
    }
  } catch { /* A broken or blocked store falls back to the default layout. */ }
  return defaultLayout();
}

export function saveLayout(layout: Layout) {
  try { localStorage.setItem(key, JSON.stringify(layout)); } catch { /* Layout persistence is optional. */ }
}
