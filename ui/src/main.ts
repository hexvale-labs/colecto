import { mount } from 'svelte';
import App from './App.svelte';
import './style.css';
import './lib/theme.svelte';
mount(App, { target: document.getElementById('app')! });
