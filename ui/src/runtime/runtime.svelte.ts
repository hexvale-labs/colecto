// The shell's side of one extension's client part (docs/extensions/ui-model.md, "Client
// runtime" and "Credentials"): it holds the extension's credential, starts its runtime frame
// and Worker, checks every message, answers API calls and keeps the surfaces the renderer
// draws. The credential and the owner token never cross into the frame or the Worker.
import { extensionHost, ServiceError, type ExtensionState } from '../lib/api';
import { checkOutgoing, checkTree, LIMITS, ProtocolError, type Node, type Patch } from './protocol';
import type { Theme } from './theme';

export interface AccountSummary { id: string; name: string; address: string; connected: boolean; configured: boolean }
export type Status = 'starting' | 'running' | 'stopped';

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

export class ExtensionRuntime {
  readonly id: string;
  readonly name: string;
  readonly surfaces: string[];
  status = $state<Status>('starting');
  /** Why it stopped, in words for the owner. */
  reason = $state('');
  trees = $state<Record<string, Node | null>>({});
  #manifest: ExtensionState['manifest'];
  #context: () => { account: AccountSummary | null; theme: Theme };
  #token = '';
  #frame: HTMLIFrameElement | null = null;
  #port: MessagePort | null = null;
  #pending = 0;
  #watchdog: ReturnType<typeof setInterval> | undefined;
  #pinged = 0;
  #answered = 0;
  #generation = 0;
  #onCrash = (event: MessageEvent) => { if (event.source === this.#frame?.contentWindow && event.data?.t === 'colecto.crash') this.stop(`${this.name} stopped`); };

  constructor(extension: ExtensionState, context: () => { account: AccountSummary | null; theme: Theme }) {
    this.#manifest = extension.manifest;
    this.id = extension.manifest.id;
    this.name = extension.manifest.name;
    this.surfaces = [...(extension.manifest.pane ? ['pane'] : []), ...(extension.manifest.settings_page ? ['settings'] : [])];
    this.#context = context;
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
        extensionHost.clientFiles(this.id),
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
      const { account, theme } = this.#context();
      this.status = 'running';
      this.send({ t: 'start', api: 1, id: this.id, theme, account, settings, surfaces: this.surfaces });
      this.#pinged = this.#answered = 0;
      this.#watchdog = setInterval(() => this.#ping(), 5000);
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
    this.#frame?.remove(); this.#frame = null;
    if (this.#token) void extensionHost.revokeCredential(this.id, this.#token).catch(() => {});
    this.#token = '';
    this.status = 'stopped'; this.reason = reason; this.trees = {};
  }

  restart() { this.stop(); void this.start(); }

  send(message: Record<string, unknown>, buffers: ArrayBuffer[] = []) {
    if (this.status !== 'running') return;
    this.#port?.postMessage({ json: JSON.stringify(message), buffers }, buffers);
  }

  /** A meaningful interaction in one of its surfaces. */
  event(surface: string, node: string, kind: string, value?: unknown) {
    this.send({ t: 'event', surface, node, kind, ...(value === undefined ? {} : { value }) });
  }

  #ping() {
    if (this.#pinged > this.#answered && this.#pinged - this.#answered >= 2) { this.stop(`${this.name} stopped responding`); return; }
    this.send({ t: 'ping', n: ++this.#pinged });
  }

  #receive(data: unknown) {
    try {
      const { json, buffers } = data as { json: unknown; buffers: unknown };
      if (typeof json !== 'string' || json.length > LIMITS.message || !Array.isArray(buffers)
        || buffers.reduce((total: number, buffer) => total + (buffer instanceof ArrayBuffer ? buffer.byteLength : Infinity), 0) > LIMITS.buffers) throw new ProtocolError('Message too large');
      const message = checkOutgoing(JSON.parse(json), this.surfaces);
      switch (message.t) {
        case 'render': this.trees[message.surface] = message.tree; break;
        case 'patch': this.trees[message.surface] = applyPatch(this.trees[message.surface] ?? null, message.ops); break;
        case 'pong': this.#answered = Math.max(this.#answered, message.n); break;
        case 'call': void this.#call(message); break;
      }
    } catch (error) {
      this.stop(`${this.name} stopped: ${(error as Error).message}`);
    }
  }

  async #call(message: { id: number; method: string; path: string; query?: Record<string, string>; body?: unknown }) {
    if (this.#pending >= LIMITS.calls) { this.send({ t: 'reply', id: message.id, ok: false, error: 'Too many calls at once' }); return; }
    this.#pending++;
    const token = this.#token;
    const account = this.#context().account?.id ?? 'demo';
    try {
      const query = message.query ? `?${new URLSearchParams(message.query)}` : '';
      // The Host is the boundary; this only keeps calls inside the API.
      const response = await fetch(`/api/v1${message.path}${query}`, {
        method: message.method, credentials: 'omit',
        headers: { authorization: `Colecto-Extension ${token}`, 'x-colecto-account': account, ...(message.body === undefined ? {} : { 'content-type': 'application/json' }) },
        ...(message.body === undefined ? {} : { body: JSON.stringify(message.body) }),
      });
      const data = await response.json().catch(() => null);
      if (!response.ok) throw new ServiceError(typeof data?.error === 'string' ? data.error : `HTTP ${response.status}`, data?.category ?? 'server', response.status);
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
