// Visual and accessibility parity for the move to the declarative extension model
// (docs/extensions/ui-model.md, "Acceptance for each migrated extension"). The baselines in
// parity.spec.ts-snapshots/ were recorded from the Svelte extensions before their migration;
// each migration must keep matching them, and any intentional difference is re-recorded in the
// same pull request and listed there.
//
// Screenshots depend on the machine's fonts and GPU, so they are compared only where
// they were recorded (`COLECTO_PARITY_SCREENSHOTS=1`); the accessibility snapshots run everywhere.
import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';

const screenshots = process.env.COLECTO_PARITY_SCREENSHOTS === '1';
const widths = { phone: { width: 390, height: 844 }, desktop: { width: 1280, height: 720 } } as const;
const schemes = ['light', 'dark'] as const;
// Message dates and the greeting depend on the clock.
const now = new Date('2026-11-14T10:30:00');

async function start(page: Page, size: keyof typeof widths, scheme: typeof schemes[number]) {
  await page.setViewportSize(widths[size]);
  await page.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });
  await page.clock.setFixedTime(now);
  // Keep the event stream quiet so no toast or refresh lands mid-capture, and hide what other
  // specs leave behind in the shared Host: accounts, installed extensions, keys, requests and
  // credential protection.
  await page.route('**/api/v1/events', route => route.fulfill({ status: 204 }));
  await page.route('**/api/v1/accounts', async route => {
    if (route.request().method() !== 'GET') return route.fallback();
    const response = await route.fetch();
    return route.fulfill({ response, json: (await response.json()).filter((account: { id: string }) => account.id === 'demo') });
  });
  await page.route('**/api/v1/extensions', async route => {
    if (route.request().method() !== 'GET') return route.fallback();
    const response = await route.fetch();
    return route.fulfill({ response, json: (await response.json()).filter((extension: { bundled: boolean }) => extension.bundled) });
  });
  await page.route('**/api/v1/api-keys', route => route.request().method() === 'GET' ? route.fulfill({ json: [] }) : route.fallback());
  await page.route('**/api/v1/send-requests', route => route.fulfill({ json: [] }));
  await page.route('**/api/v1/security', route => route.request().method() === 'GET' ? route.fulfill({ json: { protected: false, locked: false, cleanup_pending: false } }) : route.fallback());
  await page.goto('/');
  await expect(page.getByRole('button', { name: /Maya Chen/ })).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
}

async function compare(page: Page, name: string, locator = page.locator('body')) {
  await expect(locator).toMatchAriaSnapshot({ name: `${name}.aria.yml` });
  if (screenshots) await expect(page).toHaveScreenshot(`${name}.png`, { maxDiffPixelRatio: 0.002, animations: 'disabled', caret: 'hide', mask: [page.locator('.list-status')] });
}

const settingsSections = ['Accounts', 'Mail', 'Appearance', 'Notifications', 'Privacy', 'Connection', 'Extensions', 'AI', 'API keys', 'Send approvals'];

for (const size of Object.keys(widths) as (keyof typeof widths)[]) {
  for (const scheme of schemes) {
    test.describe(`${size} ${scheme}`, () => {
      const id = (name: string) => `${name}-${size}-${scheme}`;

      test('workspace', async ({ page }) => {
        await start(page, size, scheme);
        await compare(page, id('workspace'));
      });

      test('message', async ({ page }) => {
        await start(page, size, scheme);
        await page.getByRole('button', { name: /Maya Chen/ }).click();
        await expect(page.getByRole('heading', { name: 'A small plan for the weekend' })).toBeVisible();
        await compare(page, id('message'));
      });

      test('compose', async ({ page }) => {
        await start(page, size, scheme);
        await page.keyboard.press('c');
        await expect(page.getByRole('region', { name: 'Draft editor' })).toBeVisible();
        await compare(page, id('compose'));
      });

      test('command runner', async ({ page }) => {
        await start(page, size, scheme);
        await page.getByRole('button', { name: 'Commands' }).click();
        await expect(page.getByRole('dialog', { name: 'Commands' })).toBeVisible();
        await compare(page, id('commands'), page.getByRole('dialog', { name: 'Commands' }));
      });

      for (const section of settingsSections) {
        test(`settings ${section}`, async ({ page }) => {
          await start(page, size, scheme);
          await page.getByRole('button', { name: 'Settings', exact: true }).click();
          const settings = page.getByRole('dialog', { name: 'Settings' });
          await settings.getByRole('button', { name: section, exact: true }).click();
          await expect(settings.locator('.settings-crumb strong')).toHaveText(section);
          await expect(settings.getByText(/Loading/)).toHaveCount(0);
          await compare(page, id(`settings-${section.toLowerCase().replace(/ /g, '-')}`), settings);
        });
      }
    });
  }
}
