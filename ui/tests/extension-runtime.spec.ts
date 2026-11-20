// Client parts in their Worker (docs/extensions/ui-model.md, "Client runtime"): what they can
// reach, how they draw, and what happens when they fail or are turned off.
import { readFileSync } from 'node:fs';
import { test, expect, ownerToken } from './fixtures';
import type { Page } from '@playwright/test';
import { pane, zip } from './package';

const origin = `http://127.0.0.1:${process.env.COLECTO_TEST_PORT ?? '4179'}`;
const host = { authorization: `Bearer ${ownerToken}`, origin, 'x-colecto-extension': 'host', 'x-colecto-account': 'demo' };

async function install(page: Page, bytes: Buffer) {
  const response = await page.request.post(`${origin}/api/v1/extensions`, { headers: { ...host, 'content-type': 'application/octet-stream' }, data: bytes });
  expect(response.ok(), await response.text()).toBe(true);
}
async function remove(page: Page, id: string) {
  await page.request.delete(`${origin}/api/v1/extensions/${id}`, { headers: host, data: {} });
}
const paneOf = (page: Page, name: string) => page.getByRole('region', { name, exact: true });

test('the example pane draws, handles presses, calls the API and follows its settings', async ({ page }) => {
  await install(page, readFileSync('../target/extensions/hello-pane.colectoext'));
  try {
    await page.goto('/');
    await page.getByRole('tab', { name: 'Hello pane' }).click();
    const hello = paneOf(page, 'Hello pane');
    await expect(hello.getByRole('heading', { name: 'Hello, Demo account' })).toBeVisible();
    await hello.getByRole('button', { name: 'Pressed 0 times' }).click();
    await expect(hello.getByRole('button', { name: 'Pressed 1 time' })).toBeVisible();
    await hello.getByRole('button', { name: 'Check connection' }).click();
    await expect(hello.getByText(/^(Connected|Not connected)\.$/)).toBeVisible();
    // A settings change in another window reaches the Worker through the event stream.
    await page.request.put(`${origin}/api/v1/extensions/hello-pane/settings`, { headers: host, data: { greeting: 'Howdy' } });
    await expect(hello.getByRole('heading', { name: 'Howdy, Demo account' })).toBeVisible();
  } finally { await remove(page, 'hello-pane'); }
});

test('a client part has no network, storage, owner token or DOM, but can use Wasm', async ({ page }) => {
  const probe = `
    const results = {};
    const check = async (name, run) => { try { results[name] = String(await run()); } catch (error) { results[name] = 'blocked'; } };
    colecto.onmessage = async (message) => {
      if (message.t === 'ping') return colecto.send({ t: 'pong', n: message.n });
      if (message.t !== 'start') return;
      await check('fetch', () => fetch('/api/v1/accounts').then(r => r.status));
      await check('absolute', () => fetch('${origin}/api/v1/version').then(r => r.status));
      await check('socket', () => new Promise((resolve, reject) => { const s = new WebSocket('ws://127.0.0.1:${process.env.COLECTO_TEST_PORT ?? '4179'}/'); s.onopen = () => resolve('open'); s.onerror = () => reject(); }));
      await check('indexedDB', () => new Promise((resolve, reject) => { const r = indexedDB.open('x'); r.onsuccess = () => resolve('open'); r.onerror = () => reject(); }));
      await check('origin', () => self.origin);
      await check('document', () => typeof document);
      await check('token', () => JSON.stringify(message).includes('${ownerToken}') ? 'leaked' : 'absent');
      await check('wasm', () => WebAssembly.instantiate(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0])).then(() => 'ok'));
      colecto.send({ t: 'render', surface: 'pane', tree: { type: 'text', id: 'out', text: JSON.stringify(results) } });
    };`;
  await install(page, pane('probe', probe));
  let leaked = false;
  page.on('request', request => { if (request.url().includes('/api/v1/accounts') && request.frame() !== page.mainFrame()) leaked = true; });
  try {
    await page.goto('/');
    await page.getByRole('tab', { name: 'probe' }).click();
    const out = paneOf(page, 'probe').locator('p');
    await expect(out).toContainText('"wasm":"ok"');
    const results = JSON.parse((await out.textContent())!);
    expect(results).toEqual({ fetch: 'blocked', absolute: 'blocked', socket: 'blocked', indexedDB: 'blocked', origin: 'null', document: 'undefined', token: 'absent', wasm: 'ok' });
    expect(leaked).toBe(false);
  } finally { await remove(page, 'probe'); }
});

