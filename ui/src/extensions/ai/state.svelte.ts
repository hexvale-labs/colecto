// State shared by this extension's own components only (settings page, reader button, summary).
import { aiHost, aiService } from './api';

export const ai = $state({ enabled: false });
let loading: Promise<void> | undefined;
/** Load once; Settings → AI calls `setEnabled` when the owner flips the switch. */
export function loadAi() {
  loading ??= aiHost.settings().then(view => { ai.enabled = view.settings.enabled; }).catch(() => { loading = undefined; });
  return loading;
}
export function setEnabled(on: boolean) { ai.enabled = on; }

export interface SummaryState { text: string | null; busy: boolean; error: string; loaded: boolean }
const summaries = $state<Record<string, SummaryState>>({});
const key = (account: string, id: string) => `${account}\n${id}`;
export function summary(account: string, id: string): SummaryState {
  return summaries[key(account, id)] ?? { text: null, busy: false, error: '', loaded: false };
}
function set(account: string, id: string, change: Partial<SummaryState>) {
  summaries[key(account, id)] = { ...summary(account, id), ...change };
}
export async function loadSummary(account: string, id: string) {
  if (summary(account, id).loaded) return;
  try { set(account, id, { text: (await aiService(account).summary(id)).summary, loaded: true }); }
  catch { set(account, id, { loaded: true }); }
}
export async function summarize(account: string, id: string) {
  if (summary(account, id).busy) return;
  set(account, id, { busy: true, error: '' });
  try { set(account, id, { text: (await aiService(account).summarize(id)).summary, loaded: true }); }
  catch (e) { set(account, id, { error: (e as Error).message }); }
  finally { set(account, id, { busy: false }); }
}
export async function forget(account: string, id: string) {
  const previous = summary(account, id).text;
  set(account, id, { text: null, error: '' });
  try { await aiService(account).forget(id); }
  catch (e) { set(account, id, { text: previous, error: (e as Error).message }); }
}
