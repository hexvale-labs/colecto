import { installLayers } from './lib/layers';
installLayers();
import { mount } from 'svelte';
import App from './App.svelte';
import '@fontsource-variable/mona-sans/wdth.css';
import './style.css';
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
if (import.meta.env.DEV && location.pathname === '/__stress') {
  void import('./StressPreview.svelte').then(({ default: Preview }) => mount(Preview, { target: document.getElementById('app')! }));
} else {
  mount(App, { target: document.getElementById('app')! });
}
