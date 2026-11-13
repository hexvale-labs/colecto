import { readFileSync } from 'node:fs';
import { test, expect, ownerToken } from './fixtures';
import type { Page } from '@playwright/test';

const origin = `http://127.0.0.1:${process.env.COLECTO_TEST_PORT ?? '4179'}`;
const host = { authorization: `Bearer ${ownerToken}`, origin, 'x-colecto-extension': 'host', 'x-colecto-account': 'demo' };
const savePreferences = (page: Page, data: Record<string, unknown>) => page.request.post(`${origin}/api/v1/preferences`, { headers: host, data });
const defaultNotifications = { enabled: true, mail: true, approvals: true, system: false, position: 'top', updates: true };

async function openSettings(page: Page, section: string) {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const settings = page.getByRole('dialog', { name: 'Settings' });
  await settings.getByRole('button', { name: section, exact: true }).click();
  return settings;
}

/** Serve one burst of Host events, then nothing. */
async function events(page: Page, list: [string, unknown][]) {
  let sent = false;
  await page.route('**/api/v1/events', async route => {
    if (sent) return route.fulfill({ status: 204 });
    sent = true;
    await expect(page.getByRole('button', { name: 'Settings', exact: true })).toBeEnabled();
    return route.fulfill({ status: 200, contentType: 'text/event-stream', body: list.map(([name, data]) => `event: ${name}\ndata: ${JSON.stringify(data)}\n\n`).join('') });
  });
}

/** Whether the element's centre is the topmost thing there, so it is visible and clickable. */
const onTop = (page: Page, selector: string) => page.locator(selector).first().evaluate(element => {
  const box = element.getBoundingClientRect();
  return element.contains(document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2));
});

test('notifications show above Settings, with their actions starting from the left', async ({ page }) => {
  await events(page, [['approval', { id: 'r9', account: 'demo', from: 'me@example.test', subject: 'Over settings' }]]);
  await page.goto('/');
  const request = page.getByRole('alertdialog', { name: 'Automation wants to send a message' });
  await expect(request).toBeVisible();
  const settings = await openSettings(page, 'Notifications');
  await expect(settings.getByRole('heading', { name: 'Notifications', level: 2 })).toBeVisible();
  expect(await onTop(page, '.toast')).toBe(true);
  // A notification that arrives while Settings is open is on top too.
  await settings.getByRole('button', { name: 'Show a test notification' }).click();
  await expect(page.getByRole('status', { name: 'This is where notifications appear' })).toBeVisible();
  expect(await onTop(page, '.toast:has-text("This is where notifications appear") .toast-close')).toBe(true);
  // Actions line up with the toast's left edge, not with the text after the icon.
  const toast = (await request.boundingBox())!;
  const first = (await request.getByRole('button', { name: 'Review message' }).boundingBox())!;
  const mark = (await request.locator('.toast-mark').boundingBox())!;
  expect(Math.abs(first.x - mark.x)).toBeLessThan(1);
  expect(first.x - toast.x).toBeLessThan(16);
});

/** Dismiss the toast at `index` (oldest first) and record every toast's top edge each frame. */
const dismissAndWatch = (page: Page, index: number) => page.evaluate(async (index) => {
  const toasts = [...document.querySelectorAll<HTMLElement>('.toasts .toast')];
  const tops = () => toasts.map(toast => toast.isConnected ? toast.getBoundingClientRect().top : null);
  const frames = [tops()];
  toasts[index].querySelector<HTMLButtonElement>('.toast-close')!.click();
  const start = performance.now();
  while (performance.now() - start < 700) { await new Promise(requestAnimationFrame); frames.push(tops()); }
  return toasts.map((_, which) => frames.map(frame => frame[which]).filter((top): top is number => top !== null));
}, index);

test('a leaving notification fades in place while the others slide, without jumping', async ({ page }) => {
  // Oldest first in the DOM; at the top of the window the newest shows first.
  await events(page, [['mail', { account: 'demo', ids: ['n-1'] }], ['approval', { id: 'r8', account: 'demo', from: 'me@example.test', subject: 'Stays' }], ['mail', { account: 'demo', ids: ['n-2'] }]]);
  await page.goto('/');
  await expect(page.locator('.toasts .toast')).toHaveCount(3);
  await page.waitForTimeout(400);
  const range = (tops: number[]) => Math.max(...tops) - Math.min(...tops);
  // The oldest, lowest toast leaves first, as it does when it times out. It fades where it is
  // (drifting at most its 10 px exit) and nothing else moves.
  const [oldest, approval, newest] = await dismissAndWatch(page, 0);
  expect(range(oldest)).toBeLessThan(12);
  expect(range(approval)).toBeLessThan(1);
  expect(range(newest)).toBeLessThan(1);
  await expect(page.locator('.toasts .toast')).toHaveCount(2);
  // When the top one leaves, the one below slides up over several frames, never in one snap.
  const [stays] = await dismissAndWatch(page, 1).then(([stays, leaving]) => { expect(range(leaving)).toBeLessThan(12); return [stays]; });
  const travel = Math.abs(stays.at(-1)! - stays[0]);
  expect(travel).toBeGreaterThan(20);
  const steps = stays.slice(1).map((top, index) => Math.abs(top - stays[index]));
  expect(Math.max(...steps)).toBeLessThan(travel * 0.75);
  expect(steps.filter(step => step > 0.5).length).toBeGreaterThan(3);
});

