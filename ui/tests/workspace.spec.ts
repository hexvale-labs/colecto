import { test, expect } from '@playwright/test';

test('defaults dark and remembers explicit and system appearance', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Appearance', exact: true }).click();
  await page.getByLabel('Color theme').selectOption('light');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Appearance', exact: true }).click();
  await page.getByLabel('Color theme').selectOption('system');
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('keeps separate closable message tabs and has a sent folder on mobile', async ({ page }) => {
  const messages = [{ id: 'maya', sender: 'Maya Chen', address: 'maya@example.test', subject: 'A small plan for the weekend', body: 'Coffee?', date: 'Today' }, { id: 'alex', sender: 'Alex Rivera', address: 'alex@example.test', subject: 'Plain text, with a little character', body: 'Hello', date: 'Today' }];
  await page.route('**/api/v1/messages**', route => {
    const url = new URL(route.request().url());
    if (url.searchParams.get('folder') === 'sent') return route.fulfill({ json: [] });
    const id = url.pathname.split('/').at(-1);
    return route.fulfill({ json: id === 'messages' ? messages : messages.find(message => message.id === id) });
  });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await page.getByRole('button', { name: 'Back to list' }).click();
  await page.getByRole('button', { name: /Alex Rivera/ }).click();
  await page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'A small plan for the weekend', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'A small plan for the weekend' })).toBeVisible();
  await page.getByRole('button', { name: 'Close A small plan for the weekend', exact: true }).click();
  await expect(page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'A small plan for the weekend', exact: true })).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Plain text, with a little character' })).toBeVisible();
  await page.getByRole('button', { name: 'Sent', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Sent', exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('opens unified settings without discard confirmation for a clean workspace', async ({ page }) => {
  let dialogs = 0; page.on('dialog', async dialog => { dialogs++; await dialog.dismiss(); });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Settings', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Extensions', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Extensions', exact: true })).toBeVisible();
  expect(dialogs).toBe(0);
});

test('remote access token stays in the session and reconnecting preserves a draft', async ({ page }) => {
  const requireToken = true;
  await page.route('**/api/v1/**', route => {
    if (requireToken && route.request().headers().authorization !== 'Bearer sample-owner-token') return route.fulfill({ status: 401, json: { error: 'Enter the daemon access token to connect' } });
    return route.continue();
  });
  await page.goto('/');
  await page.getByLabel('Daemon access token').fill('sample-owner-token');
  await page.getByRole('button', { name: 'Connect', exact: true }).click();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('Subject', { exact: true }).fill('Keep this through reconnect');
  await page.evaluate(() => sessionStorage.removeItem('colecto.token'));
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await page.getByLabel('Daemon access token').fill('sample-owner-token');
  await page.getByRole('button', { name: 'Connect', exact: true }).click();
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Keep this through reconnect');
  expect(await page.evaluate(() => localStorage.getItem('colecto.token'))).toBeNull();
  expect(page.url()).not.toContain('sample-owner-token');
});

test('profile names fall back to email and message tabs stay with their account', async ({ page }) => {
  const accounts = ['a', 'b'].map(id => ({ id, configured: true, connected: false, settings: { name: '', address: `${id}@example.test`, protocol: 'imap', imap_host: '', imap_port: 993, smtp_host: '', smtp_port: 587, username: '', session_url: '' } }));
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: accounts }));
  await page.route('**/api/v1/messages', route => {
    const account = route.request().headers()['x-colecto-account'];
    return route.fulfill({ json: [{ id: 'same-id', sender: account, address: `${account}@example.test`, subject: `Mail for ${account}`, body: 'Body', date: 'Today' }] });
  });
  await page.route('**/api/v1/messages/same-id', route => {
    const account = route.request().headers()['x-colecto-account'];
    return route.fulfill({ json: { id: 'same-id', sender: account, address: `${account}@example.test`, subject: `Mail for ${account}`, body: 'Body', date: 'Today' } });
  });
  await page.goto('/');
  await page.getByLabel('Current account').selectOption('a');
  await page.getByRole('button', { name: /Mail for a/ }).click();
  await expect(page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'Mail for a', exact: true })).toBeVisible();
  await page.getByLabel('Current account').selectOption('b');
  await expect(page.getByRole('navigation', { name: 'Open messages' })).toHaveCount(0);
  await expect(page.getByRole('combobox', { name: 'Current account' }).locator('option:checked')).toHaveText('b@example.test');
  await page.getByRole('button', { name: /Mail for b/ }).click();
  await page.getByLabel('Current account').selectOption('a');
  await expect(page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'Mail for a', exact: true })).toBeVisible();
  await expect(page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'Mail for b', exact: true })).toHaveCount(0);
});

test('account action labels use email when the optional profile name is empty', async ({ page }) => {
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: [{ id: 'unnamed', configured: true, connected: false, settings: { name: '', address: 'unnamed@example.test', protocol: 'imap', imap_host: 'imap.example.test', imap_port: 993, smtp_host: 'smtp.example.test', smtp_port: 587, username: 'unnamed', session_url: '' } }] }));
  await page.route('**/api/v1/security', route => route.fulfill({ json: { protected: false, locked: false, cleanup_pending: false } }));
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit / reconnect unnamed@example.test', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Disconnect unnamed@example.test', exact: true })).toBeVisible();
  let question = '';
  page.once('dialog', async dialog => { question = dialog.message(); await dialog.dismiss(); });
  await page.getByRole('button', { name: 'Disconnect unnamed@example.test', exact: true }).click();
  expect(question).toContain('Disconnect unnamed@example.test?');
});

test('mobile controls stay reachable and mail text stays selectable on narrow screens', async ({ page }) => {
  const message = { id: 'mobile-reader', sender: 'Maya Chen', address: 'maya@example.test', subject: 'Reading on a phone', body: 'Selectable message text.\n'.repeat(100), date: 'Today' };
  await page.route('**/api/v1/messages**', route => route.fulfill({ json: new URL(route.request().url()).pathname.endsWith('/messages') ? [message] : message }));
  await page.setViewportSize({ width: 320, height: 640 });
  await page.goto('/');
  const folders = page.getByRole('navigation', { name: 'Mail folders' });
  const bounds = await folders.boundingBox();
  expect(bounds!.y + bounds!.height).toBe(640);
  for (const button of await folders.getByRole('button').all()) {
    const box = await button.boundingBox();
    expect(box!.width).toBeGreaterThanOrEqual(44);
    expect(box!.height).toBeGreaterThanOrEqual(48);
  }
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await expect(page.locator('.letter-body')).toBeVisible();
  expect(await page.locator('.letter').evaluate(el => getComputedStyle(el).userSelect)).toBe('text');
  await page.locator('.reading-pane').evaluate(el => el.scrollTop = el.scrollHeight);
  await expect(page.getByRole('button', { name: 'Back to list' })).toBeInViewport();
  await page.getByRole('button', { name: 'Back to list' }).click();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('Message', { exact: true }).fill('A long draft\n'.repeat(100));
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Draft saved');
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(320);
});
