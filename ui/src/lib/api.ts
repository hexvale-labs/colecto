// Version 1 wire contracts mirror colecto-extension-api. Rust validates all input.
import type { Discovery } from './discovery';
export type Labels = Record<string, string[]>;
export interface Message { seen: boolean; flagged: boolean; id: string; sender: string; address: string; subject: string; body: string; date: string }
export interface MailSnapshot { folder: string; messages: Message[]; fetched_at: number | null }
export class ServiceError extends Error { constructor(message: string, readonly category: string, readonly status?: number) { super(message); this.name = 'ServiceError'; } }
export interface Draft { id: string; recipient: string; cc?: string; subject: string; body: string; in_reply_to?: string; references?: string; forward_of?: string }
export interface OutgoingAttachment { name: string; content_type: string; data: string }
export interface DraftInput extends Omit<Draft, 'id'> { id?: string; attachments?: OutgoingAttachment[] }
export interface AttachmentInfo { index: number; name: string; content_type: string; size: number }
export interface MessageContent { text: string; html: string | null; attachments: AttachmentInfo[]; message_id: string; references: string; reply_to: string; to: string; cc: string }
export interface MailItem { id: string; name: string; subject: string; preview: string; date: string }
export interface AccountSettings { name: string; address: string; protocol: 'imap' | 'jmap'; imap_host: string; imap_port: number; smtp_host: string; smtp_port: number; username: string; session_url: string; imap_starttls?: boolean; tls_certificate?: string }
export interface AccountInfo { id: string; settings: AccountSettings | null; connected: boolean; configured: boolean }
/** `version` is the Colecto that wrote the unit; `update_to` is set when this app can rewrite it. */
export interface BackgroundService { installed: boolean; version?: string | null; update_to?: string | null; enabled: boolean; active: boolean; supported: boolean; reason?: string | null; socket?: string | null; token_file?: string | null }
export interface Upstream { address: string; token_file: string }
/** A socket names its daemon's token file; a remote origin sends its access token once. */
export type UpstreamInput = { address: string; token_file: string } | { address: string; token?: string };
/** How an account syncs right now (`GET /sync`, and the `sync` event). */
export interface AccountSync { account: string; protocol: 'imap' | 'jmap' | 'demo'; mode: 'local' | 'jmap_push' | 'imap_idle' | 'polling' | 'offline'; interval: number; activity: 'idle' | 'syncing' | 'prefetching'; progress: [number, number] | null; error: string | null; updated_at: number | null }
export interface SecurityInfo { cleanup_pending: boolean; protected: boolean; locked: boolean }

let transientToken = '';
export function setAccessToken(token: string) { transientToken = token; try { if (token) sessionStorage.setItem('colecto.token', token); else sessionStorage.removeItem('colecto.token'); } catch { /* Session storage may be disabled. */ } }
function accessToken() { try { return sessionStorage.getItem('colecto.token') ?? ''; } catch { return transientToken; } }

function headers(extension: string, account: string): Record<string, string> {
  return { ...(accessToken() ? { Authorization: `Bearer ${accessToken()}` } : {}), 'x-colecto-extension': extension, 'x-colecto-account': account };
}
async function download(extension: string, account: string, path: string): Promise<Blob> {
  let response: Response;
  try { response = await fetch(`/api/v1${path}`, { credentials: 'omit', headers: headers(extension, account) }); }
  catch { throw new ServiceError('Cannot reach Colecto. Check that your mail service is running, then try again.', 'network'); }
  if (!response.ok) { const data = await response.json().catch(() => null); throw new ServiceError(typeof data?.error === 'string' ? data.error : `Download failed (HTTP ${response.status}).`, typeof data?.category === 'string' ? data.category : 'server', response.status); }
  return response.blob();
}
const deliveryWarning = 'Delivery status is unknown. Check Sent and delivery requests before sending again.';
/** Calls that may have delivered mail even when no readable answer came back. */
const isDelivery = (path: string) => path === '/send' || (path.startsWith('/send-requests/') && path.endsWith('/approve'));
/** Why a call failed, worded so nobody resends a message that may already have gone out. Used
 *  for the shell's own calls and for the calls it makes for extensions' client parts. */
