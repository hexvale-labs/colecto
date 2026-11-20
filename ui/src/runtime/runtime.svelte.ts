// The shell's side of one extension's client part (docs/extensions/ui-model.md, "Client
// runtime" and "Credentials"): it holds the extension's credential, starts its runtime frame
// and Worker, checks every message, answers API calls and keeps the surfaces the renderer
// draws. The credential and the owner token never cross into the frame or the Worker.
import { callFailure, extensionHost, ServiceError, type ExtensionState } from '../lib/api';
import { checkOutgoing, checkTree, LIMITS, ProtocolError, type MenuItem, type Node, type Patch, type ToastAction } from './protocol';
import type { Theme } from './theme';
import { dismissKey, notify } from '../shell/toast-store.svelte';
import type { IconName } from '../Icon.svelte';

export interface AccountSummary { id: string; name: string; address: string; connected: boolean; configured: boolean }
export type Status = 'starting' | 'running' | 'stopped';

/** What the shell does for its runtimes: the actions only trusted UI may take. */
export interface ShellActions {
  /** Open a destination such as `inbox` or `settings/send-approvals`, after selecting `account`. */
  navigate(target: string, account?: string): void;
  /** Reject a send request, as the owner, and report the result. */
  rejectSend(id: string): void;
  /** Open Settings → Send approvals at one request. */
  reviewSend(id: string): void;
  /** Also show a desktop notification, if the owner allowed them and Colecto is in the background. */
  desktop(title: string, body: string, tag: string): void;
  /** Bring one of its surfaces into view (its pane), for example after opening a message. */
  show(id: string, surface: string): void;
  /** Its pane is done for now (a back button): on narrow windows, show the mailbox again. */
  leave(id: string, surface: string): void;
  /** A credential was renewed; the shared event stream must carry the new one. */
  credentialsChanged(): void;
}
export interface SurfaceState { dirty: boolean; busy: boolean; back: boolean }
export interface Command { id: string; name: string; disabled?: boolean }
export interface AppAction { id: string; label: string; icon?: IconName; shortcut?: string; disabled?: boolean }
export interface RuntimeContext { account: AccountSummary | null; accounts: AccountSummary[]; theme: Theme; shell: ShellActions | null }

/** Where runtime frames live: one hidden container for the whole app. */
function frames() {
  let host = document.getElementById('extension-runtimes');
  if (!host) {
    host = Object.assign(document.createElement('div'), { id: 'extension-runtimes', hidden: true });
    host.setAttribute('aria-hidden', 'true');
    document.body.append(host);
  }
  return host;
}

/** Shell messages that only follow something the owner did, like pressing a button in a surface. */
const OWNER_ACTIONS = new Set(['command', 'command_submit', 'app_action', 'toast_action', 'back']);

export class ExtensionRuntime {
  readonly id: string;
  readonly name: string;
  readonly surfaces: string[];
  status = $state<Status>('starting');
  /** Why it stopped, in words for the owner. */
  reason = $state('');
  trees = $state<Record<string, Node | null>>({});
  /** What each surface reported with `state`. */
  states = $state<Record<string, SurfaceState>>({});
  dialog = $state<Node | null>(null);
  /** A row's context menu: asked for with a `context` event, filled by the extension's `menu`.
   *  A menu with `choose` is the renderer's own (a component's built-in actions). */
  menu = $state<{ surface: string; node: string; key: string; x: number; y: number; items: MenuItem[] | null; label?: string; choose?: (item: string) => void } | null>(null);
  /** Its contributions to other extensions' slots, by full slot name (`reader/v1/actions`). */
  contributions = $state<Record<string, Node | null>>({});
  commands = $state<Command[]>([]);
  appActions = $state<AppAction[]>([]);
  /** The command runner scope it searches (`command_search`), and its newest hits. */
  searchScope = $state('');
  hits = $state<{ seq: number; hits: { id: string; title: string; hint?: string }[] }>({ seq: 0, hits: [] });
  #lastEvent = 0;
  #rendered = new Set<string>();
  #manifest: ExtensionState['manifest'];
  #context: () => RuntimeContext;
  #onStarted?: () => void;
  readonly bundled: boolean;
  readonly capabilities: readonly string[];
  #token = '';
  #frame: HTMLIFrameElement | null = null;
  #port: MessagePort | null = null;
  #pending = 0;
  #watchdog: ReturnType<typeof setInterval> | undefined;
  #pinged = 0;
  #answered = 0;
  #generation = 0;
  #onCrash = (event: MessageEvent) => { if (event.source === this.#frame?.contentWindow && event.data?.t === 'colecto.crash') this.stop(`${this.name} stopped`); };

