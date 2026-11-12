async function openExtensions(page: import('@playwright/test').Page) {
  // Settings is a card over the workspace; open it unless it is open, or the workspace is off and Extensions shows on its own.
  const settings = page.getByRole('button', { name: 'Settings', exact: true });
  if (!await page.getByRole('dialog', { name: 'Settings' }).count() && await settings.count()) await settings.click();
  await page.getByRole('button', { name: 'Extensions', exact: true }).click();
}
import { test, expect, ownerToken, chooseAccount } from './fixtures';

test('reads sample messages as text and returns to the inbox on a phone', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Inbox', exact: true })).toBeVisible();
  await page.getByRole('button', { name: /Alex Rivera/ }).click();
  await expect(page.getByRole('heading', { name: 'Plain text, with a little character' })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Message reader' }).getByText("<script>alert('hello')</script>", { exact: false })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Do not click', exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole('button', { name: 'Back to list' }).click();
  await expect(page.getByRole('button', { name: /Maya Chen/ })).toBeVisible();
});

test('creates a draft, reloads it, updates it, and reopens it', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('To', { exact: true }).fill('maya@example.test');
  await page.getByLabel('Subject', { exact: true }).fill('Coffee by the river');
  await page.getByLabel('Message', { exact: true }).fill('Saturday sounds lovely. 世界');
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Draft editor' }).getByRole('status')).toContainText('Draft saved');
  await page.reload();
  await page.getByRole('button', { name: /^Drafts/ }).click();
  await page.getByRole('button', { name: /Coffee by the river/ }).click();
  await expect(page.getByLabel('Message', { exact: true })).toHaveValue('Saturday sounds lovely. 世界');
  await page.getByLabel('Subject', { exact: true }).fill('Sunday by the river');
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Draft editor' }).getByRole('status')).toContainText('Draft saved');
  await page.reload();
  await page.getByRole('button', { name: /^Drafts/ }).click();
  await page.getByRole('button', { name: /Sunday by the river/ }).click();
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Sunday by the river');
});

test('keeps unsaved edits when navigation is canceled or saving fails', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('Subject', { exact: true }).fill('Do not lose this');
  page.once('dialog', dialog => dialog.dismiss());
  await page.getByRole('button', { name: /^Inbox/ }).click();
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Do not lose this');
  await page.route('**/api/v1/drafts', route => route.request().method() === 'POST'
    ? route.fulfill({ status: 500, json: { error: 'Storage is unavailable' } }) : route.continue());
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Storage is unavailable');
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Do not lose this');
  await expect(page.getByRole('status')).not.toContainText('Draft saved');
  page.once('dialog', dialog => dialog.accept());
  await page.getByRole('button', { name: /^Inbox/ }).click();
  await expect(page.getByRole('heading', { name: 'Inbox', exact: true })).toBeVisible();
});

test('recovers from a mailbox load failure and handles an empty folder', async ({ page }) => {
  await page.route('**/api/v1/messages/refresh*', route => route.fulfill({ status: 503, json: { error: 'Mailbox unavailable' } }), { times: 1 });
  await page.goto('/');
  await expect(page.getByRole('alert')).toContainText('Mailbox unavailable');
  await page.getByRole('button', { name: 'Try again' }).click();
  await expect(page.getByRole('button', { name: /Maya Chen/ })).toBeVisible();
  await page.route('**/api/v1/drafts', route => route.fulfill({ json: [] }));
  await page.getByRole('button', { name: /^Drafts/ }).click();
  await expect(page.getByText('No drafts yet')).toBeVisible();
});

