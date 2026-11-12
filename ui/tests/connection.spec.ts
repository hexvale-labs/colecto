import { test, expect } from './fixtures';

test('connection controls handle service states and long socket paths on a phone', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const socket = '/run/user/1000/colecto/personal-mail-background-service/mail.sock';
  await page.route('**/api/v1/upstream', route => route.fulfill({ json: { upstream: { address: `unix:${socket}`, token_file: '/home/aleksandra/.local/state/colecto/owner-token' } } }));
  await page.route('**/api/v1/services', route => route.fulfill({ json: {
    daemon: { installed: true, active: false, enabled: true, supported: true, socket },
    notifications: { installed: false, active: false, enabled: false, supported: true, socket },
  } }));
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Connection', exact: true }).click();
  await expect(page.getByText('Background mail service')).toBeVisible();
  await expect(page.getByText(`unix:${socket}`)).toBeVisible();
  await expect(page.getByText('Stopped', { exact: true })).toBeVisible();
  await expect(page.getByText('Not installed', { exact: true })).toBeVisible();
  await expect(page.getByRole('switch', { name: 'Start Mail service on login' })).toBeChecked();
  // Not yet installed: the choice is kept for the install, defaulting to on.
  await expect(page.getByRole('switch', { name: 'Start Desktop notifications on login' })).toBeChecked();
  expect(await page.locator('.settings-page').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  await page.route('**/api/v1/services/daemon', route => route.fulfill({ status: 400, json: { error: 'This unit was not written by Colecto' } }));
  await page.getByRole('button', { name: 'Start Mail service', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('not written by Colecto');
});