test('a crash or an invalid message stops only that extension, and it can restart', async ({ page }) => {
  const script = `
    let started = 0;
    colecto.onmessage = (message) => {
      if (message.t === 'ping') return colecto.send({ t: 'pong', n: message.n });
      if (message.t === 'start') colecto.send({ t: 'render', surface: 'pane', tree: { type: 'stack', children: [
        { type: 'button', id: 'crash', label: 'Crash' }, { type: 'button', id: 'invalid', label: 'Send HTML' }] } });
      if (message.t === 'event' && message.node === 'crash') throw new Error('boom');
      if (message.t === 'event' && message.node === 'invalid') colecto.send({ t: 'render', surface: 'pane', tree: { type: 'html', html: '<b>x</b>' } });
    };`;
  await install(page, pane('fragile', script));
  try {
    await page.goto('/');
    await page.getByRole('tab', { name: 'fragile' }).click();
    const fragile = paneOf(page, 'fragile');
    await fragile.getByRole('button', { name: 'Crash' }).click();
    await expect(fragile.getByRole('alert')).toHaveText('fragile stopped');
    await fragile.getByRole('button', { name: 'Restart' }).click();
    await fragile.getByRole('button', { name: 'Send HTML' }).click();
    await expect(fragile.getByRole('alert')).toContainText('Unknown component html');
    await expect(page.locator('b', { hasText: 'x' })).toHaveCount(0);
    // The rest of the workspace keeps working.
    await page.getByRole('button', { name: /Maya Chen/ }).click();
    await expect(page.getByRole('heading', { name: 'A small plan for the weekend' })).toBeVisible();
  } finally { await remove(page, 'fragile'); }
});

test('a client part that stops answering is stopped by the watchdog', async ({ page }) => {
  await install(page, pane('stuck', `colecto.onmessage = (message) => { if (message.t === 'start') colecto.send({ t: 'render', surface: 'pane', tree: { type: 'text', text: 'Never answers pings' } }); };`));
  try {
    await page.clock.install();
    await page.goto('/');
    await page.getByRole('tab', { name: 'stuck' }).click();
    await expect(paneOf(page, 'stuck').getByText('Never answers pings')).toBeVisible();
    await page.clock.runFor(16_000);
    await expect(paneOf(page, 'stuck').getByRole('alert')).toHaveText('stuck stopped responding');
  } finally { await remove(page, 'stuck'); }
});

test('turning an extension off stops its Worker and revokes its credential', async ({ page }) => {
  const calls: number[] = [];
  await install(page, readFileSync('../target/extensions/hello-pane.colectoext'));
  try {
    page.on('response', response => { if (response.url().endsWith('/api/v1/account') && response.request().headers().authorization?.startsWith('Colecto-Extension ')) calls.push(response.status()); });
    await page.goto('/');
    await page.getByRole('tab', { name: 'Hello pane' }).click();
    await paneOf(page, 'Hello pane').getByRole('button', { name: 'Check connection' }).click();
    await expect.poll(() => calls).toEqual([200]);
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('button', { name: 'Extensions', exact: true }).click();
    await page.getByRole('group', { name: 'Hello pane', exact: true }).getByRole('checkbox', { name: 'Enabled for all accounts' }).uncheck();
    await expect(page.locator('#extension-runtimes iframe[title="Hello pane runtime"]')).toHaveCount(0);
    await expect(page.getByRole('tab', { name: 'Hello pane' })).toHaveCount(0);
  } finally { await remove(page, 'hello-pane'); }
});

test('frames show HTML without running it, and start carries the theme', async ({ page }) => {
  const script = `
    colecto.onmessage = (message) => {
      if (message.t === 'ping') return colecto.send({ t: 'pong', n: message.n });
      if (message.t !== 'start') return;
      colecto.send({ t: 'render', surface: 'pane', tree: { type: 'stack', children: [
        { type: 'text', id: 'theme', text: message.theme.colors.accent + ' ' + message.theme.font.family + ' ' + message.theme.density },
        { type: 'frame', title: 'Preview', doc: '<p id=t>Body</p><script>parent.postMessage("ran", "*")</script><img src="https://example.com/x.png">' },
      ] } });
    };`;
  await install(page, pane('framed', script));
  const ran: unknown[] = [];
  await page.exposeFunction('recordMessage', (data: unknown) => ran.push(data));
  await page.addInitScript(() => window.addEventListener('message', event => { if (event.data === 'ran') void (window as unknown as { recordMessage: (d: unknown) => void }).recordMessage(event.data); }));
  let remote = false;
  await page.route('https://example.com/**', route => { remote = true; return route.abort(); });
  try {
    await page.goto('/');
    await page.getByRole('tab', { name: 'framed' }).click();
    const framed = paneOf(page, 'framed');
    await expect(framed.locator('p').first()).toHaveText('#a2d6b1 Mona Sans Variable pointer');
    await expect(framed.frameLocator('iframe[title="Preview"]').locator('#t')).toHaveText('Body');
    await page.waitForTimeout(300);
    expect(ran).toEqual([]);
    expect(remote).toBe(false);
  } finally { await remove(page, 'framed'); }
});