test('shows a failed draft-open error and recovery on a narrow screen', async ({ page, request }) => {
  const response = await request.post('/api/v1/drafts', {
    headers: { authorization: `Bearer ${ownerToken}`, origin: 'http://127.0.0.1:4179', 'x-colecto-extension': 'composer', 'x-colecto-account': 'demo' },
    data: { recipient: '', subject: 'Open failure example', body: 'Still saved' },
  });
  expect(response.ok()).toBe(true);
  const draft = await response.json();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: /^Drafts/ }).click();
  await page.route(`**/api/v1/drafts/${draft.id}`, route => route.fulfill({ status: 404, json: { error: 'Draft unavailable' } }), { times: 1 });
  await page.getByRole('button', { name: /Open failure example/ }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page.getByRole('alert')).toContainText('Draft unavailable');
  await page.getByRole('button', { name: 'Try again' }).click();
  await page.getByRole('button', { name: /Open failure example/ }).click();
  await expect(page.getByLabel('Message', { exact: true })).toHaveValue('Still saved');
});

test('sending preserves edits on failure and prevents duplicate submission after acceptance', async ({ page }) => {
  await page.route('**/api/v1/labels', route => route.fulfill({ json: {} }));
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: [{ id: 'demo', settings: null, connected: true, configured: false }] }));
  await page.route('**/api/v1/account', route => route.fulfill({ json: true }));
  let sends = 0;
  await page.route('**/api/v1/send', async route => {
    sends++;
    expect(route.request().postDataJSON()).toEqual({ recipient: 'friend@example.test', subject: 'Hello', body: 'Keep this text' });
    await route.fulfill(sends === 1
      ? { status: 502, json: { error: 'Mail server unavailable' } }
      : { json: true });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('To', { exact: true }).fill('friend@example.test');
  await page.getByLabel('Subject', { exact: true }).fill('Hello');
  await page.getByLabel('Message', { exact: true }).fill('Keep this text');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Mail server unavailable');
  await expect(page.getByLabel('Message', { exact: true })).toHaveValue('Keep this text');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Accepted by mail server');
  await expect(page.getByRole('region', { name: 'Draft editor' }).getByRole('button', { name: 'Sent', exact: true })).toBeDisabled();
  expect(sends).toBe(2);
});

