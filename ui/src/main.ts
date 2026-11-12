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
// The static overlay survives module loading; let it paint before mounting Svelte.
const startupPaint = window.colectoStartupPaint = new Promise<number>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now()))));
async function revealApp() {
  const paintedAt = await startupPaint;
  const fast = navigator.webdriver && sessionStorage.getItem('colecto.fastStartup') === '1';
  await new Promise(resolve => setTimeout(resolve, Math.max(0, Math.ceil((fast ? 0 : 800) - (performance.now() - paintedAt)))));
  // Keep the overlay until the extension bootstrap has produced a usable surface.
  while (document.querySelector('#app .boot[role="status"]') || !document.querySelector('#app > *')) {
    await new Promise(resolve => setTimeout(resolve, 16));
  }
  const startup = document.getElementById('startup');
  if (!fast && !matchMedia('(prefers-reduced-motion: reduce)').matches && startup) {
    await startup.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 200, easing: 'ease-out', fill: 'forwards' }).finished;
  }
  startup?.remove();
  document.getElementById('app')!.inert = false;
}
void revealApp();
await startupPaint;
if (import.meta.env.DEV && location.pathname === '/__stress') {
  void import('./StressPreview.svelte').then(({ default: Preview }) => mount(Preview, { target: document.getElementById('app')! }));
} else {
  mount(App, { target: document.getElementById('app')! });
}
