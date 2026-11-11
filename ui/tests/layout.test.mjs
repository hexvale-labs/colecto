import { expect, test } from 'bun:test';
import { defaultLayout, dock, dockEdge, groupOf, leaves, measure, parseLayout, reconcile, setVisible, zoneIn } from '../src/lib/layout.ts';

const panes = layout => leaves(layout.root).map(group => group.panes);

test('the default puts the account switcher under the mail list, with the reader beside them', () => {
  expect(panes(defaultLayout())).toEqual([['mailbox'], ['accounts'], ['reader']]);
});

test('dropping on a group edge keeps splitting space, so three panes can share one column', () => {
  const layout = dock(defaultLayout(), 'reader', groupOf(defaultLayout(), 'accounts').id, 'bottom');
  const column = layout.root;
  // The reader left its own column, so the mail column became the whole workspace: three stacked groups.
  expect(column.dir).toBe('column');
  expect(panes(layout)).toEqual([['mailbox'], ['accounts'], ['reader']]);
  expect(column.sizes.reduce((a, b) => a + b)).toBeCloseTo(1);
});

test('reordering groups within one column keeps every group\'s height', () => {
  let layout = reconcile(defaultLayout(), ['mailbox', 'accounts', 'reader', 'ext:a']);
  layout = dock(layout, 'ext:a', groupOf(layout, 'accounts').id, 'bottom');
  const column = () => layout.root.children[0];
  column().sizes = [0.6, 0.15, 0.25];
  // Move the mail list below the extension: the order changes, the sizes travel with their groups.
  layout = dock(layout, 'mailbox', groupOf(layout, 'ext:a').id, 'bottom');
  expect(panes(layout).slice(0, 3)).toEqual([['accounts'], ['ext:a'], ['mailbox']]);
  expect(column().sizes).toEqual([0.15, 0.25, 0.6]);
  // Two groups swap places the same way, instead of resetting to an even split.
  layout = dock(layout, 'ext:a', groupOf(layout, 'accounts').id, 'top');
  expect(column().sizes).toEqual([0.25, 0.15, 0.6]);
});

test('the main area divides into four by splitting twice in each direction', () => {
  let layout = reconcile(defaultLayout(), ['mailbox', 'accounts', 'reader', 'ext:a', 'ext:b', 'ext:c']);
  const reader = () => groupOf(layout, 'reader').id;
  layout = dock(layout, 'ext:a', reader(), 'right');
  layout = dock(layout, 'ext:b', reader(), 'bottom');
  layout = dock(layout, 'ext:c', groupOf(layout, 'ext:a').id, 'bottom');
  const { groups } = measure(layout.root, { x: 0, y: 0, w: 1201, h: 801 });
  const rects = ['reader', 'ext:a', 'ext:b', 'ext:c'].map(pane => groups.get(groupOf(layout, pane).id));
  // Two columns and two rows of groups beside the mail column: a 2 × 2 main area.
  expect(new Set(rects.map(rect => rect.x)).size).toBe(2);
  expect(new Set(rects.map(rect => rect.y)).size).toBe(2);
  expect(rects.every(rect => rect.x > 300 && rect.h < 401)).toBe(true);
});

test('dropping in the middle or on the tab strip adds a tab and shows it', () => {
  const layout = dock(defaultLayout(), 'accounts', groupOf(defaultLayout(), 'reader').id, 'center', 0);
  expect(panes(layout)).toEqual([['mailbox'], ['accounts', 'reader']]);
  expect(groupOf(layout, 'reader').active).toBe('accounts');
});

test('a lone pane dropped on its own edge stays put, and docking along the workspace edge wraps the root', () => {
  const start = defaultLayout();
  expect(dock(start, 'reader', groupOf(start, 'reader').id, 'left')).toBe(start);
  const layout = dockEdge(start, 'accounts', 'bottom');
  expect(layout.root.dir).toBe('column');
  expect(layout.root.children.at(-1).panes).toEqual(['accounts']);
});

test('hidden panes leave the tree and come back when shown; unknown panes are dropped', () => {
  let layout = setVisible(defaultLayout(), 'accounts', false);
  expect(panes(reconcile(layout, ['mailbox', 'accounts', 'reader']))).toEqual([['mailbox'], ['reader']]);
  layout = reconcile(setVisible(layout, 'accounts', true), ['mailbox', 'accounts', 'reader', 'ext:new']);
  expect(panes(layout)).toEqual([['mailbox'], ['reader', 'accounts', 'ext:new']]);
  expect(panes(reconcile(defaultLayout(), ['reader']))).toEqual([['reader']]);
});

test('stored layouts with a broken shape fall back to the default', () => {
  expect(parseLayout(null)).toEqual(defaultLayout());
  expect(parseLayout({ version: 1, docks: {} })).toEqual(defaultLayout());
  const cyclicIds = { version: 2, hidden: [], root: { kind: 'split', dir: 'row', sizes: [0.5, 0.5], children: [{ kind: 'tabs', id: 't1', panes: ['a'], active: 'a' }, { kind: 'tabs', id: 't1', panes: ['b'], active: 'b' }] } };
  expect(parseLayout(cyclicIds)).toEqual(defaultLayout());
  const good = dockEdge(defaultLayout(), 'reader', 'right');
  expect(parseLayout(JSON.parse(JSON.stringify(good)))).toEqual(good);
});

test('pointer zones: the tab strip and centre add a tab, the outer quarters split', () => {
  const rect = { x: 0, y: 0, w: 400, h: 434 };
  expect(zoneIn(rect, 200, 10)).toBe('center');
  expect(zoneIn(rect, 200, 230)).toBe('center');
  expect(zoneIn(rect, 20, 230)).toBe('left');
  expect(zoneIn(rect, 390, 230)).toBe('right');
  expect(zoneIn(rect, 200, 420)).toBe('bottom');
});
