import { test, expect, ownerToken, chooseAccount } from './fixtures';

test('keeps separate closable message tabs and has a sent folder on mobile', async ({ page }) => {
  const messages = [{ id: 'maya', sender: 'Maya Chen', address: 'maya@example.test', subject: 'A small plan for the weekend', body: 'Coffee?', date: 'Today' }, { id: 'alex', sender: 'Alex Rivera', address: 'alex@example.test', subject: 'Plain text, with a little character', body: 'Hello', date: 'Today' }];
  await page.route('**/api/v1/messages**', route => {
    const url = new URL(route.request().url());
    if (url.pathname.endsWith('/cache')) return route.fallback();
    if (url.searchParams.get('folder') === 'sent') return route.fulfill({ json: [] });
    const id = url.pathname.split('/').at(-1);
    return route.fulfill({ json: id === 'refresh' ? messages : messages.find(message => message.id === id) });
  });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await page.getByRole('button', { name: 'Back to list' }).click();
  await page.getByRole('button', { name: /Alex Rivera/ }).click();
  await page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'A small plan for the weekend', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'A small plan for the weekend' })).toBeVisible();
  await page.getByRole('button', { name: 'Close A small plan for the weekend', exact: true }).click();
  await expect(page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'A small plan for the weekend', exact: true })).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Plain text, with a little character' })).toBeVisible();
  await page.getByRole('button', { name: 'Sent', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Sent', exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('opens unified settings without discard confirmation for a clean workspace', async ({ page }) => {
  let dialogs = 0; page.on('dialog', async dialog => { dialogs++; await dialog.dismiss(); });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Settings', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Extensions', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Extensions', exact: true })).toBeVisible();
  expect(dialogs).toBe(0);
});

test('remote access token stays in the session and reconnecting preserves a draft', async ({ page }) => {
  const requireToken = true;
  await page.route('**/api/v1/**', route => {
    if (requireToken && route.request().headers().authorization !== 'Bearer sample-owner-token') return route.fulfill({ status: 401, json: { error: 'Enter the daemon access token to connect' } });
    const headers = { ...route.request().headers() };
    headers.authorization = `Bearer ${ownerToken}`;
    return route.continue({ headers });
  });
  await page.goto('/');
  await page.getByLabel('Daemon access token').fill('sample-owner-token');
  await page.getByRole('button', { name: 'Connect', exact: true }).click();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('Subject', { exact: true }).fill('Keep this through reconnect');
  await page.evaluate(() => sessionStorage.removeItem('colecto.token'));
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await page.getByLabel('Daemon access token').fill('sample-owner-token');
  await page.getByRole('button', { name: 'Connect', exact: true }).click();
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Keep this through reconnect');
  expect(await page.evaluate(() => localStorage.getItem('colecto.token'))).toBeNull();
  expect(page.url()).not.toContain('sample-owner-token');
});

test('profile names fall back to email and message tabs stay with their account', async ({ page }) => {
  const accounts = ['a', 'b'].map(id => ({ id, configured: true, connected: false, settings: { name: '', address: `${id}@example.test`, protocol: 'imap', imap_host: '', imap_port: 993, smtp_host: '', smtp_port: 587, username: '', session_url: '' } }));
  await page.route('**/api/v1/labels', route => route.fulfill({ json: {} }));
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: accounts }));
  await page.route('**/api/v1/messages/refresh*', route => {
    const account = route.request().headers()['x-colecto-account'];
    return route.fulfill({ json: [{ id: 'same-id', sender: account, address: `${account}@example.test`, subject: `Mail for ${account}`, body: 'Body', date: 'Today' }] });
  });
  await page.route('**/api/v1/messages/same-id', route => {
    const account = route.request().headers()['x-colecto-account'];
    return route.fulfill({ json: { id: 'same-id', sender: account, address: `${account}@example.test`, subject: `Mail for ${account}`, body: 'Body', date: 'Today' } });
  });
  await page.goto('/');
  await chooseAccount(page, 'a@example.test');
  await page.getByRole('button', { name: /Mail for a/ }).click();
  await expect(page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'Mail for a', exact: true })).toBeVisible();
  await chooseAccount(page, 'b@example.test');
  await expect(page.getByRole('navigation', { name: 'Open messages' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Current account' })).toContainText('b@example.test');
  await page.getByRole('button', { name: /Mail for b/ }).click();
  await chooseAccount(page, 'a@example.test');
  await expect(page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'Mail for a', exact: true })).toBeVisible();
  await expect(page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'Mail for b', exact: true })).toHaveCount(0);
});

