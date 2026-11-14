// Every running client part, and the one Host event stream they share
// (docs/extensions/ui-model.md, "Event stream"). The Host picks each event's recipients by
// their credentials; this only delivers it.
import { extensionHost, openEvents, type ExtensionState } from '../lib/api';
import { ExtensionRuntime, type AccountSummary } from './runtime.svelte';
import { currentTheme, type Theme } from './theme';

class Runtimes {
  list = $state<ExtensionRuntime[]>([]);
  #account: AccountSummary | null = null;
  #theme: Theme | null = null;
  #stopEvents = () => {};
  #tokens = '';

  get(id: string) { return this.list.find(runtime => runtime.id === id); }

  /** Start client parts that were turned on or installed, stop the ones that went away. */
  sync(extensions: ExtensionState[]) {
    const wanted = extensions.filter(extension => extension.enabled && extension.manifest.client && !extension.bundled);
    for (const runtime of this.list.filter(runtime => !wanted.some(extension => extension.manifest.id === runtime.id))) runtime.stop();
    const kept = this.list.filter(runtime => wanted.some(extension => extension.manifest.id === runtime.id));
    for (const extension of wanted.filter(extension => !kept.some(runtime => runtime.id === extension.manifest.id))) {
      const runtime = new ExtensionRuntime(extension, () => ({ account: this.#account, theme: this.#theme ??= currentTheme() }));
      kept.push(runtime);
      void runtime.start().then(() => this.#listen());
    }
    this.list = kept;
    this.#listen();
  }

  setAccount(account: AccountSummary | null) {
    if (JSON.stringify(account) === JSON.stringify(this.#account)) return;
    this.#account = account;
    for (const runtime of this.list) runtime.send({ t: 'account', account });
  }

  /** Call after Appearance changes have been applied to the page. */
  refreshTheme() {
    const theme = currentTheme();
    if (JSON.stringify(theme) === JSON.stringify(this.#theme)) return;
    this.#theme = theme;
    for (const runtime of this.list) runtime.send({ t: 'theme', theme });
  }

  stopAll() {
    for (const runtime of this.list) runtime.stop();
    this.list = [];
    this.#listen();
  }

  /** One stream for all running client parts, reopened when the set of credentials changes. */
  #listen() {
    const running = this.list.filter(runtime => runtime.token);
    const tokens = running.map(runtime => runtime.token);
    if (tokens.join() === this.#tokens) return;
    this.#tokens = tokens.join();
    this.#stopEvents();
    this.#stopEvents = () => {};
    if (!tokens.length) return;
    this.#stopEvents = openEvents((name, data) => {
      const { to, data: payload } = (data ?? {}) as { to?: string[]; data?: Record<string, unknown> };
      for (const id of to ?? []) {
        const runtime = this.get(id);
        if (!runtime) continue;
        if (name === 'bus.request' || name === 'bus.event') runtime.send({ t: name, ...payload });
        else if (name === 'extension.settings') void extensionHost.settings(id).then(view => runtime.send({ t: 'settings', settings: view.values })).catch(() => {});
        else runtime.send({ t: 'host', name, data: payload });
      }
    }, '/extensions/events', { tokens });
  }
}

export const runtimes = new Runtimes();
if (typeof window !== 'undefined') window.addEventListener('pagehide', () => runtimes.stopAll());
