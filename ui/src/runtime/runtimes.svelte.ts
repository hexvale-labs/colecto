// Every running client part, and the one Host event stream they share
// (docs/extensions/ui-model.md, "Event stream"). The Host picks each event's recipients by
// their credentials; this only delivers it.
import { extensionHost, openEvents, type ExtensionState } from '../lib/api';
import { ExtensionRuntime, type AccountSummary, type ShellActions } from './runtime.svelte';
import { currentTheme, type Theme } from './theme';

class Runtimes {
  list = $state<ExtensionRuntime[]>([]);
  #account: AccountSummary | null = null;
  #accounts: AccountSummary[] = [];
  shell: ShellActions | null = null;
  #theme: Theme | null = null;
  #stopEvents = () => {};
  #tokens = '';

  #slots = new Map<string, unknown>();
  /** App actions drawn inside a pane (`action_ref`), which the top bar then leaves out. */
  refs = $state<Record<string, number>>({});
  /** Narrow windows show one pane at a time and lift bottom navigation out of panes. */
  compact = $state(false);
  /** False until the shell knows the account; app actions wait for it. */
  ready = $state(false);
  /** Reopen the event stream when a credential was renewed. */
  relisten() { this.#listen(); }

  get(id: string) { return this.list.find(runtime => runtime.id === id); }

  /** Whether any surface reported busy work, such as a draft being saved. */
  get busy() { return this.list.some(runtime => Object.values(runtime.states).some(state => state.busy)); }
  get dirty() { return this.list.some(runtime => Object.values(runtime.states).some(state => state.dirty)); }

  /** A slot appeared, changed context or went away (`null`); contributors hear about it. */
  openSlot(full: string, context: unknown) {
    if (context === null) this.#slots.delete(full); else this.#slots.set(full, context);
    for (const runtime of this.list) this.#tellSlot(runtime, full, context);
  }
  #tellSlot(runtime: ExtensionRuntime, full: string, context: unknown) {
    const [owner, ...rest] = full.split('/');
    if (runtime.capabilities.includes('ui.contribute') && runtime.id !== owner) runtime.send({ t: 'slot', slot: rest.join('/'), owner, context });
  }

  showRef(key: string, visible: boolean) { this.refs[key] = Math.max(0, (this.refs[key] ?? 0) + (visible ? 1 : -1)); }

  /** Ask every extension that searches this scope; each answers with `command_hits`. */
  query(scope: string, seq: number, query: string) {
    for (const runtime of this.list) if (runtime.searchScope === scope) runtime.send({ t: 'command_query', seq, query });
  }

  /** Start client parts that were turned on or installed, stop the ones that went away. */
  sync(extensions: ExtensionState[]) {
    const wanted = extensions.filter(extension => extension.enabled && extension.manifest.client);
    for (const runtime of this.list.filter(runtime => !wanted.some(extension => extension.manifest.id === runtime.id))) runtime.stop();
    const kept = this.list.filter(runtime => wanted.some(extension => extension.manifest.id === runtime.id));
    for (const extension of wanted.filter(extension => !kept.some(runtime => runtime.id === extension.manifest.id))) {
      const runtime = new ExtensionRuntime(extension, () => ({ account: this.#account, accounts: this.#accounts, theme: this.#theme ??= currentTheme(), shell: this.shell }));
      kept.push(runtime);
      void runtime.start().then(() => {
        this.#listen();
        for (const [full, context] of this.#slots) this.#tellSlot(runtime, full, context);
      });
    }
    this.list = kept;
    this.#listen();
  }

  /** The selected account and every account that is on. */
  setAccount(account: AccountSummary | null, accounts: AccountSummary[]) {
    if (JSON.stringify([account, accounts]) === JSON.stringify([this.#account, this.#accounts])) return;
    this.#account = account; this.#accounts = accounts;
    for (const runtime of this.list) runtime.send({ t: 'account', account, ...runtime.accounts(accounts) });
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