test('account action labels use email when the optional profile name is empty', async ({ page }) => {
  await page.route('**/api/v1/labels', route => route.fulfill({ json: {} }));
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: [{ id: 'unnamed', configured: true, connected: false, settings: { name: '', address: 'unnamed@example.test', protocol: 'imap', imap_host: 'imap.example.test', imap_port: 993, smtp_host: 'smtp.example.test', smtp_port: 587, username: 'unnamed', session_url: '' } }] }));
  await page.route('**/api/v1/security', route => route.fulfill({ json: { protected: false, locked: false, cleanup_pending: false } }));
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Remove', exact: true })).toBeVisible();
  let question = '';
  page.once('dialog', async dialog => { question = dialog.message(); await dialog.dismiss(); });
  await page.getByRole('button', { name: 'Remove', exact: true }).click();
  expect(question).toContain('Remove saved credentials for unnamed@example.test?');
});

test('mobile controls stay reachable and mail text stays selectable on narrow screens', async ({ page }) => {
  const message = { id: 'mobile-reader', sender: 'Maya Chen', address: 'maya@example.test', subject: 'Reading on a phone', body: 'Selectable message text.\n'.repeat(100), date: 'Today' };
  await page.route('**/api/v1/messages**', route => /\/(cache|content)$/.test(new URL(route.request().url()).pathname) ? route.fallback() : route.fulfill({ json: new URL(route.request().url()).pathname.endsWith('/refresh') ? [message] : message }));
  await page.setViewportSize({ width: 320, height: 640 });
  await page.goto('/');
  const folders = page.getByRole('navigation', { name: 'Mail folders' });
  const bounds = await folders.boundingBox();
  expect(bounds!.y + bounds!.height).toBe(640);
  for (const button of await folders.getByRole('button').all()) {
    const box = await button.boundingBox();
    expect(box!.width).toBeGreaterThanOrEqual(44);
    expect(box!.height).toBeGreaterThanOrEqual(48);
  }
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await expect(page.locator('.letter-body')).toBeVisible();
  expect(await page.locator('.letter').evaluate(el => getComputedStyle(el).userSelect)).toBe('text');
  await page.locator('.reading-pane').evaluate(el => el.scrollTop = el.scrollHeight);
  await expect(page.getByRole('button', { name: 'Back to list' })).toBeInViewport();
  await page.getByRole('button', { name: 'Back to list' }).click();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('Message', { exact: true }).fill('A long draft\n'.repeat(100));
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('Draft saved');
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(320);
});

test('command runner opens by keyboard, filters actions and searches drafts', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('Subject', { exact: true }).fill('Find this local note');
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  // Commands are disabled while the save is in flight.
  await expect(page.getByRole('status')).toHaveText('Draft saved');
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('drafts');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('dialog', { name: 'Commands' })).not.toBeVisible();
  // Search follows typing; there is nothing to submit.
  await page.getByRole('searchbox').fill('absent words');
  await expect(page.getByText('No matching messages')).toBeVisible();
  await page.getByRole('searchbox').fill('local note');
  await expect(page.getByRole('button', { name: /Find this local note/ })).toBeVisible();
});


test('worst-case mail remains usable at narrow widths and enlarged text', async ({ page }) => {
  const { worst } = await import('../src/lib/stress-fixtures');
  await page.route('**/api/v1/messages**', route => {
    const id = new URL(route.request().url()).pathname.split('/').at(-1);
    if (id === 'cache') return route.fallback();
    return route.fulfill({ json: id === 'refresh' ? worst : worst.find(message => message.id === id) });
  });
  for (const width of [320, 390, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/');
    await page.getByRole('button', { name: /Aleksandra/ }).click();
    await page.evaluate(() => { document.documentElement.style.fontSize = '200%'; });
    await expect(page.getByRole('heading', { name: worst[0].subject })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    if (width <= 800) await page.getByRole('button', { name: 'Back to list' }).click();
    await page.getByRole('button', { name: /Priya/ }).click();
    await expect(page.locator('.letter .avatar')).toHaveText('👩🏽‍💻');
    await expect(page.getByRole('heading', { name: '(No subject)' })).toBeVisible();
  }
});

test('command navigation preserves canceled unsaved edits and restores focus', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  const subject = page.getByLabel('Subject', { exact: true });
  await subject.fill('Keep my changes');
  await page.keyboard.press('Control+k');
  await page.keyboard.press('Escape');
  await expect(subject).toBeFocused();
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Go to Inbox');
  page.once('dialog', dialog => dialog.dismiss());
  await page.keyboard.press('Enter');
  await expect(subject).toHaveValue('Keep my changes');
});

test('search command keeps focus and send command uses the existing send flow', async ({ page }) => {
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: [{ id: 'demo', settings: null, configured: false, connected: true }] }));
  await page.route('**/api/v1/send', route => route.fulfill({ json: true }));
  await page.goto('/');
  await page.getByRole('button', { name: /Commands/ }).click();
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Search emails');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('searchbox')).toBeFocused();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('To', { exact: true }).fill('maya@example.test');
  await page.getByLabel('Subject', { exact: true }).fill('Command send');
  await page.keyboard.press('Control+k');
  await page.getByRole('combobox', { name: 'Search commands' }).fill('Send message');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('status')).toHaveText('Accepted by mail server');
});