test('manages encrypted accounts, switches drafts and protects unsaved changes', async ({ page }) => {
  test.setTimeout(90000);
  await page.route('**/api/v1/messages/refresh*', route => route.request().headers()['x-colecto-account'] !== 'demo' ? route.fulfill({ json: [] }) : route.continue());
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Accounts & security' })).toBeVisible();
  await page.getByLabel('Master password', { exact: true }).fill('correct horse battery staple');
  await page.getByLabel('Confirm master password').fill('correct horse battery staple');
  await page.getByRole('button', { name: 'Enable password protection' }).click();
  await expect(page.getByText('Password protection enabled', { exact: true })).toBeVisible({ timeout: 20000 });
  await expect(page.getByLabel('Account name (optional)')).not.toBeVisible();
  await page.getByRole('button', { name: 'Add account', exact: true }).click();
  await page.getByLabel('Account name (optional)').fill('Work');
  await page.getByLabel('Email address', { exact: true }).fill('work@example.test');
  await page.getByLabel('IMAP host').fill('imap.example.test');
  await page.getByLabel('SMTP host').fill('smtp.example.test');
  await page.getByLabel('Username').fill('work@example.test');
  await page.getByLabel('Password or app password').fill('provider-secret');
  await page.getByRole('button', { name: 'Save account', exact: true }).click();
  await expect(page.getByText('Account saved. Open Inbox to check the connection.')).toBeVisible();
  await page.getByRole('button', { name: 'Close settings' }).click();
  await chooseAccount(page, 'Work');
  await page.getByRole('button', { name: /^Drafts/ }).click();
  await expect(page.getByText('No drafts yet')).toBeVisible();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Draft editor' }).getByText('work@example.test', { exact: true })).toBeVisible();
  await page.getByLabel('Subject', { exact: true }).fill('Work only');
  page.once('dialog', dialog => dialog.dismiss());
  // On a phone the sidebar is hidden while composing, so switch through the command runner.
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Switch account: Demo account');
  await page.keyboard.press('Enter');
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Work only');
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Draft editor' }).getByRole('status')).toContainText('Draft saved');
  await page.getByRole('button', { name: 'Back to list' }).click();
  await chooseAccount(page, 'Demo account');
  await page.getByRole('button', { name: /^Drafts/ }).click();
  await expect(page.getByRole('button', { name: /Work only/ })).toHaveCount(0);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Lock credentials', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Credential protection' }).getByText('Credentials locked', { exact: true })).toBeVisible();
  await page.getByLabel('Master password', { exact: true }).fill('incorrect password');
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Incorrect master password', { timeout: 20000 });
  await page.getByLabel('Master password', { exact: true }).fill('correct horse battery staple');
  await page.getByRole('button', { name: 'Unlock', exact: true }).click();
  await expect(page.getByText('Password protection enabled', { exact: true })).toBeVisible({ timeout: 20000 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('late account responses cannot replace the selected inbox', async ({ page }) => {
  const setting = (name: string) => ({ name, address: `${name}@example.test`, protocol: 'imap', imap_host: 'imap.example.test', imap_port: 993, smtp_host: 'smtp.example.test', smtp_port: 587, username: name, session_url: '' });
  await page.route('**/api/v1/labels', route => route.fulfill({ json: {} }));
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: [{ id: 'demo', settings: null, connected: false, configured: false }, ...['a', 'b'].map(id => ({ id, settings: setting(id), connected: true, configured: true }))] }));
  await page.route('**/api/v1/security', route => route.fulfill({ json: { protected: false, locked: false, cleanup_pending: false } }));
  let release!: () => void;
  const gate = new Promise<void>(resolve => release = resolve);
  let started!: () => void;
  const pending = new Promise<void>(resolve => started = resolve);
  await page.route('**/api/v1/messages/refresh*', async route => {
    const id = route.request().headers()['x-colecto-account'];
    if (id === 'a') { started(); await gate; }
    await route.fulfill({ json: id === 'demo' ? [] : [{ id: 'same-id', sender: id, address: `${id}@example.test`, subject: `Inbox ${id}`, body: 'Preview', date: 'Today' }] });
  });
  await page.goto('/');
  await chooseAccount(page, 'a');
  await pending;
  await chooseAccount(page, 'b');
  await expect(page.getByRole('button', { name: /Inbox b/ })).toBeVisible();
  const late = page.waitForResponse(response => response.url().includes('/api/v1/messages/refresh') && response.request().headers()['x-colecto-account'] === 'a');
  release();
  await (await late).finished();
  await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
  await expect(page.getByRole('button', { name: /Inbox a/ })).toHaveCount(0);
  await expect(page.getByRole('button', { name: /Inbox b/ })).toBeVisible();
});

test('waits for restored account before allowing compose', async ({ page }) => {
  await page.route('**/api/v1/preferences', route => route.fulfill({ json: { mcp: false, account: 'remembered', workspace: null } }));
  let release!: () => void;
  const gate = new Promise<void>(resolve => release = resolve);
  await page.route('**/api/v1/labels', route => route.fulfill({ json: {} }));
  await page.route('**/api/v1/accounts', async route => {
    await gate;
    await route.fulfill({ json: [{ id: 'remembered', settings: { name: 'Remembered', address: 'remembered@example.test', protocol: 'imap', imap_host: '', imap_port: 993, smtp_host: '', smtp_port: 587, username: '', session_url: '' }, connected: true, configured: true }] });
  });
  await page.route('**/api/v1/messages/refresh*', route => route.fulfill({ json: [] }));
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Compose', exact: true })).toBeDisabled();
  release();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Draft editor' }).getByText('remembered@example.test', { exact: true })).toBeVisible();
});