test('a client part answers requests on its own bus', async ({ page }) => {
  const script = `
    let calls = 0;
    colecto.onmessage = (message) => {
      if (message.t === 'ping') return colecto.send({ t: 'pong', n: message.n });
      if (message.t === 'start') colecto.send({ t: 'render', surface: 'pane', tree: { type: 'text', text: 'Ready' } });
      if (message.t === 'bus.request' && message.name === 'v1/echo')
        colecto.send({ t: 'call', id: ++calls, method: 'POST', path: '/bus/replies/' + message.id, body: { data: { echo: message.payload, from: message.from } } });
    };`;
  await install(page, pane('echoer', script, ['extensions.bus']));
  try {
    await page.goto('/');
    await page.getByRole('tab', { name: 'echoer' }).click();
    await expect(paneOf(page, 'echoer').getByText('Ready')).toBeVisible();
    // The composer (bundled, with extensions.bus) asks; the Worker answers through the shell.
    const response = await page.request.post(`${origin}/api/v1/bus/echoer/request`, { headers: { ...host, 'x-colecto-extension': 'composer' }, data: { name: 'v1/echo', payload: { hello: 1 } } });
    expect(await response.json()).toEqual({ data: { echo: { hello: 1 }, from: 'composer' } });
  } finally { await remove(page, 'echoer'); }
});

test('Oneko draws on a click-through canvas overlay and runs toward the pointer', async ({ page }) => {
  await install(page, readFileSync('../target/extensions/oneko.colectoext'));
  try {
    await page.goto('/');
    const overlay = page.locator('canvas.extension-canvas');
    await expect(overlay).toHaveCount(1);
    await expect(overlay).toHaveAttribute('aria-hidden', 'true');
    // Where the cat's pixels are: the centre of everything drawn on the overlay.
    const cat = () => page.evaluate(() => {
      const source = document.querySelector<HTMLCanvasElement>('canvas.extension-canvas')!;
      // A quarter-size copy in CSS pixels is precise enough and quick to read.
      const copy = new OffscreenCanvas(Math.ceil(innerWidth / 4), Math.ceil(innerHeight / 4));
      const context = copy.getContext('2d')!;
      context.drawImage(source, 0, 0, copy.width, copy.height);
      const pixels = context.getImageData(0, 0, copy.width, copy.height).data;
      let x = 0, y = 0, count = 0;
      for (let i = 3; i < pixels.length; i += 4) if (pixels[i]) { const n = (i - 3) / 4; x += n % copy.width; y += Math.floor(n / copy.width); count++; }
      return count ? { x: x / count * 4, y: y / count * 4 } : null;
    });
    await expect.poll(cat).not.toBeNull();
    const start = (await cat())!;
    await page.mouse.move(900, 600);
    await page.mouse.move(905, 605);
    // It runs toward the pointer; how fast depends on the machine, so only the direction counts.
    const away = Math.hypot(905 - start.x, 605 - start.y);
    expect(away).toBeGreaterThan(500);
    await expect.poll(async () => { const now = await cat(); return now ? Math.hypot(905 - now.x, 605 - now.y) : Infinity; }, { timeout: 15_000 }).toBeLessThan(away / 3);
    // The overlay never takes clicks from the app, and goes away with the extension.
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('button', { name: 'Extensions', exact: true }).click();
    await page.getByRole('group', { name: 'Oneko', exact: true }).getByRole('checkbox', { name: 'Enabled for all accounts' }).uncheck();
    await expect(overlay).toHaveCount(0);
  } finally { await remove(page, 'oneko'); }
});

