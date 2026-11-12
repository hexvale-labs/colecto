import { request as post, subscribeEvents, extensionHost } from './api';
export type Handler = (payload: any, from: string) => unknown | Promise<unknown>;
/** Each extension binds its own identity; no other extension's state is shared. */
export function extensionBus(id: string) {
  const handlers = new Map<string, Handler>();
  let stop: (() => void) | undefined;
  return {
    async request(bus: string, name: string, payload: unknown) {
      return (await post<{ data: unknown }>(id, 'demo', `/bus/${encodeURIComponent(bus)}/request`, { name, payload })).data;
    },
    async emit(name: string, payload: unknown) { await post(id, 'demo', '/bus/events', { name, payload }); },
    handle(name: string, fn: Handler) {
      handlers.set(name, fn);
      stop ??= subscribeEvents((event, value) => {
        if (event !== 'bus.request') return;
        const input = value as { id: string; bus: string; name: string; from: string; payload: unknown };
        if (input.bus !== id) return;
        void (async () => {
          let data: unknown = null;
          try { data = await handlers.get(input.name)?.(input.payload, input.from) ?? null; } catch { /* Failed handlers have no answer. */ }
          await post(id, 'demo', `/bus/replies/${encodeURIComponent(input.id)}`, { data });
        })().catch(() => {});
      });
      return () => { handlers.delete(name); if (!handlers.size) { stop?.(); stop = undefined; } };
    },
  };
}

export async function extensionName(id: string): Promise<string> {
  try { return (await extensionHost.list('demo')).find(e => e.manifest.id === id)?.manifest.name ?? id; }
  catch { return id; }
}
