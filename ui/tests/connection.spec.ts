import { test, expect } from './fixtures';

test('connection controls handle service states and long socket paths on a phone', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const socket = '/run/user/1000/colecto/personal-mail-background-service/mail.sock';
  await page.route('**/api/v1/upstream', route => route.fulfill({ json: { upstream: { address: `unix:${socket}`, token_file: '/home/aleksandra/.local/state/colecto/owner-token' } } }));
  await page.route('**/api/v1/services', route => route.fulfill({ json: {
    daemon: { installed: true, active: false, enabled: true, supported: true, socket, version: '0.2.0', update_to: '0.3.0' },
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
  await expect(page.getByText('v0.2.0', { exact: true })).toBeVisible();
  expect(await page.locator('.settings-page').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  await page.route('**/api/v1/services/daemon', route => route.fulfill({ status: 400, json: { error: 'This unit was not written by Colecto' } }));
  await page.getByRole('button', { name: 'Start Mail service', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('not written by Colecto');
  let sent: unknown;
  await page.route('**/api/v1/services/daemon', route => { sent = route.request().postDataJSON(); return route.fulfill({ json: true }); });
  await page.getByRole('button', { name: 'Update Mail service to version 0.3.0' }).click();
  await expect(page.getByRole('status')).toContainText('Mail service updated to version 0.3.0.');
  expect(sent).toEqual({ action: 'update' });
});

// Regression: a background service that could not restart after an update left the app on
// "Mail workspace disabled" with only an Extensions button, and no way back to Connection.
test('an unreachable mail service gets a recovery screen that reconnects on its own', async ({ page }) => {
  const socket = '/run/user/1000/colecto/mail.sock';
  const down = 'The background mail service is not running (No such file or directory (os error 2)). Start it in Settings → Connection.';
  let up = false;
  await page.route('**/api/v1/extensions', route => up ? route.fallback() : route.fulfill({ status: 502, json: { error: down } }));
  await page.route('**/api/v1/upstream', route => route.fulfill({ json: { upstream: { address: `unix:${socket}`, token_file: '/home/user/.local/state/colecto/owner-token' } } }));
  await page.route('**/api/v1/services', route => route.fulfill({ json: {
    daemon: { installed: true, active: false, enabled: true, supported: true, socket, version: '0.4.0', update_to: '0.4.1' },
    notifications: { installed: false, active: false, enabled: false, supported: true, socket },
  } }));
  let sent: unknown;
  await page.route('**/api/v1/services/daemon', route => { sent = route.request().postDataJSON(); up = true; return route.fulfill({ json: true }); });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Can’t reach your mail' })).toBeVisible();
  await expect(page.getByText('Mail workspace disabled')).toHaveCount(0);
  await expect(page.getByText('journalctl --user -u colecto-daemon')).toBeVisible();
  await page.getByRole('button', { name: 'Update the mail service' }).click();
  expect(sent).toEqual({ action: 'update' });
  // Once the service answers, the workspace opens without a manual reload.
  await expect(page.getByRole('button', { name: 'Settings', exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Can’t reach your mail' })).toHaveCount(0);
});

test('the recovery screen reconnects by itself when the service comes back', async ({ page }) => {
  let up = false;
  await page.route('**/api/v1/extensions', route => up ? route.fallback() : route.fulfill({ status: 502, json: { error: 'The background mail service did not answer in time.' } }));
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Can’t reach your mail' })).toBeVisible();
  up = true;
  await expect(page.getByRole('button', { name: 'Settings', exact: true })).toBeVisible({ timeout: 8000 });
});
