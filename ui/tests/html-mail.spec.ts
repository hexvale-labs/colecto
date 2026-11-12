import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import { templates } from './fixtures/html-mail';

// Real, published company templates (see fixtures/html-mail/README.md). Everything is served
// locally: images are data: URIs and every remote request is intercepted, so timings measure
// the reader, not the network.
// Median of three cold opens must stay under `budget`; one slow open on a busy two-core
// CI machine is scheduler noise. Any single open over `ceiling` is a real regression.
const budget = 400;
const ceiling = 1000;

async function serve(page: Page, html: string, sender: string, subject: string) {
  const message = { id: 'template', sender, address: 'mail@example.test', subject, body: 'Plain text part', date: 'Today' };
  const content = { text: 'Plain text part', html, attachments: [], message_id: '', references: '', reply_to: '', to: '', cc: '' };
  await page.route('**/api/v1/messages**', route => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith('/cache') || path.endsWith('/content')) return route.fallback();
    return route.fulfill({ json: path.endsWith('/refresh') ? [message] : message });
  });
  // Answer the opened message from inside the page. A Playwright route relays every response
  // through the test process, which adds tens of milliseconds the app never pays.
  await page.addInitScript(({ message, content }) => {
    const fetch = window.fetch.bind(window);
    window.fetch = (input, init) => {
      const path = new URL(input instanceof Request ? input.url : String(input), location.href).pathname;
      const reply = path.endsWith('/messages/template/content') ? content : path.endsWith('/messages/template') ? message : null;
      return reply ? Promise.resolve(Response.json(reply)) : fetch(input, init);
    };
  }, { message, content });
  await page.goto('/');
  return page.getByRole('button', { name: new RegExp(subject.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')) });
}

/**
 * Record, inside the page, the time from the click to the first frame painted after the
 * mail frame's `load` event. `load` fires once the document and all its inline images are
 * decoded (blocked remote images fail at once), so this is "fully shown". Timing in the
 * page keeps Playwright's own polling and IPC out of the measurement.
 */
async function timeOpening(page: Page) {
  await page.evaluate(() => {
    const timing = (window as unknown as { mailTiming: Promise<number> });
    timing.mailTiming = new Promise(resolve => {
      let clicked = 0;
      document.addEventListener('click', event => { clicked ||= event.timeStamp; }, { capture: true, once: true });
      const watch = new MutationObserver(() => {
        const frame = document.querySelector('iframe[title="Message content"]');
        if (!frame) return;
        watch.disconnect();
        frame.addEventListener('load', () => requestAnimationFrame(() => resolve(performance.now() - clicked)), { once: true });
      });
      watch.observe(document.body, { subtree: true, childList: true });
    });
  });
  return () => page.evaluate(() => (window as unknown as { mailTiming: Promise<number> }).mailTiming);
}

/** Wait until the body text is visible and every inline image in the mail has decoded. */
async function painted(page: Page, text: string) {
  const frame = page.frameLocator('iframe[title="Message content"]');
  await expect(frame.getByText(text, { exact: false }).first()).toBeVisible({ timeout: 5000 });
  await expect.poll(() => frame.locator('img[src^="data:"]').evaluateAll(images =>
    images.length > 0 && images.every(image => (image as HTMLImageElement).complete && (image as HTMLImageElement).naturalWidth > 0)), { intervals: [10] }).toBe(true);
  return frame;
}

for (const template of templates) {
  test(`${template.name}: shows text, images and a working button within ${budget} ms (median)`, async ({ page }) => {
    const remote: string[] = [];
    await page.route('https://remote.test/**', route => { remote.push(route.request().url()); return route.abort(); });
    const open = await serve(page, template.html, template.sender, template.subject);
    // The first open in a newly launched test browser includes engine start-up work, so it
    // is not timed. Each timed open then starts from a freshly loaded app.
    await open.click();
    await painted(page, template.text);
    const runs: number[] = [];
    for (let run = 0; run < 3; run++) {
      await page.reload();
      await open.waitFor();
      const elapsed = await timeOpening(page);
      await open.click();
      await painted(page, template.text);
      runs.push(Math.round(await elapsed()));
    }
    const median = [...runs].sort((a, b) => a - b)[1];
    test.info().annotations.push({ type: 'open-ms', description: runs.join(' ') });
    expect(median, `${template.name} opened in ${runs.join(', ')} ms`).toBeLessThan(budget);
    expect(Math.max(...runs), `${template.name} opened in ${runs.join(', ')} ms`).toBeLessThan(ceiling);
    const frame = page.frameLocator('iframe[title="Message content"]');

    const iframe = page.locator('iframe[title="Message content"]');
    await expect(iframe).toHaveCSS('opacity', '1');
    expect((await iframe.boundingBox())!.height).toBeGreaterThan(300);
    // Remote images stay blocked until the owner asks for them.
    expect(remote).toEqual([]);

    // Buttons in mail are links; they open outside the app, never inside the frame.
    const button = frame.getByRole('link', { name: template.button.name }).first();
    await expect(button).toHaveAttribute('href', template.button.href);
    await page.context().route(/^https?:\/\/(?!127\.0\.0\.1)/, route => route.fulfill({ body: 'opened' }));
    const [popup] = await Promise.all([page.waitForEvent('popup'), button.click()]);
    expect(popup.url()).toBe(new URL(template.button.href).href);
    await popup.close();
    await expect(frame.getByText(template.text, { exact: false }).first()).toBeVisible();
  });
}

test('mail shows straight away while allowed remote images are still loading', async ({ page }) => {
  // The frame used to stay invisible until its load event, which waits for every image.
  // A slow or dead image server then hid the whole mail.
  await page.route('**/api/v1/preferences', async route => {
    if (route.request().method() !== 'GET') return route.fallback();
    const response = await route.fetch();
    return route.fulfill({ response, json: { ...await response.json(), remote_images: true } });
  });
  let requested = 0;
  await page.route('https://remote.test/**', () => { requested++; });
  const template = templates[0];
  const open = await serve(page, template.html, template.sender, template.subject);
  await open.click();
  // The images never answer, so the frame never fires `load`; the mail must show regardless.
  const frame = page.frameLocator('iframe[title="Message content"]');
  await expect(frame.getByText(template.text, { exact: false }).first()).toBeVisible({ timeout: 2000 });
  await expect(page.locator('iframe[title="Message content"]')).toHaveCSS('opacity', '1');
  await expect.poll(() => requested).toBeGreaterThan(0);
});
