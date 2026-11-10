import { mount } from 'svelte';
import App from './App.svelte';
import '@fontsource-variable/mona-sans/wdth.css';
import './style.css';
if (import.meta.env.DEV && location.pathname === '/__stress') {
  void import('./StressPreview.svelte').then(({ default: Preview }) => mount(Preview, { target: document.getElementById('app')! }));
} else {
  mount(App, { target: document.getElementById('app')! });
}
