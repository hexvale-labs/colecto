import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { hostBinary } from './host-binary.mjs';
const directory = mkdtempSync(join(tmpdir(), 'colecto-ui-'));
writeFileSync(join(directory, 'owner-token'), 'b'.repeat(64), { mode: 0o600 });
const child = spawn(hostBinary, {
  env: { ...process.env, COLECTO_PORT: process.env.COLECTO_TEST_PORT ?? '4179', COLECTO_DATA_DIR: directory, COLECTO_UPDATE_CHECK: 'off',
    // The binary may live in a shared Cargo target dir, so name this checkout's UI build explicitly.
    COLECTO_ASSETS: fileURLToPath(new URL('../dist', import.meta.url)) }, stdio: 'inherit',
});
function stop() { child.kill('SIGINT'); }
process.on('SIGTERM', stop);
process.on('SIGINT', stop);
child.on('exit', (code) => { rmSync(directory, { recursive: true, force: true }); process.exit(code ?? 1); });
