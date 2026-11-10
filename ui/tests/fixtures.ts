import { test as base, expect } from '@playwright/test';
export const ownerToken = 'b'.repeat(64);
export const test = base.extend({
  page: async ({ page }, use) => {
    await page.addInitScript(token => {
      if (window.location.origin === 'http://127.0.0.1:4179') sessionStorage.setItem('colecto.token', token);
    }, ownerToken);
    await page.route('**/api/v1/messages/cache?*', route => {
      if (route.request().headers()['x-colecto-account'] === 'demo') return route.continue();
      return route.fulfill({ json: { folder: new URL(route.request().url()).searchParams.get('folder'), messages: [], fetched_at: null } });
    });
    await use(page);
  },
});
export { expect };
