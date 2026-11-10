import { test, expect } from './fixtures';

test('local labels can be created, assigned, filtered and removed', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /Maya Chen/ })).toBeVisible();
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Create a local label');
  await page.keyboard.press('Enter');
  await expect(page.getByLabel('New label')).toBeFocused();
  await page.getByLabel('New label').fill('Follow up');
  await page.getByRole('button', { name: 'Create label' }).click();
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await page.getByLabel('Label Follow up', { exact: true }).check();
  await expect(page.getByLabel('Label Follow up', { exact: true })).toBeEnabled();
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Remove label: Follow up');
  await page.keyboard.press('Enter');
  await expect(page.getByLabel('Label Follow up', { exact: true })).not.toBeChecked();
  await page.getByLabel('Label Follow up', { exact: true }).check();
  await page.reload();
  await page.getByRole('group', { name: 'Filter by label' }).getByRole('button', { name: 'Follow up', exact: true }).click();
  await expect(page.getByRole('button', { name: /Maya Chen/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /Alex Rivera/ })).toHaveCount(0);
  page.once('dialog', dialog => dialog.accept());
  await page.getByRole('button', { name: 'Delete label Follow up' }).click();
  await expect(page.getByRole('button', { name: /Alex Rivera/ })).toBeVisible();
});
