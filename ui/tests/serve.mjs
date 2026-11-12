import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { hostBinary } from './host-binary.mjs';
const directory = mkdtempSync(join(tmpdir(), 'colecto-ui-'));
writeFileSync(join(directory, 'owner-token'), 'b'.repeat(64), { mode: 0o600 });
const child = spawn(hostBinary, {
  env: { ...process.env, COLECTO_PORT: process.env.COLECTO_TEST_PORT ?? '4179', COLECTO_DATA_DIR: directory }, stdio: 'inherit',
});
function stop() { child.kill('SIGINT'); }
process.on('SIGTERM', stop);
process.on('SIGINT', stop);
child.on('exit', (code) => { rmSync(directory, { recursive: true, force: true }); process.exit(code ?? 1); });