test('extension controls expose TOML metadata and recover a disabled mail workspace', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Inbox', exact: true })).toBeVisible();
  page.on('dialog', dialog => dialog.accept());
  await openExtensions(page);
  const shell = page.getByRole('group', { name: 'Mail workspace' });
  await expect(shell.getByText('By Colecto · API 1')).toBeVisible();
  await shell.getByRole('checkbox').uncheck();
  await page.getByRole('button', { name: 'Back to mail', exact: true }).click();
  await expect(page.getByText('Mail workspace disabled. Open Extensions to restore it.')).toBeVisible();
  await openExtensions(page);
  await shell.getByRole('checkbox').check();
  await page.getByRole('button', { name: 'Back to mail', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Inbox', exact: true })).toBeVisible();
});

test('install consent enables all capabilities and settings keep them read-only', async ({ page }) => {
  await page.goto('/');
  await openExtensions(page);
  await page.getByLabel('Extension package', { exact: true }).setInputFiles('../target/extensions/connection-panel.colectoext');
  await page.getByRole('button', { name: 'Install', exact: true }).click();
  const warning = page.getByRole('dialog', { name: 'Only install extensions you trust' });
  await expect(warning.getByText('Read account connection status')).toBeVisible();
  await expect(warning.getByRole('button', { name: 'Install (5)' })).toBeDisabled();
  await warning.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByRole('group', { name: 'Connection panel', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Install', exact: true }).click();
  await warning.getByRole('button', { name: 'Install anyway' }).click({ timeout: 8000 });
  const panel = page.getByRole('group', { name: 'Connection panel', exact: true });
  await expect(panel.getByRole('checkbox', { name: 'Enabled for all accounts' })).toBeChecked();
  await expect(panel.getByText('Read account connection status')).toBeVisible();
  await expect(panel.getByRole('checkbox')).toHaveCount(2);
  page.on('dialog', dialog => dialog.accept());
  await panel.getByRole('button', { name: 'Remove extension', exact: true }).click();
});

test('Oneko follows the pointer and hops aside when touched', async ({ page }) => {
  await page.goto('/');
  await openExtensions(page);
  await page.getByLabel('Extension package', { exact: true }).setInputFiles('../docs/extensions/examples/oneko.colectoext');
  await page.getByRole('button', { name: 'Install', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Install anyway' }).click({ timeout: 8000 });
  await page.getByRole('button', { name: 'Close settings', exact: true }).click();
  const cat = page.locator('.sprite');
  await expect(cat).toBeVisible();
  await expect.poll(() => cat.locator('image').evaluate(node => (node as SVGGraphicsElement).getBBox().width)).toBeGreaterThan(0);
  await page.mouse.move(700, 400);
  await expect.poll(async () => { const box = await cat.boundingBox(); return box ? Math.max(Math.abs(box.x + 16 - 700), Math.abs(box.y + 16 - 400)) : 999; }, { timeout: 12000 }).toBeLessThan(46);
  const box = (await cat.boundingBox())!;
  await page.mouse.move(box.x + 16, box.y + 16);
  await expect.poll(async () => { const moved = (await cat.boundingBox())!; return moved.x > box.x + 32 || moved.x + 32 < box.x + 16 || moved.y > box.y + 32 || moved.y + 32 < box.y + 16; }).toBe(true);
  await openExtensions(page);
  page.on('dialog', dialog => dialog.accept());
  await page.getByRole('group', { name: 'Oneko', exact: true }).getByRole('button', { name: 'Remove extension' }).click();
});

test('owner generates a scoped key, sees its secret once, and revokes it on a phone', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'API keys', exact: true }).click();
  await page.getByLabel('Key name', { exact: true }).fill('Phone assistant');
  await page.getByRole('checkbox', { name: 'Demo account', exact: true }).check();
  await page.getByRole('checkbox', { name: 'List messages', exact: true }).check();
  await page.getByRole('button', { name: 'Generate API key', exact: true }).click();
  const secret = page.getByRole('textbox', { name: 'API key secret', exact: true });
  await expect(secret).toHaveValue(/^colecto_/);
  const issuedSecret = await secret.inputValue();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole('button', { name: 'Dismiss secret' }).click();
  await expect(secret).toHaveCount(0);
  await page.reload();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'API keys', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Phone assistant', exact: true })).toBeVisible();
  await expect(page.locator('body')).not.toContainText(issuedSecret);
  await expect(secret).toHaveCount(0);
  await page.getByRole('button', { name: 'Revoke Phone assistant', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('API key revoked.');
  await expect(page.getByRole('button', { name: 'Revoke Phone assistant', exact: true })).toHaveCount(0);
});

test('a rejected owner bearer preserves draft edits until the owner reconnects', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('Subject', { exact: true }).fill('Keep edits through owner reconnect');
  await page.route('**/api/v1/drafts', route => route.fulfill({ status: 401, json: { error: 'Owner authentication required' } }), { times: 1 });
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByLabel('Daemon access token')).toBeVisible();
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Keep edits through owner reconnect');
  await page.getByLabel('Daemon access token').fill(ownerToken);
  await page.getByRole('button', { name: 'Connect', exact: true }).click();
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Draft editor' }).getByRole('status')).toContainText('Draft saved');
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Keep edits through owner reconnect');
});

test('an ordinary browser needs the local owner token before it can connect', async ({ browser }) => {
  const page = await browser.newPage();
  try {
    await page.goto('http://127.0.0.1:4179/');
    await expect(page.getByLabel('Daemon access token')).toBeVisible();
    await page.getByLabel('Daemon access token').fill('wrong-owner-token');
    await page.getByRole('button', { name: 'Connect', exact: true }).click();
    await expect(page.getByLabel('Daemon access token')).toBeVisible();
    await page.getByLabel('Daemon access token').fill(ownerToken);
    await page.getByRole('button', { name: 'Connect', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Inbox', exact: true })).toBeVisible();
    await expect(page.getByLabel('Daemon access token')).toHaveCount(0);
  } finally { await page.close(); }
});

test('another loopback port receives no owner credential after legitimate owner login', async ({ page }) => {
  const { createServer } = await import('node:http');
  let captured: import('node:http').IncomingHttpHeaders = {};
  const server = createServer((request, response) => {
    if (request.url === '/api/capture') captured = request.headers;
    response.writeHead(200, { 'content-type': 'text/html' });
    response.end('<!doctype html><title>Other local service</title>Other local service');
  });
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    await page.goto('/');
    await expect(page.getByRole('heading', { name: 'Inbox', exact: true })).toBeVisible();
    const port = (server.address() as import('node:net').AddressInfo).port;
    await page.goto(`http://127.0.0.1:${port}/`);
    await page.evaluate(async () => { await fetch('/api/capture'); });
    expect(Boolean(captured.authorization)).toBe(false);
    expect(Boolean(captured.cookie?.includes('colecto_owner='))).toBe(false);
    expect(await page.evaluate(() => sessionStorage.getItem('colecto.token'))).toBeNull();
    const replay = await page.request.get('http://127.0.0.1:4179/api/v1/accounts', { headers: { origin: 'http://127.0.0.1:4179', 'x-colecto-extension': 'host', ...(captured.cookie ? { cookie: captured.cookie } : {}) } });
    expect(replay.status()).toBe(401);
  } finally {
    server.closeAllConnections();
    await new Promise<void>((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
  }
});

test('owner reviews the complete immutable automation snapshot on a phone and sees uncertain delivery', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const request = { id: 'review-1', requester: 'assistant-key', account: 'demo', from: 'owner@example.test', created_at: 1, expires_at: Math.floor(Date.now() / 1000) + 600, status: 'pending', snapshot: { id: null, recipient: 'recipient@example.test', subject: 'Review exact content', body: '<script>untrusted content</script>\nOriginal message body' } };
  await page.route('**/api/v1/send-requests', route => route.fulfill({ json: [request] }));
  await page.route('**/api/v1/api-keys', route => route.fulfill({ json: [{ id: 'assistant-key', name: 'Research assistant' }] }));
  let approvals = 0;
  await page.route('**/api/v1/send-requests/review-1/approve', route => { approvals++; return route.fulfill({ json: { ...request, status: 'uncertain' } }); });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Send approvals', exact: true }).click();
  const review = page.getByRole('article', { name: 'Delivery request: Review exact content' });
  await expect(review).toContainText('Research assistant');
  await expect(review).toContainText('owner@example.test');
  await expect(review).toContainText('recipient@example.test');
  await expect(review).toContainText('<script>untrusted content</script>');
  await expect(review).toContainText('Original message body');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await review.getByRole('button', { name: 'Approve and send', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Delivery outcome is uncertain');
  await expect(review.getByRole('button', { name: 'Approve and send', exact: true })).toHaveCount(0);
  expect(approvals).toBe(1);
});


test('notifications can be disabled while the mail workspace keeps working', async ({ page }) => {
  await page.goto('/');
  await openExtensions(page);
  const toggle = page.getByRole('group', { name: 'Notifications', exact: true }).getByRole('checkbox', { name: 'Enabled for all accounts' });
  await toggle.uncheck();
  await page.getByRole('button', { name: 'Close settings', exact: true }).click();
  await expect(page.locator('section.toasts')).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Inbox', exact: true })).toBeVisible();
  await openExtensions(page);
  await toggle.check();
  await page.getByRole('button', { name: 'Close settings', exact: true }).click();
  await expect(page.locator('section.toasts')).toHaveCount(1);
});

test('pending attachment reads reserve limits and block sending, and failures remain visible', async ({ page }) => {
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: [{ id: 'demo', settings: null, connected: true, configured: false }] }));
  await page.addInitScript(() => {
    const readers: FileReader[] = [];
    (window as any).attachmentReaders = readers;
    window.FileReader = class {
      result = 'data:text/plain;base64,YQ==';
      onload: any; onerror: any; onabort: any;
      readAsDataURL() { readers.push(this as unknown as FileReader); }
    } as any;
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  const input = page.locator('input[type=file]');
  await page.evaluate(() => Object.defineProperty(File.prototype, 'size', { configurable: true, get: () => 14 * 1024 * 1024 }));
  await input.setInputFiles({ name: 'large.txt', mimeType: 'text/plain', buffer: Buffer.from('a') });
  await input.setInputFiles({ name: 'also-large.txt', mimeType: 'text/plain', buffer: Buffer.from('a') });
  await expect(page.getByRole('alert')).toContainText('25 MB');
  await page.evaluate(() => {
    delete (File.prototype as any).size;
    (window as any).attachmentReaders[0].onload();
  });
  await page.getByRole('button', { name: 'Remove large.txt', exact: true }).click();
  await page.evaluate(() => { (window as any).attachmentReaders.splice(0); });
  await input.setInputFiles(Array.from({ length: 20 }, (_, i) => ({ name: `${i}.txt`, mimeType: 'text/plain', buffer: Buffer.from('a') })));
  const send = page.getByRole('button', { name: 'Send', exact: true });
  await expect(send).toBeDisabled();
  await expect(page.getByText('Reading attachments… Send will be available when all files are ready.')).toBeVisible();
  await input.setInputFiles({ name: 'extra.txt', mimeType: 'text/plain', buffer: Buffer.from('a') });
  await expect(page.getByRole('alert')).toContainText('at most 20');
  await page.evaluate(() => {
    const readers = (window as any).attachmentReaders;
    readers[0].onerror();
    readers.slice(1).forEach((reader: any) => reader.onload());
  });
  await expect(page.getByRole('alert')).toContainText('Could not read 0.txt');
  await expect(send).toBeDisabled();
  await page.getByRole('button', { name: 'Remove all attachments' }).click();
  await expect(send).toBeEnabled();
  await input.setInputFiles({ name: 'ready.txt', mimeType: 'text/plain', buffer: Buffer.from('a') });
  await page.evaluate(() => (window as any).attachmentReaders.at(-1).onload());
  await expect(send).toBeEnabled();
  await expect(page.getByRole('list', { name: 'Files to send' })).toContainText('ready.txt');
  let payload: any;
  await page.route('**/api/v1/send', route => { payload = route.request().postDataJSON(); return route.fulfill({ json: true }); });
  await send.click();
  await expect(page.getByRole('region', { name: 'Draft editor' }).getByRole('button', { name: 'Sent', exact: true })).toBeVisible();
  expect(payload.attachments).toEqual([{ name: 'ready.txt', content_type: 'text/plain', data: 'YQ==' }]);
});
