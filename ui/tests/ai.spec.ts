import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';

const settings = (enabled: boolean) => ({
  settings: { enabled, text: { base_url: 'http://127.0.0.1:9/v1', model: 'test', max_tokens: 512, effort: null }, decision: { base_url: 'https://openrouter.ai', model: 'typesafe/jev-1.14', threshold: 0.7 }, autolabel: false, labels: {} },
  keys: { text: true, decision: false },
});

/** Pretend AI is on and keep summaries in memory, so they survive a reload like the Host's do. */
async function aiOn(page: Page) {
  const summaries = new Map<string, string>();
  const replies: unknown[] = [];
  await page.route('**/api/v1/ai/settings', route => route.fulfill({ json: settings(true) }));
  await page.route('**/api/v1/ai/summaries/*', route => {
    const id = decodeURIComponent(new URL(route.request().url()).pathname.split('/').pop()!);
    const method = route.request().method();
    if (method === 'POST') summaries.set(id, 'Maya suggests a slow Saturday: coffee, a walk and the bookshop by the river.');
    if (method === 'DELETE') summaries.delete(id);
    return route.fulfill({ json: { summary: summaries.get(id) ?? null } });
  });
  await page.route('**/api/v1/ai/reply', route => {
    replies.push(route.request().postDataJSON());
    return route.fulfill({ json: { text: 'cant make it saturday lol, next time' } });
  });
  return { summaries, replies };
}

async function openWeekend(page: Page) {
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await expect(page.getByRole('heading', { name: 'A small plan for the weekend' })).toBeVisible();
}

test('the reader shows no AI button while AI features are off', async ({ page }) => {
  // The real Host starts with AI off.
  await page.goto('/');
  await openWeekend(page);
  await expect(page.getByRole('button', { name: 'Reply', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'AI', exact: true })).toHaveCount(0);
  await expect(page.getByRole('region', { name: 'Summary' })).toHaveCount(0);
});

test('summaries appear above the message, stay after a reload and can be deleted', async ({ page }) => {
  const { summaries } = await aiOn(page);
  await page.goto('/');
  await openWeekend(page);
  const button = page.getByRole('button', { name: 'AI', exact: true });
  await expect(button).toBeVisible();
  // The menu is keyboard operable: arrows open it on the first item, Escape returns to the button.
  await button.focus();
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('menuitem', { name: 'Summarize' })).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('menuitem', { name: 'Reply' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('menu')).toHaveCount(0);
  await expect(button).toBeFocused();

  await button.click();
  await page.getByRole('menuitem', { name: 'Summarize' }).click();
  const card = page.getByRole('region', { name: 'Summary' });
  await expect(card).toContainText('bookshop by the river');
  expect(summaries.has('weekend')).toBe(true);

  await page.reload();
  await openWeekend(page);
  await expect(card).toContainText('bookshop by the river');
  await card.getByRole('button', { name: 'Delete' }).click();
  await expect(card).toHaveCount(0);
  expect(summaries.has('weekend')).toBe(false);
});

test('reply drafts take a tone from the segmented control and open in the composer', async ({ page }) => {
  const { replies } = await aiOn(page);
  await page.goto('/');
  await openWeekend(page);
  await page.getByRole('button', { name: 'AI', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Reply' }).click();
  const dialog = page.getByRole('dialog', { name: 'Draft a reply' });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel('What do you want to say?').fill('cant come sat, maybe next time');

  const casual = dialog.getByRole('radio', { name: 'Casual', exact: true });
  await expect(casual).toBeChecked();
  // Arrow keys move along the pill; the focused tone explains itself.
  await casual.focus();
  await page.keyboard.press('ArrowRight');
  const lazy = dialog.getByRole('radio', { name: 'Lazy', exact: true });
  await expect(lazy).toBeChecked();
  await expect(lazy).toBeFocused();
  await expect(lazy).toHaveAccessibleDescription(/all lowercase/);
  await expect(dialog.getByRole('tooltip', { name: /all lowercase/ })).toBeVisible();
  await expect(dialog.getByRole('tooltip', { name: /Formal and polished/ })).toBeHidden();
  await dialog.locator('label').filter({ has: page.getByRole('radio', { name: 'Professional', exact: true }) }).hover();
  await expect(dialog.getByRole('tooltip', { name: /Formal and polished/ })).toBeVisible();

  await dialog.getByRole('button', { name: 'Generate' }).click();
  const draft = dialog.getByLabel('Reply', { exact: true });
  await expect(draft).toHaveValue('cant make it saturday lol, next time');
  expect(replies).toEqual([{ message_id: 'weekend', input: 'cant come sat, maybe next time', tone: 'lazy' }]);
  await draft.fill('cant make it saturday lol, next one for sure');
  await dialog.getByRole('button', { name: 'Use as reply' }).click();

  await expect(dialog).toBeHidden();
  const body = page.getByLabel('Message', { exact: true });
  await expect(body).toHaveValue(/^cant make it saturday lol, next one for sure\n\n\nOn .*Maya Chen <maya@example\.test> wrote:\n> Hey,/s);
  await expect(page.getByLabel('Subject')).toHaveValue('Re: A small plan for the weekend');
});

test('Settings → AI keeps everything inactive until AI features are on', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: 'AI', exact: true }).click();
  const master = page.getByRole('checkbox', { name: 'Use AI features' });
  await expect(master).not.toBeChecked();
  await expect(page.getByLabel('Base URL').first()).toBeDisabled();
  await expect(page.getByRole('checkbox', { name: 'Label new mail automatically' })).toBeDisabled();
});
