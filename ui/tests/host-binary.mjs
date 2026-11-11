import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Ask Cargo, because build.target-dir or CARGO_TARGET_DIR may place builds outside the repository.
const metadata = execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], {
  cwd: fileURLToPath(new URL('../..', import.meta.url)), encoding: 'utf8',
});
const target = JSON.parse(metadata).target_directory;
export const hostBinary = join(target, 'debug', 'colecto-host');
export const appBinary = join(target, 'debug', 'colecto');
