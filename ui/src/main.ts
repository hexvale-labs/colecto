import { installLayers } from './lib/layers';
import { installBack } from './lib/back';
import { installPressFeedback, platform } from './lib/native';
import { installZoom } from './lib/zoom';
installLayers();
installBack();
installPressFeedback();
installZoom();
// Motion follows the platform's conventions (see lib/back.ts); styles can too.
document.documentElement.dataset.platform = platform;
// Overlay scrollbars take no width. WebKitGTK fades them on a layer above everything else,
// so one in the app showed through the startup splash, Settings and dialogs (#96); style.css
// hides them while one of those covers the app. Classic scrollbars stay, because hiding them
// would reflow the page behind.
{
  const probe = document.createElement('div');
  probe.style.cssText = 'position:absolute;top:-999px;width:50px;height:50px;overflow:scroll';
  document.documentElement.append(probe);
  if (probe.offsetWidth === probe.clientWidth) document.documentElement.dataset.overlayScrollbars = '';
  probe.remove();
}
import { mount } from 'svelte';
import App from './App.svelte';
import '@fontsource-variable/mona-sans/wdth.css';
import './style.css';
// The browser's own menu (Back, Reload, Inspect) means nothing in an app. It stays only where it
// helps: text fields, selectable text and links. Rows with a menu of their own open that (#95).
document.addEventListener('contextmenu', event => {
  const target = event.target instanceof Element ? event.target : null;
  const selected = !!window.getSelection()?.toString();
  if (!selected && !target?.closest('input, textarea, [contenteditable], a[href], .selectable, .letter')) event.preventDefault();
});
// Images, icons and links are part of the interface, not things to drag elsewhere. Selected text still drags.
document.addEventListener('dragstart', event => {
  if (event.target instanceof Element && !event.target.closest('[draggable="true"], input, textarea')) event.preventDefault();
});
declare global { interface Window { colectoStartupPaint: Promise<number> } }
// The splash in index.html must actually reach the screen before Svelte replaces anything: two
// frames guarantee one paint. It then stays at least 800 ms and fades out over 200 ms, so a fast
// start never flashes. Playwright sets `colecto.fastStartup` to skip the wait.
const startupPaint = window.colectoStartupPaint = new Promise<number>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now()))));
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
/** Resolves once the app has rendered something other than its own "Loading extensions" status. */
function appReady(app: HTMLElement) {
  const ready = () => app.childElementCount > 0 && !app.querySelector('.boot[role="status"]');
  return new Promise<void>(resolve => {
    if (ready()) return resolve();
    const observer = new MutationObserver(() => { if (ready()) { observer.disconnect(); resolve(); } });
    observer.observe(app, { childList: true, subtree: true });
  });
}
async function revealApp() {
  const app = document.getElementById('app')!;
  const startup = document.getElementById('startup');
  const fast = navigator.webdriver && sessionStorage.getItem('colecto.fastStartup') === '1';
  const paintedAt = await startupPaint;
  await Promise.all([sleep(fast ? 0 : 800 - (performance.now() - paintedAt)), appReady(app)]);
  app.inert = false;
  // Clicks reach the app while the splash fades.
  if (startup) startup.style.pointerEvents = 'none';
  if (startup && !fast && !matchMedia('(prefers-reduced-motion: reduce)').matches) {
    await startup.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 200, easing: 'cubic-bezier(0.23, 1, 0.32, 1)', fill: 'forwards' }).finished;
  }
  startup?.remove();
}
void revealApp();
await startupPaint;
mount(App, { target: document.getElementById('app')! });
