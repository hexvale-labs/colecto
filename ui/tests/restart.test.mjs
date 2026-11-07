import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const binary = fileURLToPath(new URL('../../target/debug/colecto-host', import.meta.url));
async function start(directory) {
  const child = spawn(binary, { env: { ...process.env, COLECTO_PORT: '0', COLECTO_DATA_DIR: directory }, stdio: ['ignore', 'pipe', 'pipe'] });
  const exited = once(child, 'exit');
  let output = '';
  let errors = '';
  child.stderr.on('data', data => { errors += data; });
  const url = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => { child.kill('SIGKILL'); reject(new Error(`Host startup timed out: ${errors}`)); }, 10000);
    child.on('error', error => { clearTimeout(timer); reject(error); });
    child.on('exit', code => { clearTimeout(timer); reject(new Error(`Host exited: ${code}; ${errors}`)); });
    child.stdout.on('data', data => {
      output += data;
      const match = output.match(/http:\/\/127\.0\.0\.1:\d+/);
      if (match) { clearTimeout(timer); resolve(match[0]); }
    });
  });
  return { url, stop: async () => { child.kill('SIGINT'); await exited; } };
}

test('a saved draft survives an actual Host process restart', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'colecto-restart-'));
  let host;
  try {
    host = await start(directory);
    const headers = { 'content-type': 'application/json', 'x-colecto-extension': 'composer', 'x-colecto-account': 'demo', origin: host.url };
    const response = await fetch(`${host.url}/api/v1/drafts`, { method: 'POST', headers,
      body: JSON.stringify({ recipient: 'friend@example.test', subject: 'Remember me', body: 'Across process boundaries. 世界' }) });
    assert.equal(response.status, 200);
    const saved = await response.json();
    await host.stop(); host = null;
    host = await start(directory);
    const reopened = await fetch(`${host.url}/api/v1/drafts/${saved.id}`, { headers: { ...headers, origin: host.url } });
    assert.equal(reopened.status, 200);
    assert.deepEqual(await reopened.json(), saved);
    const frontend = await fetch(host.url);
    assert.equal(frontend.status, 200);
    assert.match(await frontend.text(), /Colecto/);
  } finally {
    await host?.stop();
    await rm(directory, { recursive: true, force: true });
  }
});
