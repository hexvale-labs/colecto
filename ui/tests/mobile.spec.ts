import { devices } from '@playwright/test';
import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';

const messages = [
  { id: 'maya', sender: 'Maya Chen', address: 'maya@example.test', subject: 'A small plan for the weekend', body: 'Coffee?', date: 'Today' },
  { id: 'alex', sender: 'Alex Rivera', address: 'alex@example.test', subject: 'Plain text', body: 'Hello', date: 'Today' },
];
async function inbox(page: Page) {
  await page.route('**/api/v1/messages**', route => {
    const url = new URL(route.request().url());
    if (/\/(cache|content)$/.test(url.pathname)) return route.fallback();
    const id = url.pathname.split('/').at(-1);
    return route.fulfill({ json: id === 'refresh' ? messages : messages.find(message => message.id === id) });
  });
}
// Same-document history steps, like the Android back button inside the app.
const back = (page: Page) => page.evaluate(() => history.back());

test('back closes the newest screen first and only then leaves the app', async ({ page }) => {
  await inbox(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  const base = await page.evaluate(() => history.length);
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await expect(page.getByRole('heading', { name: 'A small plan for the weekend' })).toBeVisible();
  await back(page);
  await expect(page.getByRole('button', { name: /Alex Rivera/ })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'A small plan for the weekend' })).not.toBeVisible();

  // Settings → a section → back to the list → back to the inbox.
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const settings = page.getByRole('dialog', { name: 'Settings' });
  await settings.getByRole('button', { name: 'Appearance', exact: true }).click();
  await expect(settings.getByRole('heading', { name: 'Appearance' })).toBeVisible();
  await back(page);
  await expect(settings.getByRole('button', { name: 'Appearance', exact: true })).toBeVisible();
  await back(page);
  await expect(settings).not.toBeVisible();
  // Every screen gave its history entry back, so the next back would leave the app.
  await expect.poll(() => page.evaluate(() => history.state)).toBeNull();

  // The in-app back arrow takes the same path as the system back.
  await page.getByRole('button', { name: /Alex Rivera/ }).click();
  await page.getByRole('button', { name: 'Back to list' }).click();
  await expect(page.getByRole('button', { name: /Maya Chen/ })).toBeVisible();
  await expect.poll(() => page.evaluate(() => history.state)).toBeNull();
  expect(await page.evaluate(() => history.length)).toBeGreaterThanOrEqual(base);
});

test('a message opened right after going back stays open', async ({ page }) => {
  await inbox(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await page.getByRole('button', { name: 'Back to list' }).click();
  // The list shows while the message slides away; the tap lands once it has gone.
  await page.getByRole('button', { name: /Alex Rivera/ }).click();
  await expect(page.getByRole('heading', { name: 'Plain text' })).toBeVisible();
  await page.waitForTimeout(400);
  await expect(page.getByRole('heading', { name: 'Plain text' })).toBeVisible();
  await expect.poll(() => page.evaluate(() => history.state)).toEqual({ colectoBack: 1 });
});

test('back closes dialogs and listboxes before the page under them', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const settings = page.getByRole('dialog', { name: 'Settings' });
  await settings.getByRole('button', { name: 'Appearance', exact: true }).click();
  await page.getByRole('button', { name: 'Interface font' }).click();
  await expect(page.getByRole('listbox', { name: 'Interface font' })).toBeVisible();
  await back(page);
  await expect(page.getByRole('listbox', { name: 'Interface font' })).not.toBeVisible();
  await expect(settings).toBeVisible();
  await page.keyboard.press('Control+k');
  await expect(page.getByRole('dialog', { name: 'Commands' })).toBeVisible();
  await back(page);
  await expect(page.getByRole('dialog', { name: 'Commands' })).not.toBeVisible();
  await expect(settings).toBeVisible();
  await back(page);
  await expect(settings).not.toBeVisible();
});

