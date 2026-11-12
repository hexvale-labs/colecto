import { test, expect } from './fixtures';

test('connection controls handle unavailable services and long socket paths on a phone', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const socket = '/run/user/1000/colecto/personal-mail-background-service/mail.sock';
  await page.route('**/api/v1/upstream', route => route.fulfill({ json: {address: `unix:${socket}`, token_file: '/home/aleksandra/.local/state/colecto/owner-token'} }));
  await page.route('**/api/v1/services', route => route.fulfill({ json: {
    daemon: {installed: true, active: false, enabled: true, supported: true, socket},
    notifications: {installed: false, active: false, enabled: false, supported: true, socket},
  } }));
  await page.goto('/');
  await page.getByRole('button', {name: 'Settings', exact: true}).click();
  await page.getByRole('button', {name: 'Connection', exact: true}).click();
  await expect(page.getByText('Stopped', {exact: true})).toBeVisible();
  await expect(page.getByText('Not installed', {exact: true})).toBeVisible();
  await expect(page.getByRole('switch', {name: 'Start on login'})).toBeChecked();
  await expect(page.getByLabel('Server address')).toHaveValue(`unix:${socket}`);
  expect(await page.locator('.connection-settings').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  await page.route('**/api/v1/services/daemon', route => route.fulfill({status: 400, json: {error: 'This unit was not written by Colecto'}}));
  await page.getByRole('button', {name: 'Start daemon', exact: true}).click();
  await expect(page.getByRole('alert')).toContainText('not written by Colecto');
});
