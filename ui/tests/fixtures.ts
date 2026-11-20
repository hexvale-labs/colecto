import { test as base, expect, type Page } from '@playwright/test';
const testOrigin = `http://127.0.0.1:${process.env.COLECTO_TEST_PORT ?? '4179'}`;
export const ownerToken = 'b'.repeat(64);
const owner = { authorization: `Bearer ${'b'.repeat(64)}`, origin: testOrigin, 'x-colecto-extension': 'host', 'x-colecto-account': 'demo' };
/** Bundled extensions' setting keys, returned to their defaults before each test. */
const bundledSettings: Record<string, string[]> = {
  inbox: ['density', 'date_groups', 'prefetch', 'prefetch_count'],
  reader: ['remote_images'],
  notifications: ['mail', 'approvals', 'updates'],
};
/** Core settings shown in Settings, returned to their defaults before each test. */
const coreSettings = ['accent', 'font', 'font_size', 'text_spacing.letter', 'text_spacing.word', 'text_spacing.line', 'text_spacing.paragraph',
  'notifications.enabled', 'notifications.system', 'notifications.position'];

/** Change a bundled extension's settings, as Settings would. */
export async function setExtensionSettings(page: Page, id: string, values: Record<string, unknown>) {
  await page.request.put(`${testOrigin}/api/v1/extensions/${id}/settings`, { headers: owner, data: values });
}

/** Serve one burst of Host events, then nothing. The shell hears them on its own stream and the
 *  extensions on theirs, where each event names its recipients; here every running extension
 *  gets every event, which the Host's filtering would narrow down. */
export async function hostEvents(page: Page, list: [string, unknown][]) {
  const recipients = ['inbox', 'reader', 'composer', 'notifications', 'ai'];
  const serve = (pattern: string, wrap: (data: unknown) => unknown) => {
    let sent = false;
    return page.route(pattern, async route => {
      if (sent) return route.fulfill({ status: 204 });
      sent = true;
      // Wait until the workspace is ready, so nothing arrives before anyone listens.
      await expect(page.getByRole('button', { name: 'Settings', exact: true })).toBeEnabled();
      return route.fulfill({ status: 200, contentType: 'text/event-stream', body: list.map(([name, data]) => `event: ${name}\ndata: ${JSON.stringify(wrap(data))}\n\n`).join('') });
    });
  };
  await serve('**/api/v1/events', data => data);
  // The extension stream reopens as each extension starts; send once it carries all of them.
  let sent = false;
  await page.route('**/api/v1/extensions/events', async route => {
    const tokens = (route.request().postDataJSON() as { tokens?: unknown[] })?.tokens ?? [];
    if (sent || tokens.length < recipients.length) return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
    sent = true;
    return route.fulfill({ status: 200, contentType: 'text/event-stream', body: list.map(([name, data]) => `event: ${name}\ndata: ${JSON.stringify({ to: recipients, data })}\n\n`).join('') });
  });
}
export const test = base.extend({
  page: async ({ page }, use) => {
    // The Host keeps the layout and selected account, so every test starts from the defaults.
    await page.request.post(`${testOrigin}/api/v1/preferences`, {
      headers: { authorization: `Bearer ${ownerToken}`, origin: testOrigin, 'x-colecto-extension': 'host', 'x-colecto-account': 'demo' },
      // A fresh Host starts with the demo account off and onboarding pending; most specs use sample mail.
      data: { workspace: {}, account: 'demo', list_density: 'full', date_groups: true, accounts_enabled: { demo: true }, onboarded: true,
        reset: coreSettings },
    });
    // Bundled extensions keep their settings in the shared Host too: start each test from defaults.
    for (const [id, keys] of Object.entries(bundledSettings)) {
      await page.request.post(`${testOrigin}/api/v1/extensions/${id}/settings/reset`, { headers: owner, data: { keys } });
    }
    await page.addInitScript(({ token, origin }) => {
      // `window.origin` is null in sandboxed frames (extension runtimes), unlike location.origin.
      if (window.origin === origin) {
        sessionStorage.setItem('colecto.token', token);
        sessionStorage.setItem('colecto.fastStartup', '1');
      }
    }, { token: ownerToken, origin: testOrigin });
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
