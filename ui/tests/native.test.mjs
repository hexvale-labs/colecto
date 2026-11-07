import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

test('native window starts with its loopback service and bundled registry', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'colecto-native-'));
  const environment = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('COLECTO_')));
  const binary = fileURLToPath(new URL('../../target/debug/colecto-host', import.meta.url));
  const child = spawn(binary, { env: { ...environment, COLECTO_PORT: '0', COLECTO_DATA_DIR: directory }, stdio: ['ignore', 'pipe', 'pipe'] });
  const exited = once(child, 'exit');
  let output = ''; let errors = '';
  child.stdout.on('data', data => output += data);
  child.stderr.on('data', data => errors += data);
  try {
    const deadline = Date.now() + 12000;
    let url; let windowFound = false;
    while (Date.now() < deadline) {
      assert.equal(child.exitCode, null, `Native Host exited: ${errors}`);
      url = output.match(/http:\/\/127\.0\.0\.1:\d+/)?.[0];
      windowFound = execFileSync('xwininfo', ['-root', '-tree'], { encoding: 'utf8' }).includes('"Colecto"');
      if (url && windowFound) break;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    assert.ok(url && windowFound, `Native window did not start: ${errors}`);
    const response = await fetch(`${url}/api/v1/extensions`, { headers: { 'x-colecto-extension': 'host', 'x-colecto-account': 'demo' } });
    assert.equal(response.status, 200);
    const extensions = await response.json();
    assert.equal(extensions.length, 4);
    assert.ok(extensions.every(extension => extension.bundled));
  } finally {
    child.kill('SIGTERM');
    await exited;
    await rm(directory, { recursive: true, force: true });
  }
}, 15000);
