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
if (import.meta.env.DEV && location.pathname === '/__stress') {
  void import('./StressPreview.svelte').then(({ default: Preview }) => mount(Preview, { target: document.getElementById('app')! }));
} else {
  mount(App, { target: document.getElementById('app')! });
}
