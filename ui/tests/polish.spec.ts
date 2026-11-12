import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';

/** Every scroll area ends at its content, and the window itself never scrolls. */
async function expectNoOverscroll(page: Page, where: string) {
  const problems = await page.evaluate(() => {
    const found: string[] = [];
    const root = document.scrollingElement!;
    if (root.scrollHeight > innerHeight + 1) found.push(`the window scrolls ${root.scrollHeight - innerHeight}px`);
    for (const element of document.querySelectorAll<HTMLElement>('*')) {
      const style = getComputedStyle(element);
      if (!/auto|scroll/.test(style.overflowY) || element.scrollHeight <= element.clientHeight + 1 || !element.checkVisibility()) continue;
      const top = element.getBoundingClientRect().top - element.scrollTop;
      let bottom = 0;
      for (const child of element.querySelectorAll<HTMLElement>('*')) {
        const box = child.getBoundingClientRect();
        if (box.width > 1 && box.height > 1 && child.checkVisibility()) bottom = Math.max(bottom, box.bottom - top);
      }
      const empty = element.scrollHeight - bottom - parseFloat(style.paddingBottom);
      if (empty > 24) found.push(`${element.className || element.tagName} scrolls ${Math.round(empty)}px past its content`);
    }
    return found;
  });
  expect(problems, where).toEqual([]);
}

/** Serve a fixed inbox, so tests do not depend on what earlier tests left in the shared Host. */
async function inbox(page: Page, messages: { id: string; sender: string; address: string; subject: string; body: string; date: string }[]) {
  await page.route('**/api/v1/messages**', route => {
    const url = new URL(route.request().url());
    if (/\/(cache|content)$/.test(url.pathname)) return route.fallback();
    return route.fulfill({ json: url.pathname.endsWith('/refresh') ? messages : messages.find(message => url.pathname.endsWith(`/${message.id}`)) });
  });
}
const pair = [
  { id: 'alex', sender: 'Alex Rivera', address: 'alex@example.test', subject: 'Plain text', body: 'Hello there', date: 'Today' },
  { id: 'maya', sender: 'Maya Chen', address: 'maya@example.test', subject: 'A small plan', body: 'Coffee?', date: 'Today' },
];
const long = { id: 'long', sender: 'Maya Chen', address: 'maya@example.test', subject: 'A long letter', body: 'Paragraph of text.\n'.repeat(120), date: 'Today' };

for (const viewport of [{ width: 1280, height: 720 }, { width: 390, height: 844 }]) {
  test(`no pane or settings page scrolls past its content at ${viewport.width}px`, async ({ page }) => {
    await page.route('**/api/v1/messages**', route => {
      const url = new URL(route.request().url());
      if (/\/(cache|content)$/.test(url.pathname)) return route.fallback();
      return route.fulfill({ json: url.pathname.endsWith('/refresh') ? Array.from({ length: 40 }, (_, i) => ({ ...long, id: `m${i}`, subject: `Message ${i}` })) : { ...long, id: url.pathname.split('/').at(-1) } });
    });
    await page.setViewportSize(viewport);
    await page.goto('/');
    await expect(page.getByRole('button', { name: /Message 39/ })).toBeAttached();
    const scrollAll = () => page.evaluate(() => document.querySelectorAll('*').forEach(element => { element.scrollTop = element.scrollHeight; }));
    // Mailbox, reader, composer and account switcher: every bundled extension's surface.
    await scrollAll(); await expectNoOverscroll(page, 'mailbox');
    await page.getByRole('button', { name: /Message 3\b/ }).click();
    await expect(page.locator('.letter-body')).toBeVisible();
    await scrollAll(); await expectNoOverscroll(page, 'reader');
    if (viewport.width < 800) await page.getByRole('button', { name: 'Back to list' }).click();
    await page.getByRole('button', { name: 'Compose', exact: true }).click();
    await scrollAll(); await expectNoOverscroll(page, 'composer');
    if (viewport.width < 800) await page.getByRole('button', { name: 'Back to list' }).click();
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.route('**/api/v1/services', route => route.fulfill({json: {daemon: {supported: false}, notifications: {supported: false}}}));
    const settings = page.getByRole('dialog', { name: 'Settings' });
    for (const section of ['Accounts', 'Appearance', 'Notifications', 'Connection', 'Extensions', 'API keys', 'Send approvals']) {
      await settings.getByRole('button', { name: section, exact: true }).click();
      await expect(settings.getByRole('heading', { level: 2 }).first()).toBeVisible();
      await scrollAll(); await expectNoOverscroll(page, `settings: ${section}`);
    }
  });
}

test('feels like an app: chrome is not selectable, mail is, and images do not drag', async ({ page }) => {
  await inbox(page, pair);
  await page.goto('/');
  await page.getByRole('button', { name: /Alex Rivera/ }).click();
  await expect(page.locator('.letter-body')).toBeVisible();
  const select = (selector: string) => page.locator(selector).first().evaluate(element => getComputedStyle(element).userSelect);
  expect(await select('.list-footer')).toBe('none');
  expect(await select('.dock-tab')).toBe('none');
  expect(await select('.letter-body')).toBe('text');
  expect(await select('#mail-search')).not.toBe('none');
  const dragged = await page.locator('.brand-mark').evaluate(image => !image.dispatchEvent(new DragEvent('dragstart', { bubbles: true, cancelable: true })));
  expect(dragged).toBe(true);
  // The reader can copy the message text.
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.getByRole('button', { name: 'Copy message text' }).click();
  await expect(page.getByRole('button', { name: 'Copy message text' })).toContainText('Copied');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toContain('Hello');
});

