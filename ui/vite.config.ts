import { defineConfig, type Plugin } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { readdirSync, existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';

/** Bundles each bundled extension's client part (`src/extensions/<id>/client/*.ts`) into one
 *  classic script, served at `extensions/<id>/client/<name>.js` with a `files.json` index.
 *  Workers and frames load them as plain scripts, not ES modules. */
function bundledClients(): Plugin {
  return {
    name: 'colecto-bundled-clients',
    async generateBundle() {
      const bun = (globalThis as { Bun?: { build: (options: object) => Promise<{ success: boolean; logs: unknown[]; outputs: { text: () => Promise<string> }[] }> } }).Bun;
      if (!bun) throw new Error('Build the UI with `bun run --bun build`: bundled client parts are built with Bun.');
      const root = join(import.meta.dirname, 'src/extensions');
      for (const id of readdirSync(root)) {
        const dir = join(root, id, 'client');
        if (!existsSync(dir)) continue;
        const files: string[] = [];
        for (const name of readdirSync(dir).filter(name => name.endsWith('.ts'))) {
          const result = await bun.build({ entrypoints: [join(dir, name)], format: 'iife', target: 'browser', minify: true });
          if (!result.success) throw new Error(`Building ${id}/client/${name} failed: ${JSON.stringify(result.logs)}`);
          const path = `client/${name.replace(/\.ts$/, '.js')}`;
          files.push(path);
          this.emitFile({ type: 'asset', fileName: `extensions/${id}/${path}`, source: await result.outputs[0].text() });
        }
        this.emitFile({ type: 'asset', fileName: `extensions/${id}/files.json`, source: JSON.stringify(files) });
      }
      // The AI service part, built by `just build ai-service`, ships with the UI: the Host loads
      // bundled service parts from these assets.
      // `COLECTO_AI_SERVICE` names a prebuilt module where Cargo is not around (the Docker UI stage).
      const service = process.env.COLECTO_AI_SERVICE || join(JSON.parse(execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], { cwd: import.meta.dirname, encoding: 'utf8' })).target_directory, 'wasm32-unknown-unknown/release/colecto_ai_service.wasm');
      if (!existsSync(service)) throw new Error(`${service} is missing: run \`just build ai-service\` before building the UI.`);
      this.emitFile({ type: 'asset', fileName: 'extensions/ai/service.wasm', source: readFileSync(service) });
    },
  };
}

export default defineConfig({ plugins: [svelte(), bundledClients()] });