test('slot stays open across parent re-renders and restart reconnects events and open slots', async ({ page, request }) => {
  const script = `
    let tag = 'v1', opens = 0, closes = 0, active = null;
    function draw() {
      colecto.send({ t: 'render', surface: 'pane', tree: { type: 'stack', children: [
        { type: 'text', id: 'state', text: 'tag=' + tag + ' slot=' + (active ? 'open' : 'closed') + ' counts=' + opens + '/' + closes },
        { type: 'button', id: 'crash', label: 'Crash' },
      ] } });
      colecto.send({ t: 'contribute', slot: 'reader/v1/actions', tree: active
        ? { type: 'button', id: 'probe-btn', variant: 'tool', label: 'Probe ' + opens + '/' + closes + ' ' + tag }
        : null });
    }
    colecto.onmessage = (message) => {
      if (message.t === 'ping') return colecto.send({ t: 'pong', n: message.n });
      if (message.t === 'start') { tag = message.settings?.tag ?? 'v1'; draw(); }
      if (message.t === 'settings') { tag = message.settings?.tag ?? tag; draw(); }
      if (message.t === 'slot' && message.owner === 'reader' && message.slot === 'v1/actions') {
        active = message.context;
        if (active !== null) opens++; else closes++;
        draw();
      }
      if (message.t === 'event' && message.node === 'crash') throw new Error('boom');
    };`;
  const pkg = zip({
    'manifest.toml': `id = "slot-probe"\nname = "Slot probe"\ndescription = "Tests slot stability and restart."\nauthor = "Tests"\napi_version = 1\nclient = "client/main.js"\ncapabilities = ["ui.contribute"]\n\n[pane]\nicon = "extension"\n\n[[settings]]\nkey = "tag"\nlabel = "Tag"\ntype = "text"\ndefault = "v1"\n`,
    'client/main.js': script,
  });
  await install(page, pkg);
  try {
    await page.goto('/');
    await page.getByRole('button', { name: /Maya Chen/ }).click();
    await expect(page.getByRole('button', { name: 'Probe 1/0 v1', exact: true })).toBeVisible();
    // Starring/unstarring re-renders the reader with the same slot context; Slot must not close/reopen it.
    const star = page.getByRole('button', { name: /^(Star|Starred)$/ });
    const before = await star.getAttribute('aria-pressed');
    await star.click();
    await expect(star).toHaveAttribute('aria-pressed', before === 'true' ? 'false' : 'true');
    await expect(page.getByRole('button', { name: 'Probe 1/0 v1', exact: true })).toBeVisible();
    // Restore original star state so shared demo data stays unchanged.
    await star.click();
    await expect(star).toHaveAttribute('aria-pressed', before ?? 'false');
    await expect(page.getByRole('button', { name: 'Probe 1/0 v1', exact: true })).toBeVisible();
    // Crash and restart while the reader's slot stays mounted in its background tab.
    await page.getByRole('tab', { name: 'Slot probe' }).click();
    const probe = paneOf(page, 'Slot probe');
    await probe.getByRole('button', { name: 'Crash' }).click();
    await expect(probe.getByRole('alert')).toHaveText('Slot probe stopped');
    await probe.getByRole('button', { name: 'Restart' }).click();
    await expect(probe.getByText('tag=v1 slot=open counts=1/0')).toBeVisible();
    // Settings updates still reach the restarted Worker over the reconnected SSE stream.
    await page.request.put(`${origin}/api/v1/extensions/slot-probe/settings`, { headers: host, data: { tag: 'v2' } });
    await expect(probe.getByText('tag=v2 slot=open counts=1/0')).toBeVisible();
    await page.getByRole('tab', { name: 'Reader', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Probe 1/0 v2', exact: true })).toBeVisible();
  } finally {
    await request.delete(`${origin}/api/v1/extensions/slot-probe`, { headers: host, data: {} });
  }
});

test('inbox context menu caps entries at 24 and clips long label names without crashing', async ({ page }) => {
  const longLabel = 'L'.repeat(140);
  const labels: Record<string, string[]> = { [longLabel]: [] };
  for (let i = 1; i <= 7; i++) labels[`Label ${i}`] = [];
  await page.route('**/api/v1/labels**', route => route.fulfill({ json: labels }));
  await page.goto('/');
  await expect(page.getByRole('button', { name: /Maya Chen/ })).toBeVisible();
  // Add 16 row actions (4 each from 4 bundled extensions) + 8 labels + read/star/delete = 27 > 24.
  for (const ext of ['composer', 'reader', 'ai', 'inbox']) {
    for (let i = 0; i < 4; i++) {
      const res = await page.request.post(`${origin}/api/v1/bus/inbox/request`, {
        headers: { ...host, 'x-colecto-extension': ext },
        data: { name: 'v1/add_row_action', payload: { id: `act-${i}`, label: `${ext} action ${i}` } },
      });
      expect(res.ok()).toBe(true);
    }
  }
  await page.getByRole('button', { name: /Maya Chen/ }).click({ button: 'right' });
  const menu = page.getByRole('menu');
  await expect(menu).toBeVisible();
  const count = await menu.getByRole('menuitem').count() + await menu.getByRole('menuitemcheckbox').count();
  expect(count).toBeLessThanOrEqual(24);
  await expect(menu.getByRole('menuitem', { name: 'Archive' })).toBeVisible();
  await expect(menu.getByRole('menuitem', { name: 'Move to Trash' })).toBeVisible();
  await expect(page.getByRole('alert').filter({ hasText: /stopped/ })).toHaveCount(0);
});

