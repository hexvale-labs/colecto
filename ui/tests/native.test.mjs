import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { appBinary as binary } from './host-binary.mjs';

/** TCP ports the process listens on, from `ss` (empty when it has none). */
function listening(pid) {
  return execFileSync('ss', ['-ltnpH'], { encoding: 'utf8' }).split('\n').filter(line => line.includes(`pid=${pid},`));
}

// The desktop window loads Colecto from the `colecto://` scheme: no port is open, and the page's
// own authenticated requests succeed in-process. A port opens only while MCP is on.
test('native window starts without a port and its page reaches the in-process service', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'colecto-native-'));
  const environment = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('COLECTO_') && key !== 'RUST_LOG'));
  const child = spawn(binary, { env: { ...environment, COLECTO_DATA_DIR: directory, COLECTO_UPDATE_CHECK: 'off', RUST_LOG: 'info,tower_http=debug' }, stdio: ['ignore', 'pipe', 'pipe'] });
  const exited = once(child, 'exit');
  let output = '';
  child.stdout.on('data', data => output += data);
  child.stderr.on('data', data => output += data);
  try {
    const deadline = Date.now() + 15000;
    let started = false; let windowFound = false; let answered = 0;
    while (Date.now() < deadline) {
      assert.equal(child.exitCode, null, `Native app exited: ${output}`);
      started = output.includes('Colecto started url=colecto://localhost');
      windowFound = execFileSync('xwininfo', ['-root', '-tree'], { encoding: 'utf8' }).includes('"Colecto"');
      answered = output.match(/finished processing request.*status=200/g)?.length ?? 0;
      // The onboarding page loads the owner's preferences, accounts and extensions.
      if (started && windowFound && answered >= 3) break;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    assert.ok(started && windowFound, `Native window did not start: ${output}`);
    assert.ok(answered >= 3, `The page's requests did not succeed: ${output}`);
    assert.doesNotMatch(output, /status=40[13]/, 'The page was refused');
    assert.deepEqual(listening(child.pid), [], 'The app must not listen on a port while MCP is off');
  } finally {
    child.kill('SIGTERM');
    await exited;
    await rm(directory, { recursive: true, force: true });
  }
}, 20000);
