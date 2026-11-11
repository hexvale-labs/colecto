import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { destinations, isDestination, resolve } from '../src/lib/navigation.ts';

test('the UI navigator and the MCP app.navigate tool offer the same pages', () => {
  const source = readFileSync(new URL('../../crates/colecto-extension-api/src/lib.rs', import.meta.url), 'utf8');
  const list = source.match(/NAVIGATION_TARGETS: &\[&str\] = &\[([^\]]*)\]/)[1];
  const rust = [...list.matchAll(/"([^"]+)"/g)].map(match => match[1]);
  expect(destinations.map(destination => destination.target).sort()).toEqual(rust.sort());
});

test('destinations resolve to folders, the composer or a settings page, and nothing else', () => {
  expect(resolve('sent')).toEqual({ folder: 'Sent' });
  expect(resolve('compose')).toEqual({ compose: true });
  expect(resolve('settings/api-keys')).toEqual({ settings: 'api-keys' });
  for (const value of ['settings/../x', 'trash', '', null]) expect(isDestination(value)).toBe(false);
});
