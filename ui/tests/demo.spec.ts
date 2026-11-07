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
