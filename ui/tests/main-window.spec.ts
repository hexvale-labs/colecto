import { test, expect } from './fixtures';

test('startup is painted for its minimum and reduced motion removes the fade', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.addInitScript(() => {
    sessionStorage.removeItem('colecto.fastStartup');
    let seen = false;
    new MutationObserver(() => {
      if (document.getElementById('startup')) seen = true;
      if (seen && !document.getElementById('startup')) {
        (window as Window & { removedAt?: number }).removedAt ??= performance.now();
      }
    }).observe(document, { childList: true, subtree: true });
  });
  await page.goto('/');
  await expect(page.locator('#startup')).toHaveCount(0);
  await page.waitForFunction(() => typeof (window as Window & { removedAt?: number }).removedAt === 'number');
  const elapsed = await page.evaluate(async () => {
    const state = window as unknown as Window & { colectoStartupPaint: Promise<number>; removedAt: number };
    return state.removedAt - await state.colectoStartupPaint;
  });
  expect(elapsed).toBeGreaterThanOrEqual(790);
  await expect(page.locator('#app')).not.toHaveAttribute('inert');
});

test('message tabs share the dock header and remain usable with extreme subjects', async ({ page }) => {
  const subject = '長い件名📨' + 'UnbrokenSubject'.repeat(30);
  await page.route('**/api/v1/messages**', route => {
    const url = new URL(route.request().url());
    if (url.pathname.endsWith('/cache')) return route.fallback();
    const messages = Array.from({ length: 8 }, (_, i) => ({ id: `stress-${i}`, sender: 'Long Sender '.repeat(10), address: 'long'.repeat(30) + '@example.test', subject: `${i} ${subject}`, body: 'Message body', date: 'Today' }));
    const id = url.pathname.split('/').at(-1);
    return route.fulfill({ json: id === 'refresh' ? messages : messages.find(message => message.id === id) });
  });
  await page.goto('/');
  for (let i = 0; i < 8; i++) await page.locator('.mail-item').nth(i).click();
  await expect(page.locator('.dock-head .message-tab')).toHaveCount(8);
  await expect(page.locator('.stage > .tab-strip')).toHaveCount(0);
  await page.keyboard.press('Tab');
  await page.locator('.tab-close').last().focus();
  expect(await page.locator('.tab-close').last().evaluate(el => getComputedStyle(el).outlineStyle)).toBe('solid');
  await page.screenshot({ path: '/tmp/colecto-main-review/after-many-tabs.png' });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator('.stage > .tab-strip')).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: '/tmp/colecto-main-review/after-compact.png' });
});

test('connected status stays green with another accent and stops moving under reduced motion', async ({ page }) => {
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: [{ id: 'demo', configured: true, connected: true, settings: { name: 'An account with a very long name '.repeat(8), address: 'long'.repeat(40) + '@example.test' } }] }));
  await page.goto('/');
  const dot = page.locator('.account-pane .status-dot');
  await expect(dot).toHaveClass(/on/);
  await page.evaluate(() => document.documentElement.style.setProperty('--mint', '#c499ed'));
  expect(await dot.evaluate(el => getComputedStyle(el).backgroundColor)).toBe('rgb(130, 201, 149)');
  expect(await dot.evaluate(el => getComputedStyle(el, '::after').animationDuration)).toBe('2.4s');
  await page.screenshot({ path: '/tmp/colecto-main-review/after-connected.png' });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  expect(await dot.evaluate(el => getComputedStyle(el, '::after').animationName)).toBe('none');
});