export function callFailure(path: string, kind: 'unreachable' | 'unreadable' | 'refused', data?: unknown, status?: number): ServiceError {
  const delivery = isDelivery(path);
  const reported = data as { error?: unknown; category?: unknown } | null | undefined;
  if (kind === 'unreachable') return new ServiceError(delivery ? deliveryWarning : 'Cannot reach Colecto. Check that your mail service is running, then try again.', delivery ? 'delivery_uncertain' : 'network');
  if (kind === 'unreadable') return new ServiceError(delivery ? deliveryWarning : 'Mail service returned an unreadable response. Check your connection and try again.', delivery ? 'delivery_uncertain' : 'invalid_response', status);
  return new ServiceError(typeof reported?.error === 'string' ? reported.error : `Mail service returned HTTP ${status}. ${delivery ? 'Check delivery status before sending again.' : 'Try again when the service is available.'}`, typeof reported?.category === 'string' ? reported.category : 'server', status);
}
export async function request<T>(extension: string, account: string, path: string, input?: unknown, method?: 'DELETE' | 'PUT'): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`/api/v1${path}`, {
      credentials: 'omit',
      method: method ?? (input === undefined ? 'GET' : 'POST'),
      headers: { ...headers(extension, account), ...(input === undefined ? {} : { 'content-type': 'application/json' }) },
      ...(input === undefined ? {} : { body: JSON.stringify(input) }),
    });
  } catch {
    throw callFailure(path, 'unreachable');
  }
  if (response.status === 401) window.dispatchEvent(new Event('colecto-auth-required'));
  const data = await response.json().catch(() => null);
  if (!response.ok) throw callFailure(path, 'refused', data, response.status);
  if (data === null) throw callFailure(path, 'unreadable', null, response.status);
  return data as T;
}

/** Owner preferences, stored by the Host in Turso. `workspace` is the dock layout. */
export type Corner = 'top-left' | 'top' | 'top-right' | 'right' | 'bottom-right' | 'bottom' | 'bottom-left' | 'left';
export interface NotificationPreferences { enabled: boolean; mail: boolean; approvals: boolean; system: boolean; position: Corner; updates: boolean }
/** Message list rows: two lines with a preview, a shorter version, or one line. */
export type ListDensity = 'full' | 'medium' | 'compact';
export interface Preferences { mcp: boolean; account: string | null; workspace: unknown; accent: string | null; font: string | null; font_size: number | null; notifications: NotificationPreferences; remote_images: boolean; list_density: ListDensity; date_groups: boolean; notified_release: string | null; accounts_enabled: Record<string, boolean>; onboarded: boolean; prefetch: PrefetchPreferences }
/** A stable release newer than the running build, from the Host's cached GitHub check. */
export interface LatestRelease { current: string; latest: { version: string; url: string } | null }
/** Accounts the owner never switched use this: the demo account starts off, every other account on. */
export const accountOn = (choices: Record<string, boolean>, id: string) => choices[id] ?? id !== 'demo';
/** After each Inbox refresh, download the newest `count` uncached messages (0–100). */
export interface PrefetchPreferences { enabled: boolean; count: number }
/** An empty accent restores the default. */
/** `accounts_enabled` entries are merged into the stored choices. */
export type PreferencesInput = Partial<Omit<Preferences, 'accent'>> & { accent?: string };
export const hostService = {
  preferences: () => request<Preferences>('host', 'demo', '/preferences'),
  savePreferences: (input: PreferencesInput) => request<Preferences>('host', 'demo', '/preferences', input),
  latestRelease: () => request<LatestRelease>('host', 'demo', '/releases/latest'),
  sendRequests: () => request<PendingSend[]>('host', 'demo', '/send-requests'),
  decideSend: (id: string, decision: 'approve' | 'reject') => request<PendingSend>('host', 'demo', `/send-requests/${encodeURIComponent(id)}/${decision}`, {}),
  apiKeys: () => request<ApiKey[]>('host', 'demo', '/api-keys'),
  mintApiKey: (input: ApiKeyInput) => request<{key: ApiKey; secret: string}>('host', 'demo', '/api-keys', input),
  revokeApiKey: (id: string) => request<boolean>('host', 'demo', `/api-keys/${encodeURIComponent(id)}`, {}, 'DELETE'),
  accounts: () => request<AccountInfo[]>('host', 'demo', '/accounts'),
  save: (input: AccountSettings & { id?: string; secret: string }) => request<AccountInfo>('host', 'demo', '/accounts', input),
  /** Looks up server settings for an address (provider list, autoconfig, DNS); waits up to ~9 s. */
  discover: (address: string) => request<Discovery | null>('host', 'demo', '/accounts/discover', { address }),
  remove: (id: string) => request<boolean>('host', 'demo', `/accounts/${encodeURIComponent(id)}`, {}, 'DELETE'),
  disconnect: (id: string) => request<boolean>('host', 'demo', `/accounts/${encodeURIComponent(id)}/disconnect`, {}),
  security: () => request<SecurityInfo>('host', 'demo', '/security'),
  /** Linux user services this app can install (local service only). */
  services: () => request<Record<'daemon' | 'notifications', BackgroundService>>('host', 'demo', '/services'),
  serviceAction: (name: 'daemon' | 'notifications', input: { action: 'install'; autostart: boolean } | { action: 'autostart'; enabled: boolean } | { action: 'start' | 'stop' | 'uninstall' | 'update' }) => request<boolean>('host', 'demo', `/services/${name}`, input),
  /** The daemon (socket or remote origin) this app forwards to, or null for its built-in service. */
  upstream: async () => (await request<{ upstream: Upstream | null }>('host', 'demo', '/upstream')).upstream,
  setUpstream: async (value: UpstreamInput | null) => (await request<{ upstream: Upstream | null }>('host', 'demo', '/upstream', value, 'PUT')).upstream,
  sync: () => request<AccountSync[]>('host', 'demo', '/sync'),
  securityAction: (action: string, password = '') => request<SecurityInfo>('host', 'demo', '/security', { action, password }),
};

