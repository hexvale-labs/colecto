import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { nextZoom } from '../src/lib/zoom.ts';

test('zoom steps like a browser and ignores other keys (#119)', () => {
  assert.equal(nextZoom(1, '+'), 1.1);
  assert.equal(nextZoom(1, '='), 1.1);
  assert.equal(nextZoom(1, '-'), 0.9);
  assert.equal(nextZoom(1.5, '0'), 1);
  assert.equal(nextZoom(2, '+'), 2);
  assert.equal(nextZoom(0.5, '-'), 0.5);
  // A level saved by an older build that is not a step starts again from 100%.
  assert.equal(nextZoom(1.05, '+'), 1.1);
  assert.equal(nextZoom(1, 'k'), null);
});