test('a .colectoext dropped on the Extensions page opens the install review', async ({ page }) => {
  await page.goto('/');
  const settings = await openSettings(page, 'Extensions');
  const drop = async (name: string, bytes: number[]) => {
    await settings.locator('.extension-settings').evaluate((section, { name, bytes }) => {
      const transfer = new DataTransfer();
      transfer.items.add(new File([new Uint8Array(bytes)], name));
      for (const type of ['dragenter', 'dragover', 'drop']) section.querySelector('.extension-badge')!.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer: transfer }));
    }, { name, bytes });
  };
  await drop('notes.txt', [104, 105]);
  await expect(settings.getByRole('alert')).toContainText('notes.txt is not an extension package');
  await expect(page.locator('dialog.install-warning')).toHaveCount(0);
  // A rejected file is not taken as the package.
  await expect(settings.getByRole('button', { name: 'Install', exact: true })).toBeDisabled();
  // Dropped anywhere on the page, not only on the drop zone.
  await drop('oneko.colectoext', [...readFileSync('../docs/extensions/examples/oneko.colectoext')]);
  const warning = page.getByRole('dialog', { name: 'Only install extensions you trust' });
  await expect(warning).toBeVisible();
  await expect(warning).toContainText('oneko.colectoext');
  await expect(warning.getByRole('button', { name: /Install/ })).toBeDisabled();
  await warning.getByRole('button', { name: 'Cancel' }).click();
  await expect(warning).toHaveCount(0);
  await expect(settings.getByRole('group', { name: 'Oneko', exact: true })).toHaveCount(0);
  await expect(settings.getByText('oneko.colectoext ·')).toBeVisible();
});

test('the message list has three densities and optional date headings', async ({ page }) => {
  await page.goto('/');
  const row = page.locator('.mail-item').first();
  await expect(row).toBeVisible();
  await expect(page.locator('.day').first()).toBeVisible();
  const height = async () => (await row.boundingBox())!.height;
  const full = await height();
  const settings = await openSettings(page, 'Appearance');
  await settings.locator('.segmented label', { hasText: 'Medium' }).click();
  await expect.poll(height).toBeLessThan(full);
  const medium = await height();
  await expect(row.locator('.item-preview')).toBeHidden();
  await settings.locator('.segmented label', { hasText: 'Compact' }).click();
  await expect.poll(height).toBeLessThan(medium);
  await settings.getByRole('checkbox', { name: 'Date headings' }).uncheck();
  await expect(page.locator('.day')).toHaveCount(0);
  // Both are owner preferences, kept by the Host.
  await page.reload();
  await expect(page.locator('.mailbox')).toHaveAttribute('data-density', 'compact');
  await expect(page.locator('.mail-item').first()).toBeVisible();
  await expect(page.locator('.day')).toHaveCount(0);
  const again = await openSettings(page, 'Appearance');
  await again.getByRole('checkbox', { name: 'Date headings' }).check();
  await again.locator('.segmented label', { hasText: 'Full' }).click();
  await expect(page.locator('.day').first()).toBeAttached();
  await expect.poll(height).toBe(full);
});

test('a newer release is announced once, and never checked when turned off', async ({ page }) => {
  const version = `${Date.now() % 100000}.0.0`;
  let checks = 0;
  await page.route('**/api/v1/releases/latest', route => { checks += 1; return route.fulfill({ json: { current: '0.3.1', latest: { version, url: `https://github.com/hexvale-labs/colecto/releases/tag/v${version}` } } }); });
  await page.addInitScript(() => { window.open = ((url: string) => { (window as unknown as { opened: string }).opened = url; return null; }) as typeof window.open; });
  try {
    await page.goto('/');
    const toast = page.getByRole('status', { name: `Colecto ${version} is available` });
    await expect(toast).toBeVisible();
    await expect(toast).toContainText('You are using 0.3.1.');
    await toast.getByRole('button', { name: 'View release' }).click();
    expect(await page.evaluate(() => (window as unknown as { opened: string }).opened)).toBe(`https://github.com/hexvale-labs/colecto/releases/tag/v${version}`);
    await expect(toast).toHaveCount(0);
    // The same version does not notify again.
    await page.reload();
    await expect.poll(() => checks).toBe(2);
    await page.waitForTimeout(300);
    await expect(page.getByRole('status', { name: `Colecto ${version} is available` })).toHaveCount(0);
    // With "New updates" off, the app never asks.
    const settings = await openSettings(page, 'Notifications');
    await settings.getByRole('checkbox', { name: 'New updates' }).uncheck();
    await expect.poll(async () => (await (await page.request.get(`${origin}/api/v1/preferences`, { headers: host })).json()).notifications.updates).toBe(false);
    checks = 0;
    await page.reload();
    await expect(page.getByRole('button', { name: 'Settings', exact: true })).toBeEnabled();
    await page.waitForTimeout(500);
    expect(checks).toBe(0);
  } finally {
    await savePreferences(page, { notifications: defaultNotifications });
  }
});
