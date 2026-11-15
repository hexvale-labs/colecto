import { expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

// Windows and macOS ignore case in file names, so `./toasts.svelte` (meant for
// `toasts.svelte.ts`) once resolved to `Toasts.svelte` there and broke the release build.
// Linux CI cannot see that; this does.
test('no two UI sources differ only in case or a trailing .ts', () => {
  const root = fileURLToPath(new URL('../src', import.meta.url));
  const seen = new Map();
  const clashes = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) { walk(path); continue; }
      // An import names `x.svelte` for `x.svelte.ts`, so both count as the same name.
      const key = relative(root, path).toLowerCase().replace(/\.svelte\.ts$/, '.svelte');
      if (seen.has(key)) clashes.push(`${seen.get(key)} and ${relative(root, path)}`);
      else seen.set(key, relative(root, path));
    }
  };
  walk(root);
  expect(clashes).toEqual([]);
});
