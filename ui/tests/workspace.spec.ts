import { test, expect, ownerToken } from './fixtures';

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
    if (url.pathname.endsWith('/cache')) return route.fallback();
    if (url.searchParams.get('folder') === 'sent') return route.fulfill({ json: [] });
    const id = url.pathname.split('/').at(-1);
    return route.fulfill({ json: id === 'refresh' ? messages : messages.find(message => message.id === id) });
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
    const headers = { ...route.request().headers() };
    headers.authorization = `Bearer ${ownerToken}`;
    return route.continue({ headers });
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
  await page.route('**/api/v1/labels', route => route.fulfill({ json: {} }));
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: accounts }));
  await page.route('**/api/v1/messages/refresh*', route => {
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
  await page.route('**/api/v1/labels', route => route.fulfill({ json: {} }));
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
  await page.route('**/api/v1/messages**', route => new URL(route.request().url()).pathname.endsWith('/cache') ? route.fallback() : route.fulfill({ json: new URL(route.request().url()).pathname.endsWith('/refresh') ? [message] : message }));
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

test('command runner opens by keyboard, filters actions and searches drafts', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('Subject', { exact: true }).fill('Find this local note');
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('drafts');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('dialog', { name: 'Commands' })).not.toBeVisible();
  await page.getByRole('searchbox').fill('absent words');
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(page.getByText('No matching messages')).toBeVisible();
  await page.getByRole('searchbox').fill('local note');
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await expect(page.getByRole('button', { name: /Find this local note/ })).toBeVisible();
});


test('worst-case mail remains usable at narrow widths and enlarged text', async ({ page }) => {
  const { worst } = await import('../src/lib/stress-fixtures');
  await page.route('**/api/v1/messages**', route => {
    const id = new URL(route.request().url()).pathname.split('/').at(-1);
    if (id === 'cache') return route.fallback();
    return route.fulfill({ json: id === 'refresh' ? worst : worst.find(message => message.id === id) });
  });
  for (const width of [320, 390, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/');
    await page.getByRole('button', { name: /Aleksandra/ }).click();
    await page.evaluate(() => { document.documentElement.style.fontSize = '200%'; });
    await expect(page.getByRole('heading', { name: worst[0].subject })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    if (width <= 800) await page.getByRole('button', { name: 'Back to list' }).click();
    await page.getByRole('button', { name: /Priya/ }).click();
    await expect(page.locator('.letter .avatar')).toHaveText('👩🏽‍💻');
    await expect(page.getByRole('heading', { name: '(No subject)' })).toBeVisible();
  }
});

test('command navigation preserves canceled unsaved edits and restores focus', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  const subject = page.getByLabel('Subject', { exact: true });
  await subject.fill('Keep my changes');
  await page.keyboard.press('Control+k');
  await page.keyboard.press('Escape');
  await expect(subject).toBeFocused();
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Go to Inbox');
  page.once('dialog', dialog => dialog.dismiss());
  await page.keyboard.press('Enter');
  await expect(subject).toHaveValue('Keep my changes');
});

test('search command keeps focus and send command uses the existing send flow', async ({ page }) => {
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: [{ id: 'demo', settings: null, configured: false, connected: true }] }));
  await page.route('**/api/v1/send', route => route.fulfill({ json: true }));
  await page.goto('/');
  await page.getByRole('button', { name: /Commands/ }).click();
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Search emails');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('searchbox')).toBeFocused();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('To', { exact: true }).fill('maya@example.test');
  await page.getByLabel('Subject', { exact: true }).fill('Command send');
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Send message');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('status')).toHaveText('Accepted by mail server');
});

test('shows cached mail during refresh and preserves reader and editor after proxy failure', async ({ page }) => {
  const cached = { id: 'cache-reader', sender: 'Maya Chen', address: 'maya@example.test', subject: 'Previously fetched mail', body: 'Cached content remains readable.', date: 'Today' };
  await page.route('**/api/v1/messages/cache?*', route => route.fulfill({ json: { folder: 'inbox', messages: [cached], fetched_at: 1 } }));
  await page.route('**/api/v1/messages/cache-reader', route => route.fulfill({ json: cached }));
  let release!: () => void;
  const gate = new Promise<void>(resolve => release = resolve);
  await page.route('**/api/v1/messages/refresh*', async route => {
    await gate;
    return route.fulfill({ status: 502, contentType: 'text/html', body: '<h1>Proxy unavailable SECRET</h1>' });
  });
  await page.goto('/');
  await expect(page.getByText('Fetching latest messages…')).toBeVisible();
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await expect(page.getByRole('region', { name: 'Message reader' })).toBeVisible();
  release();
  await expect(page.getByRole('alert')).toContainText('HTTP 502');
  await expect(page.getByRole('alert')).not.toContainText('SECRET');
  await expect(page.getByRole('region', { name: 'Message reader' })).toBeVisible();
  await expect(page.getByRole('button', { name: /Maya Chen/ })).toBeVisible();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('Subject', { exact: true }).fill('Keep unsaved draft');
  await page.getByRole('button', { name: 'Try again', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('HTTP 502');
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Keep unsaved draft');
});
