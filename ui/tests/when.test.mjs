import { expect, test } from 'bun:test';
import { when } from '../src/lib/when.ts';

const now = new Date(2026, 10, 10, 20, 0); // Tue 10 Nov 2026, 20:00 local time

test('groups mail by day and shows compact dates', () => {
  expect(when('Today', now)).toMatchObject({ label: 'Today', group: 'Today' });
  expect(when(new Date(2026, 10, 10, 9, 5).toString(), now).group).toBe('Today');
  expect(when(new Date(2026, 10, 9, 9, 5).toString(), now).group).toBe('Yesterday');
  expect(when('Nov 6', now).group).toBe('Previous 7 days');
  expect(when('Sun, 1 Nov 2026 19:44:07 +0100', now).group).toBe(new Date(2026, 10, 1).toLocaleDateString(undefined, { month: 'long' }));
});

test('pins yearless dates to the latest past occurrence', () => {
  // "Dec 24" without a year must not land in the future (or in 2001, as V8 would parse it).
  expect(when('Dec 24', now).label).toContain('2025');
});

test('keeps text it cannot parse', () => {
  expect(when('Draft', now)).toEqual({ label: 'Draft', group: 'Earlier', full: 'Draft' });
  expect(when('', now).group).toBe('Earlier');
});
