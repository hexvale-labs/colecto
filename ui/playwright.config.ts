import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests', testMatch: '**/*.spec.ts', fullyParallel: false, workers: 1,
  use: { baseURL: 'http://127.0.0.1:4179', headless: true, trace: 'retain-on-failure' },
  webServer: { command: 'bun tests/serve.mjs', url: 'http://127.0.0.1:4179', reuseExistingServer: false, timeout: 30000 },
});
