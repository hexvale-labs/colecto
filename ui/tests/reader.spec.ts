import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';

/** Serve one message with this HTML body and open it in the reader. */
async function openHtml(page: Page, html: string, subject = 'Welcome to Purelymail!') {
  const message = { id: 'html-fixture', sender: 'support@purelymail.com', address: 'support@purelymail.com', subject, body: 'Hi there', date: 'Today' };
  await page.route('**/api/v1/messages**', route => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith('/html-fixture/content')) return route.fulfill({ json: { text: 'Plain text part', html, attachments: [], message_id: '', references: '', reply_to: '', to: '', cc: '' } });
    if (path.endsWith('/cache') || path.endsWith('/content')) return route.fallback();
    return route.fulfill({ json: path.endsWith('/refresh') ? [message] : message });
  });
  await page.goto('/');
  await page.getByRole('button', { name: /support@purelymail.com/ }).click();
  const frame = page.locator('iframe[title="Message content"]');
  // The frame fades in only after it has painted; a frame that never loads stays invisible.
  await expect(frame).toHaveCSS('opacity', '1');
  expect((await frame.boundingBox())!.height).toBeGreaterThan(300);
  return page.frameLocator('iframe[title="Message content"]');
}

// Roundcube-style mail, as Purelymail sends it: a full document with a doctype, head and a styled body.
const roundcube = `<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN"><html><head><meta http-equiv="Content-Type" content="text/html; charset=UTF-8" /></head><body style='font-size: 10pt; font-family: Verdana,Geneva,sans-serif'>
<p>Hi contact@natix.dev! We hope you enjoy your new Purelymail user account.</p>
<p>You might want to check out our <a href="https://purelymail.com/docs/deviceSetup">setup guide</a>, or learn how to <a href="https://purelymail.com/docs/mailTransfer">import emails</a> to your new user account.</p>
<p><a href="https://inbox.purelymail.com">Webmail</a></p><p>&nbsp;</p><p>Cheers,</p><p>The Purelymail Team</p></body></html>`;

test('renders a full Roundcube-style document in the dark reading colours with working links', async ({ page }) => {
  const frame = await openHtml(page, roundcube);
  const intro = frame.getByText('We hope you enjoy your new Purelymail user account.');
  await expect(intro).toBeVisible();
  // Light text on the dark reader: the frame's own stylesheet applied.
  await expect(intro).toHaveCSS('color', 'rgb(213, 221, 215)');
  await expect(frame.getByRole('link', { name: 'setup guide' })).toHaveAttribute('href', 'https://purelymail.com/docs/deviceSetup');
  await expect(frame.getByRole('link', { name: 'Webmail' })).toHaveCSS('color', 'rgb(162, 214, 177)');
  await expect(frame.getByText('The Purelymail Team')).toBeVisible();
});

test('mail that sets its own dark text colour keeps a light sheet so it stays readable', async ({ page }) => {
  const frame = await openHtml(page, '<div style="color:#000000;font-family:Arial">Dark ink on purpose</div><font color="#333">Old-style font colour</font>', 'Coloured text');
  await expect(frame.getByText('Dark ink on purpose')).toHaveCSS('color', 'rgb(0, 0, 0)');
  await expect(frame.locator('body')).toHaveCSS('background-color', 'rgb(255, 255, 255)');
});

test('hostile mail cannot load remote styles, refresh away, or submit forms', async ({ page }) => {
  const remote: string[] = [];
  const windows: string[] = [];
  page.context().on('page', opened => windows.push(opened.url()));
  await page.route('https://remote.test/**', route => { remote.push(route.request().url()); return route.abort(); });
  const frame = await openHtml(page, `<meta http-equiv="refresh" content="0;url=https://remote.test/away"><link rel="stylesheet" href="https://remote.test/x.css"><style>@import url(https://remote.test/i.css); p { background: url(https://remote.test/bg.png) }</style>
<p>Still here</p><form action="https://remote.test/post"><button>Claim prize</button></form><a href="javascript:alert(1)">Click</a>`, 'Totally safe');
  await expect(frame.getByText('Still here')).toBeVisible();
  await frame.getByRole('button', { name: 'Claim prize' }).click();
  await frame.getByText('Click').click();
  await page.waitForTimeout(300);
  await expect(frame.getByText('Still here')).toBeVisible();
  expect(remote).toEqual([]);
  expect(windows).toEqual([]);
});


test('renders HTML in a script-free sandbox, gates remote images, and prefills replies', async ({ page }) => {
  const html = '<p>Hello <b>formatted</b> world</p><img alt="pixel" src="https://remote.test/pixel.png"><script>document.body.innerHTML = "HACKED"</script>';
  const message = { id: 'html-mail', sender: 'The Colecto team', address: 'hello@colecto.example', subject: 'A quieter place for your mail', body: 'Hello formatted world', date: 'Today' };
  await page.route('**/api/v1/messages**', route => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith('/cache') || path.endsWith('/content')) return route.fallback();
    return route.fulfill({ json: path.endsWith('/refresh') ? [message] : message });
  });
  await page.route('**/api/v1/messages/html-mail/content', route => route.fulfill({ json: { text: 'Hello formatted world', html, attachments: [{ index: 0, name: 'report.pdf', content_type: 'application/pdf', size: 2048 }], message_id: '<w1@colecto.example>', references: '', reply_to: '', to: 'demo@example.test', cc: '' } }));
  const remote: string[] = [];
  await page.route('https://remote.test/**', route => { remote.push(route.request().url()); return route.abort(); });
  await page.goto('/');
  await page.getByRole('button', { name: /The Colecto team/ }).click();
  const frame = page.frameLocator('iframe[title="Message content"]');
  await expect(frame.getByText('Hello formatted world')).toBeVisible();
  await expect(frame.getByText('HACKED')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Download report.pdf' })).toContainText('2 KB');
  expect(remote).toEqual([]);
  await page.getByRole('button', { name: 'Load remote images' }).click();
  await expect.poll(() => remote.length).toBeGreaterThan(0);
  await page.getByRole('button', { name: 'Show plain text' }).click();
  await expect(page.locator('.letter-body')).toHaveText('Hello formatted world');
  await page.getByRole('button', { name: 'Reply', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Reply', exact: true })).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'To', exact: true })).toHaveValue('The Colecto team <hello@colecto.example>');
  await expect(page.getByRole('textbox', { name: 'Subject', exact: true })).toHaveValue('Re: A quieter place for your mail');
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toHaveValue(/> Hello formatted world/);
});

test('keeps designed mail on its light sheet even when it ships dark-mode rules', async ({ page }) => {
  const html = '<style>@media (prefers-color-scheme: dark) { .copy { color: #fff !important } }</style><table bgcolor="#e1f2e8"><tr><td class="copy" style="color:#222">Welcome aboard</td></tr></table>';
  const message = { id: 'designed', sender: 'Support', address: 'support@example.test', subject: 'Welcome aboard', body: 'Welcome aboard', date: 'Today' };
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.route('**/api/v1/messages**', route => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith('/designed/content')) return route.fulfill({ json: { text: 'Welcome aboard', html, attachments: [], message_id: '', references: '', reply_to: '', to: '', cc: '' } });
    if (path.endsWith('/cache') || path.endsWith('/content')) return route.fallback();
    return route.fulfill({ json: path.endsWith('/refresh') ? [message] : message });
  });
  await page.goto('/');
  await page.getByRole('button', { name: /Support/ }).click();
  const cell = page.frameLocator('iframe[title="Message content"]').getByText('Welcome aboard');
  await expect(cell).toHaveCSS('color', 'rgb(34, 34, 34)');
});
