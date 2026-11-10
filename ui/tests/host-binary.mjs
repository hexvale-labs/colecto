import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Ask Cargo, because build.target-dir or CARGO_TARGET_DIR may place builds outside the repository.
const metadata = execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], {
  cwd: fileURLToPath(new URL('../..', import.meta.url)), encoding: 'utf8',
});
export const hostBinary = join(JSON.parse(metadata).target_directory, 'debug', 'colecto-host');
