import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';

const example = {
  sources: ['autoconfig'], provider: 'Example Mail', jmap: null, username: null, app_password: false, limitation: null,
  imap: { host: 'imap.example.org', port: 993, security: 'tls' }, smtp: { host: 'smtp.example.org', port: 587, security: 'starttls' },
};
const lookups: Record<string, unknown> = {
  'example.org': example,
  'gmail.com': { ...example, sources: ['provider_list'], provider: 'Gmail', app_password: true, imap: { host: 'imap.gmail.com', port: 993, security: 'tls' }, smtp: { host: 'smtp.gmail.com', port: 465, security: 'tls' } },
  'outlook.com': { ...example, sources: ['provider_list'], provider: 'Outlook.com', imap: null, smtp: null, limitation: 'oauth_only' },
};

/** Serve lookups from the table above and capture the saved account, without touching the shared Host. */
async function mock(page: Page) {
  const asked: string[] = [];
  const saved: Record<string, unknown>[] = [];
  await page.route('**/api/v1/accounts/discover', async route => {
    const { address } = route.request().postDataJSON();
    asked.push(address);
    const found = lookups[address.split('@')[1]];
    await route.fulfill({ json: found ? { ...found, username: (found as typeof example).username ?? address } : null });
  });
  await page.route('**/api/v1/accounts', async route => {
    if (route.request().method() !== 'POST') return route.continue();
    saved.push(route.request().postDataJSON());
    await route.fulfill({ json: { id: 'c'.repeat(32), settings: null, connected: false, configured: true } });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('dialog', { name: 'Settings' }).getByRole('button', { name: 'Accounts', exact: true }).click();
  await page.getByRole('button', { name: 'Add account', exact: true }).click();
  return { asked, saved, dialog: page.locator('dialog.account-dialog') };
}
const continueButton = (page: Page) => page.getByRole('button', { name: 'Continue', exact: true });
const back = (page: Page) => page.locator('dialog.account-dialog').getByRole('button', { name: 'Back', exact: true });

test('a lookup fills the servers and names the standard it used', async ({ page }) => {
  const { dialog } = await mock(page);
  await page.getByLabel('Email address', { exact: true }).fill('jo@example.org');
  await continueButton(page).click();
  const note = dialog.getByRole('status').filter({ hasText: 'Settings found' });
  await expect(note).toContainText('Settings found for Example Mail');
  await expect(note).toContainText('Thunderbird Autoconfig published by example.org');
  await expect(page.getByLabel('Incoming (IMAP) server')).toHaveValue('imap.example.org');
  await expect(page.getByRole('group', { name: 'SMTP port' }).getByRole('radio', { name: /^587/ })).toBeChecked();
  await expect(page.getByLabel('Username', { exact: true })).toHaveValue('jo@example.org');
});

test('no toast appears when nothing could be found', async ({ page }) => {
  const { dialog, asked } = await mock(page);
  await page.getByLabel('Email address', { exact: true }).fill('jo@unknown.example.net');
  await continueButton(page).click();
  await expect(page.getByRole('heading', { name: 'Server settings' })).toBeVisible();
  await expect(dialog.getByText('Settings found')).toHaveCount(0);
  await expect(page.getByLabel('Incoming (IMAP) server')).toHaveValue('');
  expect(asked).toEqual(['jo@unknown.example.net']);
});

test('going back and changing the email keeps page 2 intact', async ({ page }) => {
  const { dialog, asked, saved } = await mock(page);
  await page.getByLabel('Email address', { exact: true }).fill('jo@example.org');
  await continueButton(page).click();
  await expect(page.getByLabel('Incoming (IMAP) server')).toHaveValue('imap.example.org');
  // The owner corrects one server, picks a custom port and types the password.
  await page.getByLabel('Outgoing (SMTP) server').fill('smtp.custom.example.org');
  await page.getByLabel('Other IMAP port').fill('2993');
  await page.getByLabel('Password', { exact: true }).fill('provider-secret');

  // A domain with no settings to find changes nothing on page 2 except the username that follows the address.
  await back(page).click();
  await expect(page.getByLabel('Email address', { exact: true })).toHaveValue('jo@example.org');
  await page.getByLabel('Email address', { exact: true }).fill('jo@unknown.example.net');
  await continueButton(page).click();
  await expect(dialog.getByText('Settings found')).toHaveCount(0);
  await expect(page.getByLabel('Incoming (IMAP) server')).toHaveValue('imap.example.org');
  await expect(page.getByLabel('Outgoing (SMTP) server')).toHaveValue('smtp.custom.example.org');
  await expect(page.getByLabel('Other IMAP port')).toHaveValue('2993');
  await expect(page.getByLabel('Password', { exact: true })).toHaveValue('provider-secret');
  await expect(page.getByLabel('Username', { exact: true })).toHaveValue('jo@unknown.example.net');

  // Another provider fills only what the owner never edited, and says what it kept.
  await back(page).click();
  await page.getByLabel('Email address', { exact: true }).fill('sam@gmail.com');
  await continueButton(page).click();
  const note = dialog.getByRole('status').filter({ hasText: 'Settings found' });
  await expect(note).toContainText('Gmail');
  await expect(note).toContainText('Colecto’s built-in provider list');
  await expect(note).toContainText('outgoing server');
  await expect(page.getByLabel('Incoming (IMAP) server')).toHaveValue('imap.gmail.com');
  await expect(page.getByLabel('Outgoing (SMTP) server')).toHaveValue('smtp.custom.example.org');
  await expect(page.getByLabel('Other IMAP port')).toHaveValue('2993');
  await expect(page.getByRole('group', { name: 'SMTP port' }).getByRole('radio', { name: /^465/ })).toBeChecked();
  await expect(page.getByLabel('App password', { exact: true })).toHaveValue('provider-secret');

  // Only the part before @ changed: the lookup is reused and the username follows.
  await back(page).click();
  await page.getByLabel('Email address', { exact: true }).fill('alex@gmail.com');
  await continueButton(page).click();
  await expect(page.getByLabel('Username', { exact: true })).toHaveValue('alex@gmail.com');
  expect(asked).toEqual(['jo@example.org', 'jo@unknown.example.net', 'sam@gmail.com']);

  await continueButton(page).click();
  await expect(page.getByRole('heading', { name: 'Check and save' })).toBeVisible();
  await expect(dialog.getByText('alex@gmail.com').first()).toBeVisible();
  await expect(dialog.getByText('IMAP · port 2993 · TLS')).toBeVisible();
  await expect(dialog.getByText('SMTP · port 465 · TLS')).toBeVisible();
  // Back from the review returns to page 2 unchanged, and Change jumps to the right field.
  await dialog.getByRole('button', { name: 'Change outgoing server' }).click();
  await expect(page.getByLabel('Outgoing (SMTP) server')).toBeFocused();
  await continueButton(page).click();
  await page.getByLabel('Account name (optional)').fill('Personal');
  await page.getByRole('button', { name: 'Save account', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  expect(saved).toEqual([expect.objectContaining({
    name: 'Personal', address: 'alex@gmail.com', protocol: 'imap', username: 'alex@gmail.com', secret: 'provider-secret',
    imap_host: 'imap.gmail.com', imap_port: 2993, imap_starttls: false, smtp_host: 'smtp.custom.example.org', smtp_port: 465,
  })]);
});

test('any port is accepted, and impossible ones are stopped on page 2', async ({ page }) => {
  const { dialog } = await mock(page);
  await page.getByLabel('Email address', { exact: true }).fill('jo@example.org');
  await continueButton(page).click();
  await page.getByLabel('Password', { exact: true }).fill('provider-secret');
  await page.getByLabel('Other SMTP port').fill('70000');
  await continueButton(page).click();
  await expect(page.getByRole('heading', { name: 'Server settings' })).toBeVisible();
  await page.getByLabel('Other SMTP port').fill('25');
  await expect(page.getByRole('group', { name: 'SMTP port' }).getByRole('radio', { name: /^25/ })).toBeChecked();
  await page.getByLabel('Other SMTP port').fill('2525');
  await continueButton(page).click();
  await expect(dialog.getByText('SMTP · port 2525 · STARTTLS')).toBeVisible();
  // The review flags likely mistakes without blocking the save.
  await back(page).click();
  await page.getByRole('group', { name: 'SMTP port' }).getByRole('radio', { name: /^25\b/ }).check({ force: true });
  await continueButton(page).click();
  await expect(dialog.getByText(/Many networks block port 25/)).toBeVisible();
});

test('a provider without password sign-in is explained before page 2', async ({ page }) => {
  const { dialog } = await mock(page);
  await page.getByLabel('Email address', { exact: true }).fill('jo@outlook.com');
  await continueButton(page).click();
  await expect(dialog.getByRole('alert')).toContainText('Outlook.com only allows OAuth sign-in');
  await expect(page.getByRole('heading', { name: 'Server settings' })).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Enter settings anyway' }).click();
  await expect(page.getByRole('heading', { name: 'Server settings' })).toBeVisible();
  await expect(dialog.getByText('Settings found')).toHaveCount(0);
});

test('JMAP keeps any port in its session URL', async ({ page }) => {
  const { saved } = await mock(page);
  await page.getByLabel('Email address', { exact: true }).fill('jo@unknown.example.net');
  await page.getByRole('button', { name: 'Enter settings myself' }).click();
  await page.getByRole('radio', { name: 'JMAP', exact: true }).check({ force: true });
  await page.getByLabel('JMAP server or session URL').fill('mail.example.net/jmap/session');
  await page.getByLabel('Other JMAP port').fill('8080');
  await page.getByLabel('Username').fill('');
  await page.getByLabel('API token', { exact: true }).fill('token');
  await continueButton(page).click();
  await expect(page.getByText('https://mail.example.net:8080/jmap/session')).toBeVisible();
  await page.getByRole('button', { name: 'Save account', exact: true }).click();
  expect(saved[0]).toMatchObject({ protocol: 'jmap', session_url: 'https://mail.example.net:8080/jmap/session', username: '' });
});
