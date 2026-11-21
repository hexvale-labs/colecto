// The shared client library for extension client parts (docs/extensions/ui-model.md,
// "Shared client library"). It runs inside each extension's Worker, so every extension has
// its own copy and nothing here is shared between extensions. Bundled extensions import it;
// third-party authors can vendor it.
import type { Node, Row } from '../runtime/protocol';
export type { Node, Row };

declare const colecto: {
  onmessage: ((message: Message) => void) | null;
  send(message: object, transfer?: ArrayBuffer[]): void;
  file(path: string): Promise<ArrayBuffer>;
};

export interface Account { id: string; name: string; address: string; connected: boolean; configured: boolean }
export type Message = { t: string; [key: string]: unknown };
type Handler = (message: Message) => unknown;

export class ApiError extends Error { constructor(message: string, readonly status: number) { super(message); } }

const listeners = new Map<string, Set<Handler>>();
const nodeHandlers = new Map<string, (value: unknown, message: Message) => unknown>();
const pending = new Map<number, { resolve: (data: unknown) => void; reject: (error: Error) => void }>();
const busHandlers = new Map<string, (payload: any, from: string) => unknown>();
const busListeners = new Map<string, Set<(payload: any) => void>>();
let calls = 0;
const state = { id: '', account: null as Account | null, accounts: [] as Account[], settings: {} as Record<string, unknown>, theme: null as Record<string, unknown> | null, surfaces: [] as string[], offset: 0 };

function dispatch(type: string, message: Message) { for (const listener of listeners.get(type) ?? []) void listener(message); }

colecto.onmessage = (message) => {
  switch (message.t) {
    case 'ping': colecto.send({ t: 'pong', n: message.n }); return;
    case 'reply': {
      const waiting = pending.get(message.id as number);
      pending.delete(message.id as number);
      if (message.ok) waiting?.resolve(message.data); else waiting?.reject(new ApiError(String(message.error), Number(message.status ?? 0)));
      return;
    }
    case 'start':
      state.id = message.id as string; state.account = message.account as Account | null;
      state.settings = message.settings as Record<string, unknown>; state.theme = message.theme as Record<string, unknown>;
      state.surfaces = message.surfaces as string[];
      state.accounts = (message.accounts as Account[]) ?? [];
      state.offset = typeof message.now === 'number' ? message.now - Date.now() : 0;
      break;
    case 'account': state.account = message.account as Account | null; if (message.accounts) state.accounts = message.accounts as Account[]; break;
    case 'settings': state.settings = message.settings as Record<string, unknown>; break;
    case 'theme': state.theme = message.theme as Record<string, unknown>; break;
    case 'event': {
      const handler = nodeHandlers.get(`${message.surface}\0${message.node}\0${message.kind}`) ?? nodeHandlers.get(`*\0${message.node}\0${message.kind}`);
      void handler?.(message.value, message);
      break;
    }
    case 'host': dispatch(`host:${message.name}`, message); return;
    case 'bus.request': {
      const handler = busHandlers.get(message.name as string);
      void (async () => {
        let data: unknown = null;
        try { data = await handler?.(message.payload, message.from as string) ?? null; } catch { /* No answer. */ }
        await call('POST', `/bus/replies/${encodeURIComponent(String(message.id))}`, { body: { data } }).catch(() => {});
      })();
      return;
    }
    case 'bus.event': for (const listener of busListeners.get(`${message.bus}/${message.name}`) ?? []) listener(message.payload); return;
  }
  dispatch(message.t, message);
};

/** An API call with this extension's credential, for the selected account (or `account`). */
export function call<T = unknown>(method: 'GET' | 'POST' | 'PUT' | 'DELETE', path: string, options: { query?: Record<string, string>; body?: unknown; bytes?: boolean } = {}): Promise<T> {
  const id = ++calls;
  colecto.send({ t: 'call', id, method, path, ...(options.query ? { query: options.query } : {}), ...(options.body === undefined ? {} : { body: options.body }), ...(options.bytes ? { bytes: true } : {}) });
  return new Promise((resolve, reject) => pending.set(id, { resolve: resolve as (data: unknown) => void, reject }));
}

export const app = {
  get id() { return state.id; },
  get account() { return state.account; },
  get settings() { return state.settings; },
  get theme() { return state.theme; },
  /** Accounts that are on (needs `account.read`). */
  get accounts() { return state.accounts; },
  /** The window's clock, so dates such as "Today" match what the shell shows. */
  now: () => new Date(Date.now() + state.offset),
  /** Shell messages by type (`start`, `account`, `settings`, `theme`, `toast_action`, `command`,
   *  `command_query`, `app_action`, `slot`, `frame.message`, `state`, …), or `host:<event>`. */
  on(type: string, fn: Handler) {
    const set = listeners.get(type) ?? new Set();
    set.add(fn); listeners.set(type, set);
    return () => set.delete(fn);
  },
  /** React to an interaction with node `node` (`press`, `change`, `select`, …) in `surface` (any when omitted). */
  when(node: string, kind: string, fn: (value: any, message: Message) => unknown, surface = '*') {
    nodeHandlers.set(`${surface}\0${node}\0${kind}`, fn);
  },
  send(message: object, transfer?: ArrayBuffer[]) { colecto.send(message, transfer); },
  render(surface: string, tree: Node | null) { colecto.send({ t: 'render', surface, tree }); },
  patch(surface: string, ops: object[]) { if (ops.length) colecto.send({ t: 'patch', surface, ops }); },
  file: (path: string) => colecto.file(path),
};

export const bus = {
  /** Ask another extension; null when it is missing or does not answer. */
  async request<T = unknown>(target: string, name: string, payload: unknown): Promise<T | null> {
    return ((await call<{ data: T | null }>('POST', `/bus/${encodeURIComponent(target)}/request`, { body: { name, payload } })).data) ?? null;
  },
  async emit(name: string, payload: unknown) { await call('POST', '/bus/events', { body: { name, payload } }); },
  handle(name: string, fn: (payload: any, from: string) => unknown) { busHandlers.set(name, fn); },
  listen(topic: string, fn: (payload: any) => void) {
    const set = busListeners.get(topic) ?? new Set();
    set.add(fn); busListeners.set(topic, set);
  },
};

export const storage = {
  get: <T = unknown>(key: string) => call<{ value: T | null }>('GET', `/extensions/${state.id}/storage/${encodeURIComponent(key)}`).then(r => r.value),
  set: (key: string, value: unknown) => call('PUT', `/extensions/${state.id}/storage/${encodeURIComponent(key)}`, { body: { value } }),
};

const encoder = new TextEncoder();
/** `text` cut to at most `bytes` of UTF-8, on a character boundary, with an ellipsis when cut. */
export function clip(text: string, bytes: number) {
  if (encoder.encode(text).length <= bytes) return text;
  let out = '';
  for (const char of text) { if (encoder.encode(out + char + '…').length > bytes) break; out += char; }
  return out + '…';
}

export interface ToastAction { id: string; label: string; primary?: boolean; navigate?: string; account?: string; reject_send?: string; review_send?: string; url?: string }
export interface Toast { id: string; title: string; body?: string; icon?: string; duration?: number; system?: boolean; actions?: ToastAction[] }
export const toast = (item: Toast) => colecto.send({ t: 'toast', ...item });
