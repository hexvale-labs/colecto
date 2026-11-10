import { expect, test } from 'bun:test';
import { defaultLayout, move, reconcile } from '../src/lib/layout.ts';

test('moving a pane takes it out of its old dock and shows it in the new one', () => {
  const layout = move(defaultLayout(), 'reader', 'bottom');
  expect(layout.docks.center).toEqual([]);
  expect(layout.docks.bottom).toEqual(['reader']);
  expect(layout.active.bottom).toBe('reader');
  expect(layout.active.center).toBe('');
});

test('a dock keeps showing a remaining tab when its active pane leaves', () => {
  let layout = move(defaultLayout(), 'mailbox', 'center');
  expect(layout.docks.center).toEqual(['reader', 'mailbox']);
  layout = move(layout, 'mailbox', 'left');
  expect(layout.active.center).toBe('reader');
});

test('reconcile drops unknown panes, de-duplicates, and places new panes in the center', () => {
  const saved = { ...defaultLayout(), docks: { left: ['mailbox', 'gone'], center: ['reader', 'mailbox'], right: [], bottom: [] } };
  const layout = reconcile(saved, ['mailbox', 'reader', 'ext:panel']);
  expect(layout.docks).toEqual({ left: ['mailbox'], center: ['reader', 'ext:panel'], right: [], bottom: [] });
});
