import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { createRequire } from 'node:module';
import { createInterface } from 'node:readline';
const require = createRequire(new URL('../ui/package.json', import.meta.url));
const { chromium } = require('playwright');
const [base, cycles] = process.argv.slice(2);
const input = createInterface({ input: process.stdin });
const lines = input[Symbol.asyncIterator]();
const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
process.once('SIGTERM', async () => { await browser.close(); process.exit(1); });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 850 } });
  const ownerToken = readFileSync(join(process.env.COLECTO_DATA_DIR, 'owner-token'), 'utf8').trim();
  await page.addInitScript(({ origin, token }) => { if (window.location.origin === origin) sessionStorage.setItem('colecto.token', token); }, { origin: base, token: ownerToken });
  await page.goto(base);
  await page.getByRole('button', { name: /Maya Chen/ }).waitFor();
  console.log('READY');
  if ((await lines.next()).value !== 'RUN') throw Error('Expected RUN');
  for (let cycle = 0; cycle < Number(cycles); cycle++) {
    await page.getByRole('button', { name: /^Inbox/ }).click();
    await page.getByRole('button', { name: /Maya Chen/ }).click();
    await page.getByRole('heading', { name: 'A small plan for the weekend' }).waitFor();
    await page.getByRole('button', { name: 'Compose', exact: true }).click();
    await page.getByLabel('To', { exact: true }).fill('benchmark@example.test');
    await page.getByLabel('Subject', { exact: true }).fill(`Resource sample ${cycle}`);
    await page.getByLabel('Message', { exact: true }).fill('Sample mail for repeatable resource measurements.\n'.repeat(20));
    await page.getByRole('button', { name: 'Save draft', exact: true }).click();
    await page.getByRole('status').filter({ hasText: 'Draft saved' }).waitFor();
  }
  await page.getByRole('button', { name: /^Inbox/ }).click();
  console.log('EXERCISED');
  if ((await lines.next()).value !== 'QUIT') throw Error('Expected QUIT');
} finally {
  await browser.close();
  input.close();
}
