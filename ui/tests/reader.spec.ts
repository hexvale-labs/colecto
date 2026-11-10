import { test, expect } from './fixtures';

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
