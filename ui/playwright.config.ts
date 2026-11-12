import { defineConfig, devices } from '@playwright/test';
const baseURL = `http://127.0.0.1:${process.env.COLECTO_TEST_PORT ?? '4179'}`;
export default defineConfig({
  testDir: './tests', testMatch: '**/*.spec.ts', fullyParallel: false, workers: 1,
  use: { baseURL, headless: true, trace: 'retain-on-failure' },
  // HTML mail also renders in WebKit, the engine behind the Linux and macOS desktop apps.
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 720 } } },
    { name: 'webkit', use: { ...devices['Desktop Safari'], viewport: { width: 1280, height: 720 } }, testMatch: ['**/reader.spec.ts', '**/html-mail.spec.ts'] },
  ],
  webServer: { command: 'bun tests/serve.mjs', url: baseURL, reuseExistingServer: false, timeout: 30000 },
});