test('the settings section list collapses to icons and remembers it', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const settings = page.getByRole('dialog', { name: 'Settings' });
  await settings.getByRole('button', { name: 'Collapse the section list' }).click();
  await expect(settings.getByRole('button', { name: 'Expand the section list' })).toHaveAttribute('aria-expanded', 'false');
  await expect(settings.locator('.nav-item .nav-label').first()).toBeHidden();
  // Still reachable by name, with a tooltip for pointer users.
  await settings.getByRole('button', { name: 'Appearance', exact: true }).click();
  await expect(settings.getByRole('heading', { name: 'Appearance' })).toBeVisible();
  await page.reload();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(settings.getByRole('button', { name: 'Expand the section list' })).toBeVisible();
  await settings.getByRole('button', { name: 'Expand the section list' }).click();
  await expect(settings.locator('.nav-item .nav-label').first()).toBeVisible();
});

test('medium windows put folders in a side rail and show one pane', async ({ page }) => {
  await inbox(page);
  await page.setViewportSize({ width: 700, height: 900 });
  await page.goto('/');
  const rail = page.getByRole('navigation', { name: 'Mail folders' });
  await expect(rail).toBeVisible();
  const box = (await rail.boundingBox())!;
  expect(box.x).toBe(0);
  expect(box.width).toBe(80);
  expect(box.height).toBeGreaterThan(600);
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await expect(page.getByRole('heading', { name: 'A small plan for the weekend' })).toBeVisible();
  await expect(page.getByRole('button', { name: /Alex Rivera/ })).not.toBeVisible();
  await expect(rail).toBeVisible();
});

test.describe('on a touch screen', () => {
  const { defaultBrowserType: _, ...pixel } = devices['Pixel 7'];
  test.use(pixel);

  test('text reads one size up and keyboard hints are hidden', async ({ page }) => {
    await page.goto('/');
    await expect.poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue('--text-scale'))).toBe(String(18 / 14));
    await page.getByRole('button', { name: 'Commands' }).tap();
    const runner = page.getByRole('dialog', { name: 'Commands' });
    await expect(runner).toBeVisible();
    await expect(runner.locator('kbd').first()).toBeHidden();
    await expect(runner.locator('footer')).toBeHidden();
  });

  test('a tap leaves no hover highlight behind', async ({ page }) => {
    await inbox(page);
    await page.goto('/');
    const refresh = page.getByRole('button', { name: 'Refresh', exact: true });
    await refresh.tap();
    expect(await refresh.evaluate(element => getComputedStyle(element).backgroundColor)).toBe('rgba(0, 0, 0, 0)');
  });
});

// Regression: in the Android app a finger never moved a slider. Synthetic touch pointer events
// get no native range handling in any engine, so this checks the slider's own touch path.
test('sliders follow a finger and ignore a swipe that becomes a scroll', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('dialog', { name: 'Settings' }).getByRole('button', { name: 'Mail', exact: true }).click();
  const slider = page.getByRole('slider', { name: 'Messages to keep ready' });
  await expect(slider).toBeEnabled();
  const touch = (type: string, ratio: number) => slider.evaluate((input, [type, ratio]) => {
    const box = input.getBoundingClientRect();
    input.dispatchEvent(new PointerEvent(type as string, { pointerId: 7, pointerType: 'touch', bubbles: true, clientX: box.left + box.width * (ratio as number), clientY: box.top + box.height / 2 }));
  }, [type, ratio] as const);
  const saved = page.waitForRequest(request => request.url().endsWith('/api/v1/preferences') && request.method() === 'POST' && 'prefetch' in (request.postDataJSON() ?? {}));
  await touch('pointerdown', 0.5);
  for (const ratio of [0.6, 0.75, 0.9, 1.2]) await touch('pointermove', ratio);
  await expect(slider).toHaveValue('100');
  await expect(page.locator('output[for="prefetch-count"]')).toHaveText('100');
  await touch('pointerup', 1.2);
  expect((await saved).postDataJSON().prefetch.count).toBe(100);
  // A tap jumps to where it lands.
  await touch('pointerdown', 0.2); await touch('pointerup', 0.2);
  await expect(slider).not.toHaveValue('100');
  const tapped = await slider.inputValue();
  // A vertical swipe: the browser takes it for scrolling and cancels the pointer.
  await touch('pointerdown', 0.9); await touch('pointercancel', 0.9);
  await expect(slider).toHaveValue(tapped);
});