test('shows cached mail during refresh and preserves reader and editor after proxy failure', async ({ page }) => {
  const cached = { id: 'cache-reader', sender: 'Maya Chen', address: 'maya@example.test', subject: 'Previously fetched mail', body: 'Cached content remains readable.', date: 'Today' };
  await page.route('**/api/v1/messages/cache?*', route => route.fulfill({ json: { folder: 'inbox', messages: [cached], fetched_at: 1 } }));
  await page.route('**/api/v1/messages/cache-reader', route => route.fulfill({ json: cached }));
  let release!: () => void;
  const gate = new Promise<void>(resolve => release = resolve);
  await page.route('**/api/v1/messages/refresh*', async route => {
    await gate;
    return route.fulfill({ status: 502, contentType: 'text/html', body: '<h1>Proxy unavailable SECRET</h1>' });
  });
  await page.goto('/');
  await expect(page.getByText('Fetching latest messages…')).toBeVisible();
  await page.getByRole('button', { name: /Maya Chen/ }).click();
  await expect(page.getByRole('region', { name: 'Message reader' })).toBeVisible();
  release();
  await expect(page.getByRole('alert')).toContainText('HTTP 502');
  await expect(page.getByRole('alert')).not.toContainText('SECRET');
  await expect(page.getByRole('region', { name: 'Message reader' })).toBeVisible();
  await expect(page.getByRole('button', { name: /Maya Chen/ })).toBeVisible();
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  await page.getByLabel('Subject', { exact: true }).fill('Keep unsaved draft');
  await page.getByRole('button', { name: 'Try again', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('HTTP 502');
  await expect(page.getByLabel('Subject', { exact: true })).toHaveValue('Keep unsaved draft');
});

test('activating an existing message tab clears an invalidated refresh', async ({ page }) => {
  const messages = [
    { id: 'first-tab', sender: 'First sender', address: 'first@example.test', subject: 'First retained tab', body: 'First body', date: 'Today' },
    { id: 'second-tab', sender: 'Second sender', address: 'second@example.test', subject: 'Second retained tab', body: 'Second body', date: 'Today' },
  ];
  await page.route('**/api/v1/messages/cache?*', route => route.fulfill({ json: { folder: 'inbox', messages, fetched_at: 1 } }));
  await page.route('**/api/v1/messages/*-tab', route => route.fulfill({ json: messages.find(message => route.request().url().endsWith(message.id)) }));
  let refreshes = 0;
  let release!: () => void;
  const gate = new Promise<void>(resolve => release = resolve);
  await page.route('**/api/v1/messages/refresh*', async route => {
    if (++refreshes > 1) await gate;
    return route.fulfill({ json: messages });
  });
  await page.goto('/');
  const refresh = page.getByRole('button', { name: 'Refresh', exact: true });
  await expect(refresh).toBeEnabled();
  await page.getByRole('button', { name: /First sender/ }).click();
  await page.getByRole('button', { name: /Second sender/ }).click();
  await refresh.click();
  await expect(page.getByText('Fetching latest messages…')).toBeVisible();
  await page.getByRole('navigation', { name: 'Open messages' }).getByRole('button', { name: 'First retained tab', exact: true }).click();
  const settled = page.waitForResponse(response => response.url().includes('/messages/refresh'));
  release();
  await settled;
  await expect(page.getByText('Fetching latest messages…')).toBeHidden();
  await expect(refresh).toBeEnabled();
  await expect(page.getByRole('heading', { name: 'First retained tab' })).toBeVisible();
});

test('docks panes by dragging onto edges and tab strips, keeps an open draft, and remembers the layout', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Compose', exact: true }).click();
  const subject = page.getByLabel('Subject', { exact: true });
  await subject.fill('Moving house');
  const group = (tab: string) => page.locator('.dock-frame', { has: page.getByRole('tab', { name: tab, exact: true }) });
  async function drag(tab: string, x: number, y: number) {
    await page.getByRole('tab', { name: tab, exact: true }).hover();
    await page.mouse.down();
    await page.mouse.move(x, y, { steps: 8 });
    await expect(page.locator('.drop-preview')).toBeVisible();
    await page.mouse.up();
  }
  // The account switcher starts under the mail list.
  const mail = (await group('Mail').boundingBox())!;
  expect((await group('Accounts').boundingBox())!.y).toBeGreaterThan(mail.y + mail.height - 2);
  // Dropping on the reader's right edge splits its space.
  let reader = (await group('Reader').boundingBox())!;
  await drag('Accounts', reader.x + reader.width - 30, reader.y + reader.height / 2);
  reader = (await group('Reader').boundingBox())!;
  expect((await group('Accounts').boundingBox())!.x).toBeGreaterThanOrEqual(reader.x + reader.width);
  await expect(subject).toHaveValue('Moving house');
  // Dropping on a tab strip adds a tab instead; the hidden reader keeps the draft.
  const head = (await group('Reader').locator('.dock-head').boundingBox())!;
  await drag('Mail', head.x + head.width - 80, head.y + head.height / 2);
  await expect(group('Reader').getByRole('tab')).toHaveText(['Reader', 'Mail']);
  await expect(page.getByRole('tab', { name: 'Mail', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(subject).toHaveValue('Moving house');
  const handle = page.getByRole('separator').first();
  const before = Number(await handle.getAttribute('aria-valuenow'));
  await handle.focus();
  await page.keyboard.press('Shift+ArrowLeft');
  await expect(handle).not.toHaveAttribute('aria-valuenow', String(before));
  await page.getByRole('tab', { name: 'Reader', exact: true }).click();
  await page.getByRole('button', { name: 'Save draft', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Draft editor' }).getByRole('status')).toContainText('Draft saved');
  await page.getByRole('button', { name: 'Arrange Accounts' }).click();
  await page.getByRole('option', { name: 'Hide this pane' }).click();
  await expect(page.getByRole('tab', { name: 'Accounts', exact: true })).toHaveCount(0);
  // The layout is stored by the Host, so a reload restores it.
  await page.reload();
  await expect(group('Reader').getByRole('tab')).toHaveText(['Reader', 'Mail']);
  await expect(page.getByRole('tab', { name: 'Accounts', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Extensions', exact: true }).click();
  await page.getByRole('group', { name: 'Account switcher' }).getByLabel('Show in the workspace').check();
  await page.getByRole('button', { name: 'Close settings', exact: true }).click();
  await expect(page.getByRole('tab', { name: 'Accounts', exact: true })).toBeVisible();
  await page.keyboard.press('Control+k');
  await page.keyboard.type('layout reset');
  await page.keyboard.press('Enter');
  await expect(group('Mail').getByRole('tab')).toHaveText(['Mail']);
});

test('command runner closes on Escape and outside clicks, finds mail, sorts and navigates', async ({ page }) => {
  const inbox = [
    { id: 'maya', sender: 'Maya Chen', address: 'maya@example.test', subject: 'A small plan for the weekend', body: 'Hey, I found a little bookshop by the river.', date: 'Today' },
    { id: 'studio', sender: 'Studio North', address: 'studio@example.test', subject: 'Notes from our last conversation', body: 'A few things to keep close.', date: 'Today' },
    { id: 'alex', sender: 'Alex Rivera', address: 'alex@example.test', subject: 'Plain text, with a little character', body: 'Hello', date: 'Today' },
  ];
  await page.route('**/api/v1/messages**', route => {
    const url = new URL(route.request().url());
    if (/\/(cache|content)$/.test(url.pathname)) return route.fallback();
    const q = (url.searchParams.get('q') ?? '').toLowerCase();
    if (url.pathname.endsWith('/refresh')) return route.fulfill({ json: inbox.filter(m => `${m.subject} ${m.body}`.toLowerCase().includes(q)) });
    return route.fulfill({ json: inbox.find(m => url.pathname.endsWith(`/${m.id}`)) });
  });
  await page.goto('/');
  const runner = page.getByRole('dialog', { name: 'Commands' });
  await expect(page.getByRole('button', { name: /Maya Chen/ })).toBeVisible();
  await page.keyboard.press('Control+k');
  await expect(runner).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(runner).toBeHidden();
  await page.getByRole('button', { name: 'Commands', exact: true }).click();
  await expect(runner).toBeVisible();
  await page.mouse.click(8, page.viewportSize()!.height - 8);
  await expect(runner).toBeHidden();
  // Typing searches the loaded mail as well as commands.
  await page.keyboard.press('Control+k');
  await page.keyboard.type('bookshop');
  await expect(runner.getByRole('option', { name: /A small plan for the weekend/ })).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'A small plan for the weekend' })).toBeVisible();
  await page.keyboard.press('Control+k');
  await page.keyboard.type('sort sender');
  await page.keyboard.press('Enter');
  const senders = await page.locator('.mail-item .item-sender').allTextContents();
  expect(senders).toEqual([...senders].sort((a, b) => a.toLocaleLowerCase().localeCompare(b.toLocaleLowerCase())));
  await expect(page.getByText('Sorted sender a–z')).toBeVisible();
  await page.getByRole('button', { name: 'Clear sorting' }).click();
  await expect(page.getByText('Sorted sender a–z')).toHaveCount(0);
  // The last result hands the words to the provider's search.
  await page.keyboard.press('Control+k');
  await page.keyboard.type('weekend');
  await runner.getByRole('option', { name: 'Search Inbox for “weekend”' }).click();
  await expect(page.getByLabel('Search Inbox')).toHaveValue('weekend');
  await expect(page.getByRole('button', { name: /Studio North/ })).toHaveCount(0);
  await page.keyboard.press('Control+k');
  await page.keyboard.type('go drafts');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('navigation', { name: 'Mail folders' }).getByRole('button', { name: /Drafts/ })).toHaveAttribute('aria-current', 'page');
});

test('account dialog and long identities remain usable at 360px', async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 800 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.route('**/api/v1/accounts', route => route.fulfill({ json: Array.from({ length: 30 }, (_, index) => ({
    id: 'stress-' + index, configured: true, connected: index === 0,
    settings: { name: index ? 'Jo' : 'Aleksandra Wiśniewska-Kowalczyk', address: 'bartholomew.fitzgerald@northwind-industries-holdings.example.com', protocol: 'imap', imap_host: 'imap.example.test', imap_port: 993, smtp_host: 'smtp.example.test', smtp_port: 587, username: 'user', session_url: '' },
  })) }));
  await page.route('**/api/v1/security', route => route.fulfill({ json: { protected: false, locked: false, cleanup_pending: false } }));
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const edit = page.getByRole('button', { name: 'Edit', exact: true }).first();
  await edit.click();
  await expect(page.locator('dialog.account-dialog')).toBeVisible();
  await expect(page.getByLabel('Account name (optional)')).toHaveValue('Aleksandra Wiśniewska-Kowalczyk');
  await page.keyboard.press('Escape');
  await expect(page.locator('dialog.account-dialog')).not.toBeVisible();
  await page.getByRole('button', { name: 'Add account', exact: true }).click();
  await page.getByLabel('Email address', { exact: true }).fill('jo@example.test');
  await expect(page.getByLabel('Username', { exact: true })).toHaveValue('jo@example.test');
  await page.getByRole('radio', { name: 'JMAP', exact: true }).focus();
  await page.keyboard.press('Space');
  await expect(page.getByLabel('IMAP host')).not.toBeVisible();
  await expect(page.getByLabel('JMAP server or session URL')).toBeVisible();
  const dialog = page.locator('dialog.account-dialog');
  expect(await dialog.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  page.once('dialog', prompt => prompt.accept());
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(dialog).not.toBeVisible();
});

test('appearance applies and restores typography preferences', async ({ page }) => {
  let typography = { font: 'mona', font_size: 14 };
  await page.route('**/api/v1/preferences', async route => {
    if (route.request().method() !== 'GET') typography = { ...typography, ...route.request().postDataJSON() };
    await route.fulfill({ json: { mcp: false, account: null, workspace: null, accent: null, remote_images: false, notifications: { enabled: true, mail: true, approvals: true, system: false, position: 'top' }, ...typography } });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Appearance', exact: true }).click();
  await page.getByRole('button', { name: 'Interface font' }).click();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect.poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue('--font'))).toContain('Inter Variable');
  await page.getByRole('radio', { name: /XL/ }).focus();
  await page.keyboard.press('Space');
  await expect.poll(() => page.evaluate(() => document.documentElement.style.fontSize)).toBe('18px');
  await page.reload();
  await expect.poll(() => page.evaluate(() => document.documentElement.style.fontSize)).toBe('18px');
});