export type Capability = 'extensions.bus' | 'account.read' | 'messages.list' | 'messages.read' | 'drafts.list' | 'drafts.read' | 'drafts.write' | 'messages.send' | 'messages.organize' | 'messages.label' | 'app.navigate'
  | 'ui.frame' | 'ui.frame.remote' | 'ui.commands' | 'ui.app_actions' | 'ui.toasts' | 'ui.contribute' | 'ui.canvas' | 'approvals.notify' | 'releases.read' | 'ai.text' | 'ai.decide' | 'ai.configure';
export interface ExtensionState {
  manifest: {
    settings?: ExtensionSetting[]; id: string; name: string; description: string; author: string; api_version: number; capabilities: Capability[];
    /** The client part's entry; bundled extensions not yet on the runtime have none. */
    client?: string; service?: string; pane?: { icon: string }; overlay?: { z_index: number }; settings_page?: { title: string; icon: string }; navigation?: string[];
  };
  bundled: boolean;
  enabled: boolean;
  /** The release that discontinued this extension's API version; it never runs. */
  discontinued?: string;
}
/** A review: a `discontinued` package cannot be installed. */
export interface ExtensionReview { manifest: ExtensionState['manifest']; discontinued?: string }
/** The release notes section that explains why a release dropped an extension API version. */
export const breakingApiNotes = (release: string) => `https://github.com/hexvale-labs/colecto/releases/tag/${encodeURIComponent(release)}#breaking-api-changes`;
async function uploadExtension<T>(path: string, pkg: Uint8Array): Promise<T> {
  let response: Response;
  try { response = await fetch(`/api/v1/extensions${path}`, { method: 'POST', credentials: 'omit', headers: { ...headers('host','demo'), 'content-type': 'application/octet-stream' }, body: new Blob([pkg as BlobPart]) }); }
  catch { throw new ServiceError('Cannot reach Colecto. Try again when the service is available.', 'network'); }
  if (response.status === 401) window.dispatchEvent(new Event('colecto-auth-required'));
  const data = await response.json().catch(() => null);
  if (!response.ok || data === null) throw new ServiceError(data?.error ?? 'Extension upload failed.',data?.category ?? 'invalid_response',response.status);
  return data as T;
}
export const extensionHost = {
  settings: (id: string) => request<ExtensionSettings>("host", "demo", `/extensions/${encodeURIComponent(id)}/settings`),
  saveSettings: (id: string, values: ExtensionSettings["values"]) => request<ExtensionSettings>("host", "demo", `/extensions/${encodeURIComponent(id)}/settings`, values, "PUT"),
  /** `pkg` is a .colectoext file: a zip with manifest.toml, client/ and an optional service module. */
  install: (pkg: Uint8Array) => uploadExtension<boolean>('', pkg),
  review: (pkg: Uint8Array) => uploadExtension<ExtensionReview>('/review', pkg),
  asset: (id: string, name: string) => download('host', 'demo', `/extensions/${encodeURIComponent(id)}/assets/${encodeURIComponent(name)}`),
  remove: (id: string) => request<boolean>('host', 'demo', `/extensions/${encodeURIComponent(id)}`, {}, 'DELETE'),
  /** A credential for the extension's client part; only the shell's runtime holds it. */
  credential: async (id: string) => (await request<{ token: string }>('host', 'demo', `/extensions/${encodeURIComponent(id)}/credential`, {})).token,
  revokeCredential: (id: string, token: string) => request<boolean>('host', 'demo', `/extensions/${encodeURIComponent(id)}/credential`, { token }, 'DELETE'),
  /** Every file under a third-party extension's `client/`, by its path in the package. */
  clientFiles: async (id: string) => {
    const paths = await request<string[]>('host', 'demo', `/extensions/${encodeURIComponent(id)}/client`);
    const files: Record<string, ArrayBuffer> = {};
    await Promise.all(paths.map(async path => { files[path] = await (await download('host', 'demo', `/extensions/${encodeURIComponent(id)}/client/${path.slice('client/'.length)}`)).arrayBuffer(); }));
    return files;
  },
  list: (account: string) => request<ExtensionState[]>('host', account, '/extensions'),
  update: (account: string, id: string, enabled: boolean) =>
    request<boolean>('host', account, `/extensions/${encodeURIComponent(id)}`, { enabled }),
};

