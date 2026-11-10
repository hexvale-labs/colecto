import { test as base, expect, type Page } from '@playwright/test';
export const ownerToken = 'b'.repeat(64);
export const test = base.extend({
  page: async ({ page }, use) => {
    // The Host keeps the layout and selected account, so every test starts from the defaults.
    await page.request.post('http://127.0.0.1:4179/api/v1/preferences', {
      headers: { authorization: `Bearer ${ownerToken}`, origin: 'http://127.0.0.1:4179', 'x-colecto-extension': 'host', 'x-colecto-account': 'demo' },
      data: { workspace: {}, account: 'demo' },
    });
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

/** Pick an account from the switcher in the Accounts pane. */
export async function chooseAccount(page: Page, name: string) {
  await page.getByRole('button', { name: 'Current account' }).click();
  await page.getByRole('option', { name, exact: true }).click();
}
