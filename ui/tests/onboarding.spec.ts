import { test, expect } from './fixtures';

const demoOnly = [{ id: 'demo', settings: null, connected: true, configured: false }];
const work = { id: 'a'.repeat(32), configured: true, connected: true, settings: { name: 'Work', address: 'work@example.test', protocol: 'imap', imap_host: 'imap.example.test', imap_port: 993, smtp_host: 'smtp.example.test', smtp_port: 587, username: 'work', session_url: '' } };

/** Serve preferences from memory so these specs never change the shared Host's first-run state. */
async function preferences(page: import('@playwright/test').Page, initial: Record<string, unknown>) {
  const real = await (await page.request.get('/api/v1/preferences', { headers: { authorization: `Bearer ${'b'.repeat(64)}`, 'x-colecto-extension': 'host', 'x-colecto-account': 'demo' } })).json();
  let state = { ...real, ...initial };
  const saved: Record<string, unknown>[] = [];
  await page.route('**/api/v1/preferences', async route => {
    if (route.request().method() === 'POST') {
      const input = route.request().postDataJSON();
      saved.push(input);
      state = { ...state, ...input, accounts_enabled: { ...state.accounts_enabled, ...input.accounts_enabled } };
    }
    await route.fulfill({ json: state });
  });
  return saved;
}

test('first launch shows onboarding even though the demo account exists', async ({ page }) => {
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: demoOnly }));
  const saved = await preferences(page, { onboarded: false, accounts_enabled: {} });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Welcome to Colecto' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Settings', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Add your first account' }).click();
  await expect(page.getByRole('heading', { name: 'Add your account' })).toBeVisible();
  await expect(page.getByLabel('Email address', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Back', exact: true }).click();
  await page.getByRole('button', { name: 'Explore with sample mail first' }).click();
  await expect(page.getByRole('heading', { name: 'Inbox', exact: true })).toBeVisible();
  expect(saved.at(-1)).toMatchObject({ onboarded: true, accounts_enabled: { demo: true } });
});

test('adding the first account finishes onboarding and opens it', async ({ page }) => {
  let accounts: unknown[] = demoOnly;
  await page.route('**/api/v1/accounts', route => route.request().method() === 'POST'
    ? (accounts = [...demoOnly, work], route.fulfill({ json: work }))
    : route.fulfill({ json: accounts }));
  await page.route('**/api/v1/messages/**', route => route.request().headers()['x-colecto-account'] === 'demo' ? route.continue() : route.fulfill({ json: [] }));
  await page.route('**/api/v1/labels', route => route.fulfill({ json: {} }));
  const saved = await preferences(page, { onboarded: false, accounts_enabled: {} });
  await page.goto('/');
  await page.getByRole('button', { name: 'Add your first account' }).click();
  await page.getByLabel('Email address', { exact: true }).fill('work@example.test');
  await page.getByRole('button', { name: 'Continue', exact: true }).click();
  await page.getByLabel('Incoming (IMAP) server').fill('imap.example.test');
  await page.getByLabel('Outgoing (SMTP) server').fill('smtp.example.test');
  await page.getByLabel('Password', { exact: true }).fill('provider-secret');
  await page.getByRole('button', { name: 'Continue', exact: true }).click();
  await page.getByLabel('Account name (optional)').fill('Work');
  await page.getByRole('button', { name: 'Connect account' }).click();
  await expect(page.getByRole('heading', { name: 'You’re all set' })).toBeVisible();
  await page.getByRole('button', { name: 'Open Inbox' }).click();
  await expect(page.getByRole('button', { name: 'Current account' })).toContainText('Work');
  expect(saved.at(-1)).toMatchObject({ onboarded: true, account: work.id });
});

test('an account switched off leaves the switcher and the demo account starts off', async ({ page }) => {
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: [...demoOnly, work] }));
  await page.route('**/api/v1/messages/**', route => route.request().headers()['x-colecto-account'] === 'demo' ? route.continue() : route.fulfill({ json: [] }));
  await page.route('**/api/v1/labels', route => route.fulfill({ json: {} }));
  await preferences(page, { onboarded: true, accounts_enabled: {}, account: 'demo' });
  await page.goto('/');
  // The saved selection points at the demo account, which is off by default, so Work opens instead.
  await expect(page.getByRole('button', { name: 'Current account' })).toContainText('Work');
  await page.getByRole('button', { name: 'Current account' }).click();
  await expect(page.getByRole('option', { name: 'Demo account' })).toHaveCount(0);
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('switch', { name: 'Use Demo account' })).not.toBeChecked();
  await page.getByRole('switch', { name: 'Use Demo account' }).check();
  await page.getByRole('switch', { name: 'Use Work' }).uncheck();
  await page.getByRole('button', { name: 'Close settings' }).click();
  await expect(page.getByRole('button', { name: 'Current account' })).toContainText('Demo account');
  await page.getByRole('button', { name: 'Current account' }).click();
  await expect(page.getByRole('option', { name: 'Work', exact: true })).toHaveCount(0);
});