export interface ApiKeyInput { name: string; accounts: string[]; capabilities: Capability[]; expires_at?: number }
export interface ApiKey extends ApiKeyInput { id: string; created_at: number; last_used_at: number | null; revoked_at: number | null }

export interface PendingSend { id: string; requester: string; account: string; from: string; snapshot: DraftInput | null; created_at: number; expires_at: number; status: 'pending' | 'rejected' | 'expired' | 'submitting' | 'sent' | 'uncertain' }

/**
 * Follow the Host's server-sent events (`mail`, `approval`, `navigate`, `sync`), reconnecting with backoff.
 * Uses fetch rather than EventSource so the owner token can travel in a header. Returns a stop function.
 */
export function openEvents(onevent: (name: string, data: unknown) => void, path = '/events', body?: unknown): () => void {
  const controller = new AbortController();
  let delay = 1000;
  void (async () => {
    while (!controller.signal.aborted) {
      try {
        const response = await fetch(`/api/v1${path}`, {
          credentials: 'omit', signal: controller.signal,
          ...(body === undefined ? { headers: headers('host', 'demo') } : { method: 'POST', headers: { ...headers('host', 'demo'), 'content-type': 'application/json' }, body: JSON.stringify(body) }),
        });
        if (!response.ok || !response.body) throw new Error();
        delay = 1000;
        const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
        let buffer = '';
        for (;;) {
          const { value, done } = await reader.read();
          if (done) break;
          buffer += value;
          if (buffer.length > 262_144) throw new Error('Event too large');
          let end: number;
          while ((end = buffer.search(/\r?\n\r?\n/)) >= 0) {
            const block = buffer.slice(0, end); buffer = buffer.slice(end).replace(/^\r?\n\r?\n/, '');
            let name = 'message'; const data: string[] = [];
            for (const line of block.split(/\r?\n/)) {
              if (line.startsWith('event:')) name = line.slice(6).trim();
              else if (line.startsWith('data:')) data.push(line.slice(5).trimStart());
            }
            try { onevent(name, JSON.parse(data.join('\n') || 'null')); } catch { /* Ignore malformed events. */ }
          }
        }
      } catch { /* Reconnect below. */ }
      if (controller.signal.aborted) return;
      await new Promise(resolve => setTimeout(resolve, delay));
      delay = Math.min(delay * 2, 30_000);
    }
  })();
  return () => controller.abort();
}

type EventListener = (name: string, data: unknown) => void;
const eventListeners = new Set<EventListener>();
let closeEvents: (() => void) | undefined;
/** Listen to the Host's event stream. All listeners share one connection: browsers allow only
 *  six per origin, and a stream per extension would starve ordinary requests. */
export function subscribeEvents(onevent: EventListener): () => void {
  eventListeners.add(onevent);
  closeEvents ??= openEvents((name, data) => { for (const listener of [...eventListeners]) listener(name, data); });
  return () => {
    eventListeners.delete(onevent);
    if (!eventListeners.size) { closeEvents?.(); closeEvents = undefined; }
  };
}

export interface ExtensionSetting { key: string; label: string; description?: string; type: "toggle" | "range" | "choice" | "text"; default: boolean | number | string; min?: number; max?: number; step?: number; unit?: string; options?: {value: string; label: string}[] }
export interface ExtensionSettings { schema: ExtensionSetting[]; values: Record<string, boolean | number | string> }