/** Serve one burst of Host events, then nothing, so each test controls exactly what arrives. */
async function events(page: Page, list: [string, unknown][]) {
  let sent = false;
  await page.route('**/api/v1/events', route => {
    if (sent) return route.fulfill({ status: 204 });
    sent = true;
    return route.fulfill({ status: 200, contentType: 'text/event-stream', body: list.map(([name, data]) => `event: ${name}\ndata: ${JSON.stringify(data)}\n\n`).join('') });
  });
}

test('approval requests arrive as notifications with decisions, a close button and a chosen position', async ({ page }) => {
  const decided: string[] = [];
  await page.route('**/api/v1/send-requests/*/*', route => { decided.push(route.request().url()); return route.fulfill({ json: { id: 'r1', status: 'rejected' } }); });
  await events(page, [['approval', { id: 'r1', account: 'demo', from: 'me@example.test', subject: 'Quarterly numbers' }], ['mail', { account: 'demo', ids: ['new-1', 'new-2'] }]]);
  await page.goto('/');
  const toasts = page.locator('section.toasts');
  const request = toasts.getByRole('alertdialog', { name: 'Automation wants to send a message' });
  await expect(request).toContainText('Quarterly numbers');
  await expect(toasts.getByRole('status', { name: '2 new messages' })).toBeVisible();
  await toasts.getByRole('status', { name: '2 new messages' }).getByRole('button', { name: 'Dismiss notification' }).click();
  await expect(toasts.getByRole('status')).toHaveCount(0);
  await expect(toasts).toHaveAttribute('data-position', 'top');
  // Settings covers the notification layer.
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.locator('.settings-overlay')).toBeVisible();
  expect(await page.locator('.settings-overlay').evaluate(el => Number(getComputedStyle(el).zIndex))).toBeGreaterThan(await page.locator('.extension-overlay').first().evaluate(el => Number(getComputedStyle(el).zIndex)));
  await page.getByRole('button', { name: 'Close settings', exact: true }).click();
  await expect(request.getByRole('button', { name: 'Approve and send' })).toHaveCount(0);
  const snapshot = { id: 'r1', requester: 'key', account: 'demo', from: 'me@example.test', status: 'pending', expires_at: 2000000000, snapshot: { recipient: 'unexpected@example.test', subject: 'Quarterly numbers', body: 'Sensitive body' } };
  await page.route('**/api/v1/send-requests', route => route.fulfill({ json: [snapshot] }));
  await request.getByRole('button', { name: 'Review message' }).focus();
  await page.keyboard.press('Enter');
  const card = page.getByRole('article', { name: 'Delivery request: Quarterly numbers' });
  await expect(card).toBeFocused();
  await expect(card).toContainText('unexpected@example.test');
  await expect(card).toContainText('Sensitive body');
  expect(decided).toEqual([]);
  await card.getByRole('button', { name: 'Reject' }).click();
  expect(decided).toEqual([expect.stringContaining('/send-requests/r1/reject')]);
  await page.getByRole('button', { name: 'Close settings', exact: true }).click();

  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const settings = page.getByRole('dialog', { name: 'Settings' });
  await settings.getByRole('button', { name: 'Notifications', exact: true }).click();
  await settings.getByRole('radio', { name: 'Top left' }).check();
  await expect(toasts).toHaveAttribute('data-position', 'top-left');
  await settings.getByRole('checkbox', { name: 'Send approvals' }).uncheck();
  // Both choices are the owner's preferences, kept by the Host.
  await page.unroute('**/api/v1/events');
  await events(page, [['approval', { id: 'r2', account: 'demo', from: 'me@example.test', subject: 'Muted' }]]);
  await page.reload();
  await expect(page.locator('section.toasts')).toHaveAttribute('data-position', 'top-left');
  await page.waitForTimeout(500);
  await expect(page.getByText('Muted')).toHaveCount(0);
});

test('automation can open any page, including settings, through the shared navigator', async ({ page }) => {
  await events(page, [['navigate', { target: 'settings/appearance', account: 'demo' }], ['navigate', { target: 'settings/../secret', account: 'demo' }]]);
  await page.goto('/');
  const settings = page.getByRole('dialog', { name: 'Settings' });
  await expect(settings.getByRole('heading', { name: 'Appearance', exact: true })).toBeVisible();
  // The command runner lists the same destinations.
  await page.keyboard.press('Control+k');
  await page.keyboard.type('settings notifications');
  await page.keyboard.press('Enter');
  await expect(settings.getByRole('heading', { name: 'Notifications', exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(settings).toHaveCount(0);
});

test('the accent colour can be chosen and is remembered', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('dialog', { name: 'Settings' }).getByRole('button', { name: 'Appearance', exact: true }).click();
  await page.getByRole('radio', { name: 'Sky' }).check();
  const accent = () => page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--mint').trim());
  expect(await accent()).toBe('#8ec5f0');
  await page.reload();
  await expect.poll(accent).toBe('#8ec5f0');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('dialog', { name: 'Settings' }).getByRole('button', { name: 'Appearance', exact: true }).click();
  await page.getByRole('button', { name: 'Restore the default mint' }).click();
  await expect.poll(accent).toBe('#a2d6b1');
});

test('closes every message tab at once', async ({ page }) => {
  await inbox(page, pair);
  await page.goto('/');
  for (const sender of [/Alex Rivera/, /Maya Chen/]) await page.getByRole('button', { name: sender }).click();
  const strip = page.getByRole('navigation', { name: 'Open messages' });
  await expect(strip.getByRole('button', { name: /^Close (?!all)/ })).toHaveCount(2);
  await strip.getByRole('button', { name: 'Close all message tabs' }).click();
  await expect(strip).toHaveCount(0);
  await expect(page.getByText('Pick a message and it will open here.')).toBeVisible();
});
