import { request as post, subscribeEvents, extensionHost } from './api';
export type Handler = (payload: any, from: string) => unknown | Promise<unknown>;

// Guests answering a request or handling an event act on this account, so it must be the one
// the owner is looking at. The mail shell owns account selection and keeps this current.
let account = 'demo';
export function setBusAccount(id: string) { account = id; }

/** A bundled extension's handle on the bus. It speaks only as `id` and never sees other buses'
 *  handlers or subscribers; see docs/extensions/README.md, "Extension bus". */
export function extensionBus(id: string) {
  const handlers = new Map<string, Handler>();
  const listeners = new Map<string, Set<(payload: any) => void>>();
  let stop: (() => void) | undefined;
  function connect() {
    stop ??= subscribeEvents((event, value) => {
      if (event === 'bus.event') {
        const input = value as { bus: string; name: string; payload: unknown };
        for (const listener of listeners.get(`${input.bus}/${input.name}`) ?? []) listener(input.payload);
        return;
      }
      if (event !== 'bus.request') return;
      const input = value as { id: string; bus: string; name: string; from: string; payload: unknown };
      if (input.bus !== id) return;
      void (async () => {
        let data: unknown = null;
        try { data = await handlers.get(input.name)?.(input.payload, input.from) ?? null; } catch { /* A failed handler has no answer. */ }
        await post(id, account, `/bus/replies/${encodeURIComponent(input.id)}`, { data });
      })().catch(() => {});
    });
  }
  function disconnect() { if (!handlers.size && !listeners.size) { stop?.(); stop = undefined; } }
  return {
    /** Ask another extension; resolves to null when it is missing or has no answer. */
    async request(bus: string, name: string, payload: unknown) {
      return (await post<{ data: unknown }>(id, account, `/bus/${encodeURIComponent(bus)}/request`, { name, payload })).data;
    },
    /** Publish an event on this extension's own bus. Resolves once subscribers ran or 1.5 s passed. */
    async emit(name: string, payload: unknown) { await post(id, account, '/bus/events', { name, payload }); },
    /** Answer requests to this extension's bus. Returns a function that stops answering. */
    handle(name: string, fn: Handler) {
      handlers.set(name, fn); connect();
      return () => { handlers.delete(name); disconnect(); };
    },
    /** Receive another bus's event. The manifest must also list it under `[bus] subscribe`. */
    listen(topic: `${string}/${string}`, fn: (payload: any) => void) {
      const set = listeners.get(topic) ?? new Set();
      set.add(fn); listeners.set(topic, set); connect();
      return () => { set.delete(fn); if (!set.size) listeners.delete(topic); disconnect(); };
    },
  };
}

export async function extensionName(id: string): Promise<string> {
  try { return (await extensionHost.list(account)).find(e => e.manifest.id === id)?.manifest.name ?? id; }
  catch { return id; }
}
