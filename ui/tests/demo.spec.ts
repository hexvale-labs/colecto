import { test, expect } from '@playwright/test';

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
  await expect(page.getByRole('status')).toContainText('Draft saved');
  await page.reload();
  await page.getByRole('button', { name: /^Drafts/ }).click();
  await page.getByRole('button', { name: /Coffee by the river/ }).click();
  await expect(page.getByLabel('Message', { exact: true })).toHaveValue('Saturday sounds lovely. 世界');
  await page.getByLabel('Subject', { exact: true }).fill('Sunday by the river');
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Draft saved');
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
  await page.route('**/api/v1/messages', route => route.fulfill({ status: 503, json: { error: 'Mailbox unavailable' } }), { times: 1 });
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
    headers: { origin: 'http://127.0.0.1:4179', 'x-colecto-extension': 'composer', 'x-colecto-account': 'demo' },
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
  await expect(page.getByRole('button', { name: 'Sent', exact: true })).toBeDisabled();
  expect(sends).toBe(2);
});

test('manages encrypted accounts, switches drafts and protects unsaved changes', async ({ page }) => {
  test.setTimeout(90000);
  await page.route('**/api/v1/messages', route => route.request().headers()['x-colecto-account'] !== 'demo' ? route.fulfill({ json: [] }) : route.continue());
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Manage accounts', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Accounts & security' })).toBeVisible();
  await page.getByLabel('Master password', { exact: true }).fill('correct horse battery staple');
  await page.getByLabel('Confirm master password').fill('correct horse battery staple');
  await page.getByRole('button', { name: 'Enable password protection' }).click();
  await expect(page.getByText('Password protection enabled', { exact: true })).toBeVisible({ timeout: 20000 });
  await page.getByLabel('Account name').fill('Work');
  await page.getByLabel('Email address', { exact: true }).fill('work@example.test');
  await page.getByLabel('IMAP host').fill('imap.example.test');
  await page.getByLabel('SMTP host').fill('smtp.example.test');
  await page.getByLabel('Username').fill('work@example.test');
  await page.getByLabel('Password or app password').fill('provider-secret');
  await page.getByRole('button', { name: 'Save account', exact: true }).click();
  await expect(page.getByText('Account saved. Open Inbox to check the connection.')).toBeVisible();
  await page.getByRole('button', { name: 'Back to mail' }).click();
  await page.getByLabel('Current account').selectOption({ label: 'Work · work@example.test' });
  await page.getByRole('button', { name: /^Drafts/ }).click();
  await expect(page.getByText('No drafts yet')).toBeVisible();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await expect(page.getByText('work@example.test', { exact: true })).toBeVisible();
  await page.getByLabel('Subject', { exact: true }).fill('Work only');
  page.once('dialog', dialog => dialog.dismiss());
  await page.getByLabel('Current account').selectOption('demo');
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Work only');
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Draft saved');
  await page.getByLabel('Current account').selectOption('demo');
  await page.getByRole('button', { name: /^Drafts/ }).click();
  await expect(page.getByRole('button', { name: /Work only/ })).toHaveCount(0);
  await page.getByRole('button', { name: 'Manage accounts', exact: true }).click();
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
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: [{ id: 'demo', settings: null, connected: false, configured: false }, ...['a', 'b'].map(id => ({ id, settings: setting(id), connected: true, configured: true }))] }));
  await page.route('**/api/v1/security', route => route.fulfill({ json: { protected: false, locked: false, cleanup_pending: false } }));
  let release!: () => void;
  const gate = new Promise<void>(resolve => release = resolve);
  let started!: () => void;
  const pending = new Promise<void>(resolve => started = resolve);
  await page.route('**/api/v1/messages', async route => {
    const id = route.request().headers()['x-colecto-account'];
    if (id === 'a') { started(); await gate; }
    await route.fulfill({ json: id === 'demo' ? [] : [{ id: 'same-id', sender: id, address: `${id}@example.test`, subject: `Inbox ${id}`, body: 'Preview', date: 'Today' }] });
  });
  await page.goto('/');
  await page.getByLabel('Current account').selectOption('a');
  await pending;
  await page.getByLabel('Current account').selectOption('b');
  await expect(page.getByRole('button', { name: /Inbox b/ })).toBeVisible();
  const late = page.waitForResponse(response => response.url().endsWith('/api/v1/messages') && response.request().headers()['x-colecto-account'] === 'a');
  release();
  await (await late).finished();
  await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
  await expect(page.getByRole('button', { name: /Inbox a/ })).toHaveCount(0);
  await expect(page.getByRole('button', { name: /Inbox b/ })).toBeVisible();
});

test('waits for restored account before allowing compose', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('colecto.account', 'remembered'));
  let release!: () => void;
  const gate = new Promise<void>(resolve => release = resolve);
  await page.route('**/api/v1/accounts', async route => {
    await gate;
    await route.fulfill({ json: [{ id: 'remembered', settings: { name: 'Remembered', address: 'remembered@example.test', protocol: 'imap', imap_host: '', imap_port: 993, smtp_host: '', smtp_port: 587, username: '', session_url: '' }, connected: true, configured: true }] });
  });
  await page.route('**/api/v1/messages', route => route.fulfill({ json: [] }));
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Compose', exact: true })).toBeDisabled();
  release();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await expect(page.getByText('remembered@example.test', { exact: true })).toBeVisible();
});