  constructor(extension: ExtensionState, context: () => RuntimeContext, onStarted?: () => void) {
    this.#manifest = extension.manifest;
    this.bundled = extension.bundled;
    this.capabilities = extension.manifest.capabilities;
    this.id = extension.manifest.id;
    this.name = extension.manifest.name;
    this.surfaces = [...(extension.manifest.pane ? ['pane', 'pane.header'] : []), ...(extension.manifest.settings_page ? ['settings'] : []), 'dialog'];
    this.#context = context;
    this.#onStarted = onStarted;
  }

  /** The credential, for the shared event stream; it never leaves the shell. */
  get token() { return this.status === 'running' ? this.#token : ''; }

  async start() {
    const generation = ++this.#generation;
    this.status = 'starting'; this.reason = ''; this.trees = {};
    try {
      const entry = this.#manifest.client!;
      const [token, files, settings] = await Promise.all([
        extensionHost.credential(this.id),
        this.bundled ? bundledFiles(this.id) : extensionHost.clientFiles(this.id),
        extensionHost.settings(this.id).then(view => view.values).catch(() => ({})),
      ]);
      if (generation !== this.#generation) { void extensionHost.revokeCredential(this.id, token); return; }
      this.#token = token;
      const channel = new MessageChannel();
      this.#port = channel.port1;
      this.#port.onmessage = event => this.#receive(event.data);
      const frame = Object.assign(document.createElement('iframe'), { title: `${this.name} runtime`, src: '/extension-runtime.html' });
      // No allow-same-origin: the frame and its Worker get an opaque origin.
      frame.setAttribute('sandbox', 'allow-scripts');
      this.#frame = frame;
      window.addEventListener('message', this.#onCrash);
      await new Promise<void>((resolve, reject) => {
        frame.onload = () => resolve();
        frame.onerror = () => reject(new Error('The extension runtime did not load'));
        frames().append(frame);
      });
      if (generation !== this.#generation) return;
      const buffers = Object.values(files);
      frame.contentWindow!.postMessage({ t: 'colecto.boot', files, entry, port: channel.port2 }, '*', [channel.port2, ...buffers]);
      const { account, accounts, theme } = this.#context();
      this.status = 'running';
      // `now` lets the client part agree with the window's clock, for dates such as "Today".
      this.send({ t: 'start', api: 1, id: this.id, theme, account, ...this.accounts(accounts), settings, surfaces: this.surfaces, now: Date.now() });
      this.#pinged = this.#answered = 0;
      this.#watchdog = setInterval(() => this.#ping(), 5000);
      if (this.#manifest.overlay && this.capabilities.includes('ui.canvas')) this.#overlay(this.#manifest.overlay.z_index);
      this.#onStarted?.();
    } catch (error) {
      if (generation === this.#generation) this.stop(`${this.name} could not start: ${(error as Error).message}`);
    }
  }

  /** Stop the client part and forget its credential; `reason` is shown where it drew. */
  stop(reason = '') {
    ++this.#generation;
    clearInterval(this.#watchdog);
    window.removeEventListener('message', this.#onCrash);
    this.#port?.close(); this.#port = null;
    this.#stopOverlay();
    this.#frame?.remove(); this.#frame = null;
    if (this.#token) void extensionHost.revokeCredential(this.id, this.#token).catch(() => {});
    this.#token = '';
    this.status = 'stopped'; this.reason = reason; this.trees = {};
    this.states = {}; this.dialog = null; this.menu = null; this.contributions = {}; this.commands = []; this.appActions = []; this.searchScope = '';
  }

  restart() { this.stop(); void this.start(); }

  /** The owner's accounts, for extensions that may read account status. */
  accounts(accounts: AccountSummary[]) { return this.capabilities.includes('account.read') ? { accounts } : {}; }

  send(message: Record<string, unknown>, buffers: ArrayBuffer[] = []) {
    if (this.status !== 'running') return;
    // The owner picked one of its commands, actions or toast buttons: it may move focus next.
    if (OWNER_ACTIONS.has(message.t as string)) this.#lastEvent = performance.now();
    this.#port?.postMessage({ json: JSON.stringify(message), buffers }, buffers);
  }

  /** A meaningful interaction in one of its surfaces. */
  /** The owner asked for a row's menu at (x, y); it shows once the extension answers. */
  openMenu(surface: string, node: string, key: string, x: number, y: number) {
    this.menu = { surface, node, key, x, y, items: null };
    this.event(surface, node, 'context', { key });
  }
  /** A menu of a component's built-in actions, answered by the renderer, not the extension. */
  localMenu(x: number, y: number, label: string, items: MenuItem[], choose: (item: string) => void) {
    this.menu = { surface: '', node: '', key: '', x, y, items, label, choose };
  }
  chooseMenu(item: string) {
    const menu = this.menu;
    this.menu = null;
    if (menu?.choose) menu.choose(item);
    else if (menu) this.event(menu.surface, menu.node, 'menu', { key: menu.key, item });
  }

  event(surface: string, node: string, kind: string, value?: unknown) {
    this.#lastEvent = performance.now();
    this.send({ t: 'event', surface, node, kind, ...(value === undefined ? {} : { value }) });
  }

  /** Files picked in a `file_picker`: their bytes travel as transferred buffers. */
  files(surface: string, node: string, files: { name: string; type: string; size: number; failed?: boolean }[], buffers: ArrayBuffer[]) {
    this.#lastEvent = performance.now();
    this.send({ t: 'event', surface, node, kind: 'files', value: files.map((file, index) => ({ ...file, data: { $buffer: index } })) }, buffers);
  }

  #ping() {
    if (this.#pinged > this.#answered && this.#pinged - this.#answered >= 2) { this.stop(`${this.name} stopped responding`); return; }
    this.send({ t: 'ping', n: ++this.#pinged });
  }

  #receive(data: unknown) {
    try {
      const { json, buffers } = data as { json: unknown; buffers: unknown };
      if (typeof json !== 'string' || json.length > LIMITS.message || !Array.isArray(buffers)
        || buffers.reduce((total: number, buffer) => total + (buffer instanceof ArrayBuffer ? buffer.byteLength : typeof ImageBitmap === 'function' && buffer instanceof ImageBitmap ? buffer.width * buffer.height * 4 : Infinity), 0) > LIMITS.buffers) throw new ProtocolError('Message too large');
      const message = checkOutgoing(JSON.parse(json), this.surfaces);
      switch (message.t) {
        case 'render': this.trees[message.surface] = message.tree; this.#rendered.add(message.surface); break;
        case 'patch': this.trees[message.surface] = applyPatch(this.trees[message.surface] ?? null, message.ops); break;
        case 'pong': this.#answered = Math.max(this.#answered, message.n); break;
        case 'call': void this.#call(message); break;
        case 'toast': this.#toast(message); break;
        case 'toast_dismiss': dismissKey(`${this.id}:${message.id}`); break;
        case 'state': this.states[message.surface] = { dirty: !!message.dirty, busy: !!message.busy, back: !!message.back }; break;
        case 'show': this.#context().shell?.show(this.id, message.surface); break;
        case 'leave': this.#context().shell?.leave(this.id, message.surface); break;
        case 'focus': this.#focus(message.surface, message.node); break;
        case 'dialog': this.dialog = message.tree; break;
        case 'menu': {
          // Only an answer to the menu the owner asked for, and only while it is still wanted.
          const menu = this.menu;
          if (menu && !menu.items && menu.surface === message.surface && menu.node === message.node && menu.key === message.key) this.menu = message.items.length ? { ...menu, items: message.items } : null;
          break;
        }
        case 'contribute':
          if (!this.capabilities.includes('ui.contribute')) throw new ProtocolError('Contributions need ui.contribute');
          this.contributions[message.slot] = message.tree;
          break;
        case 'commands': this.#need('ui.commands'); this.commands = message.items; break;
        case 'command_search': this.#need('ui.commands'); this.searchScope = message.scope; break;
        case 'command_hits': if (message.seq >= this.hits.seq) this.hits = { seq: message.seq, hits: message.hits }; break;
        case 'app_actions': this.#need('ui.app_actions'); this.appActions = message.items; break;
        case 'frame': this.#need('ui.canvas'); this.#drawFrame((data as { buffers: ImageBitmap[] }).buffers[0], message.x ?? 0, message.y ?? 0); break;
        case 'download': this.#download(message.name, (data as { buffers: ArrayBuffer[] }).buffers[message.data.$buffer]); break;
      }
    } catch (error) {
      this.stop(`${this.name} stopped: ${(error as Error).message}`);
    }
  }

  #renewing: Promise<string> | null = null;
  #renew() {
    this.#renewing ??= extensionHost.credential(this.id).then(token => {
      this.#token = token; this.#renewing = null; this.#context().shell?.credentialsChanged();
      return token;
    }, error => { this.#renewing = null; throw error; });
    return this.#renewing;
  }

  #need(capability: string) { if (!this.capabilities.includes(capability)) throw new ProtocolError(`This needs ${capability}`); }

  /** Focus is honoured only right after an interaction, or on a surface's first render. */
  #focus(surface: string, node: string) {
    const first = !this.#rendered.has(`focus:${surface}`);
    this.#rendered.add(`focus:${surface}`);
    if (!first && performance.now() - this.#lastEvent > 1000) return;
    requestAnimationFrame(() => document.getElementById(`x-${this.id}-${surface}-${node}`)?.focus());
  }

  // ---- `canvas` overlays (docs/extensions/ui-model.md, "Canvas") ----
  #canvas: HTMLCanvasElement | null = null;
  #context2d: CanvasRenderingContext2D | null = null;
  #stopOverlay = () => {};
  /** A drawing surface over the whole window that never takes the pointer. The Worker draws on
   *  it directly (OffscreenCanvas), or sends finished frames where that does not work. */
  #overlay(zIndex: number) {
    const canvas = Object.assign(document.createElement('canvas'), { className: 'extension-canvas' });
    canvas.style.zIndex = String(zIndex);
    canvas.setAttribute('aria-hidden', 'true');
    document.body.append(canvas);
    this.#canvas = canvas;
    const size = () => ({ width: innerWidth, height: innerHeight, ratio: devicePixelRatio || 1 });
    const { width, height, ratio } = size();
    canvas.width = Math.round(width * ratio); canvas.height = Math.round(height * ratio);
    // WebKit (Safari, WebKitGTK) accepts the transfer but never shows what a Worker draws on
    // it, so only Chromium engines, recognised by their `userAgentData`, draw directly.
    if (typeof canvas.transferControlToOffscreen === 'function' && 'userAgentData' in navigator) {
      const offscreen = canvas.transferControlToOffscreen();
      this.#port?.postMessage({ json: JSON.stringify({ t: 'canvas', surface: 'overlay', canvas: { $buffer: 0 }, width, height, ratio }), buffers: [offscreen] }, [offscreen]);
    } else {
      this.#context2d = canvas.getContext('2d');
      this.send({ t: 'canvas', surface: 'overlay', mode: 'frames', width, height, ratio });
    }
    // The pointer, at most once per animation frame.
    let pending: { x: number; y: number; inside: boolean } | null = null;
    let frame = 0;
    const flush = () => { frame = 0; if (pending) this.send({ t: 'pointer', ...pending }); pending = null; };
    const move = (event: PointerEvent) => { pending = { x: event.clientX, y: event.clientY, inside: true }; frame ||= requestAnimationFrame(flush); };
    const leave = () => { pending = { x: pending?.x ?? 0, y: pending?.y ?? 0, inside: false }; frame ||= requestAnimationFrame(flush); };
    const resize = () => {
      const next = size();
      if (this.#context2d && this.#canvas) { this.#canvas.width = Math.round(next.width * next.ratio); this.#canvas.height = Math.round(next.height * next.ratio); }
      this.send({ t: 'resize', surface: 'overlay', ...next });
    };
    window.addEventListener('pointermove', move, { passive: true });
    document.documentElement.addEventListener('pointerleave', leave);
    window.addEventListener('resize', resize);
    this.#stopOverlay = () => {
      window.removeEventListener('pointermove', move);
      document.documentElement.removeEventListener('pointerleave', leave);
      window.removeEventListener('resize', resize);
      cancelAnimationFrame(frame);
      this.#canvas?.remove(); this.#canvas = null; this.#context2d = null;
      this.#stopOverlay = () => {};
    };
  }
  /** A frame replaces the last one; `x`, `y` (CSS pixels) let a small bitmap stand for the window. */
  #drawFrame(bitmap: ImageBitmap | undefined, x: number, y: number) {
    if (!bitmap || !this.#context2d || !this.#canvas) { bitmap?.close(); return; }
    const ratio = this.#canvas.width / Math.max(innerWidth, 1);
    this.#context2d.clearRect(0, 0, this.#canvas.width, this.#canvas.height);
    this.#context2d.drawImage(bitmap, Math.round(x * ratio), Math.round(y * ratio));
    bitmap.close();
  }

  /** Saved, never shown: attachments and exports download as plain bytes. */
  #download(name: string, buffer: ArrayBuffer | undefined) {
    if (!buffer) throw new ProtocolError('Missing download data');
    const url = URL.createObjectURL(new Blob([buffer], { type: 'application/octet-stream' }));
    const link = Object.assign(document.createElement('a'), { href: url, download: name });
    link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  #toast(message: { id: string; title: string; body?: string; icon?: IconName; duration?: number; system?: boolean; actions?: ToastAction[] }) {
    if (!this.capabilities.includes('ui.toasts')) throw new ProtocolError('Toasts need ui.toasts');
    const shell = this.#context().shell;
    const run = (action: ToastAction) => () => {
      if (action.reject_send) { if (this.capabilities.includes('approvals.notify')) shell?.rejectSend(action.reject_send); }
      else if (action.review_send) shell?.reviewSend(action.review_send);
      else if (action.navigate) shell?.navigate(action.navigate, action.account);
      else if (action.url) window.open(action.url, '_blank', 'noopener');
      else this.send({ t: 'toast_action', toast: message.id, action: action.id });
    };
    notify({
      key: `${this.id}:${message.id}`, source: this.name, title: message.title, body: message.body, icon: message.icon,
      ...(message.duration === undefined ? {} : { duration: message.duration }),
      // Extensions can never approve a send: the shell draws Reject with its own label.
      actions: message.actions?.map(action => ({ label: action.reject_send ? 'Reject' : action.label, primary: action.primary, run: run(action) })),
    });
    if (message.system) shell?.desktop(message.title, message.body ?? '', `${this.id}:${message.id}`);
  }

  async #call(message: { id: number; method: string; path: string; query?: Record<string, string>; body?: unknown; bytes?: boolean }) {
    if (this.#pending >= LIMITS.calls) { this.send({ t: 'reply', id: message.id, ok: false, error: 'Too many calls at once' }); return; }
    this.#pending++;
    let token = this.#token;
    const account = this.#context().account?.id ?? 'demo';
    try {
      const query = message.query ? `?${new URLSearchParams(message.query)}` : '';
      // The Host is the boundary; this only keeps calls inside the API.
      const send = () => fetch(`/api/v1${message.path}${query}`, {
        method: message.method, credentials: 'omit',
        headers: { authorization: `Colecto-Extension ${token}`, 'x-colecto-account': account, ...(message.body === undefined ? {} : { 'content-type': 'application/json' }) },
        ...(message.body === undefined ? {} : { body: JSON.stringify(message.body) }),
      });
      let response = await send().catch(() => { throw callFailure(message.path, 'unreachable'); });
      // A restarted Host forgot every credential: mint a new one with the owner bearer (which
      // asks the owner to reconnect if that is refused too) and try once more.
      if (response.status === 401 && token === this.#token) {
        token = await this.#renew();
        response = await send().catch(() => { throw callFailure(message.path, 'unreachable'); });
      }
      if (message.bytes && response.ok) {
        // Attachments and other binary bodies travel as a transferred buffer.
        const buffer = await response.arrayBuffer();
        if (token === this.#token) this.send({ t: 'reply', id: message.id, ok: true, data: { $buffer: 0 } }, [buffer]);
        return;
      }
      const data = response.status === 204 ? null : await response.json().catch(() => undefined);
      if (!response.ok) throw callFailure(message.path, 'refused', data, response.status);
      if (data === undefined) throw callFailure(message.path, 'unreadable', null, response.status);
      if (token === this.#token) this.send({ t: 'reply', id: message.id, ok: true, data });
    } catch (error) {
      if (token === this.#token) this.send({ t: 'reply', id: message.id, ok: false, error: (error as Error).message, status: (error as ServiceError).status ?? 0 });
    } finally { this.#pending--; }
  }
}

/** Apply all ops to a copy of the tree, or none: an invalid op rejects the message. */
export function applyPatch(tree: Node | null, ops: Patch[]): Node {
  if (!tree) throw new ProtocolError('Nothing rendered to patch');
  const next = structuredClone($state.snapshot(tree)) as Node;
  const index = new Map<string, { node: Node; parent: Node | null }>();
  const walk = (node: Node, parent: Node | null) => {
    if (node.id) index.set(node.id, { node, parent });
    for (const child of node.children ?? []) walk(child, node);
  };
  walk(next, null);
  for (const op of ops) {
    const target = op.node ? index.get(op.node) : undefined;
    if (op.op === 'set' && target && op.props && typeof op.props === 'object') {
      for (const [key, value] of Object.entries(op.props)) {
        if (['type', 'id', 'children'].includes(key)) throw new ProtocolError('set cannot change type, id or children');
        if (value === null) delete target.node[key]; else target.node[key] = value;
      }
    } else if (op.op === 'replace' && target) {
      const tree = op.tree as Node;
      if (target.parent) target.parent.children![target.parent.children!.indexOf(target.node)] = tree;
      else return checkTree(tree);
      index.clear(); walk(next, null);
    } else if (op.op === 'insert' && op.parent && index.get(op.parent)?.node.children && Number.isInteger(op.index)) {
      const children = index.get(op.parent)!.node.children!;
      if (op.index! < 0 || op.index! > children.length) throw new ProtocolError('Insert index out of range');
      children.splice(op.index!, 0, op.tree as Node);
      walk(op.tree as Node, index.get(op.parent)!.node);
    } else if (op.op === 'remove' && target?.parent) {
      target.parent.children!.splice(target.parent.children!.indexOf(target.node), 1);
      index.clear(); walk(next, null);
    } else {
      throw new ProtocolError(`Invalid ${String(op.op)} op`);
    }
  }
  return checkTree(next);
}

/** A bundled client part's files, built with the UI (`vite.config.ts`). */
async function bundledFiles(id: string) {
  const base = `/extensions/${encodeURIComponent(id)}`;
  const paths = await (await fetch(`${base}/files.json`, { credentials: 'omit' })).json() as string[];
  const files: Record<string, ArrayBuffer> = {};
  await Promise.all(paths.map(async path => { files[path] = await (await fetch(`${base}/${path}`, { credentials: 'omit' })).arrayBuffer(); }));
  return files;
}
