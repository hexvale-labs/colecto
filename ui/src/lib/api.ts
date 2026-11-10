// Version 1 wire contracts mirror colecto-extension-api. Rust validates all input.
export type Labels = Record<string, string[]>;
export interface Message { id: string; sender: string; address: string; subject: string; body: string; date: string }
export interface MailSnapshot { folder: string; messages: Message[]; fetched_at: number | null }
export class ServiceError extends Error { constructor(message: string, readonly category: string, readonly status?: number) { super(message); this.name = 'ServiceError'; } }
export interface Draft { id: string; recipient: string; cc?: string; subject: string; body: string; in_reply_to?: string; references?: string; forward_of?: string }
export interface OutgoingAttachment { name: string; content_type: string; data: string }
export interface DraftInput extends Omit<Draft, 'id'> { id?: string; attachments?: OutgoingAttachment[] }
export interface AttachmentInfo { index: number; name: string; content_type: string; size: number }
export interface MessageContent { text: string; html: string | null; attachments: AttachmentInfo[]; message_id: string; references: string; reply_to: string; to: string; cc: string }
export interface MailItem { id: string; name: string; subject: string; preview: string; date: string }
export interface AccountSettings { name: string; address: string; protocol: 'imap' | 'jmap'; imap_host: string; imap_port: number; smtp_host: string; smtp_port: number; username: string; session_url: string }
export interface AccountInfo { id: string; settings: AccountSettings | null; connected: boolean; configured: boolean }
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
async function request<T>(extension: string, account: string, path: string, input?: unknown, method?: 'DELETE'): Promise<T> {
  const delivery = path === '/send' || (path.startsWith('/send-requests/') && path.endsWith('/approve'));
  const deliveryWarning = 'Delivery status is unknown. Check Sent and delivery requests before sending again.';
  let response: Response;
  try {
    response = await fetch(`/api/v1${path}`, {
      credentials: 'omit',
      method: method ?? (input === undefined ? 'GET' : 'POST'),
      headers: { ...headers(extension, account), ...(input === undefined ? {} : { 'content-type': 'application/json' }) },
      ...(input === undefined ? {} : { body: JSON.stringify(input) }),
    });
  } catch {
    throw new ServiceError(delivery ? deliveryWarning : 'Cannot reach Colecto. Check that your mail service is running, then try again.', delivery ? 'delivery_uncertain' : 'network');
  }
  if (response.status === 401) window.dispatchEvent(new Event('colecto-auth-required'));
  const data = await response.json().catch(() => null);
  if (!response.ok) throw new ServiceError(typeof data?.error === 'string' ? data.error : `Mail service returned HTTP ${response.status}. ${delivery ? 'Check delivery status before sending again.' : 'Try again when the service is available.'}`, typeof data?.category === 'string' ? data.category : 'server', response.status);
  if (data === null) throw new ServiceError(delivery ? deliveryWarning : 'Mail service returned an unreadable response. Check your connection and try again.', delivery ? 'delivery_uncertain' : 'invalid_response', response.status);
  return data as T;
}

// Supplied service clients capture an immutable account, including during async work.
export function createServices(account: string) {
  return {
    inbox: {
      cached: (folder: 'inbox' | 'sent' = 'inbox') => request<MailSnapshot>('inbox', account, `/messages/cache?folder=${folder}`),
      labels: () => request<Labels>('inbox', account, '/labels'),
      connected: () => request<boolean>('inbox', account, '/account'),
      messages: (folder: 'inbox' | 'sent' = 'inbox', query = '') => { const params = new URLSearchParams(); if (folder !== 'inbox') params.set('folder', folder); if (query) params.set('q', query); return request<Message[]>('inbox', account, `/messages/refresh${params.size ? `?${params}` : ''}`, {}); },
      drafts: () => request<Draft[]>('inbox', account, '/drafts'),
    },
    reader: { label: (name: string, message_id?: string, remove = false) => request<Labels>('reader', account, '/labels', { name, message_id, remove }), read: (id: string) => request<Message>('reader', account, `/messages/${encodeURIComponent(id)}`),
      content: (id: string) => request<MessageContent>('reader', account, `/messages/${encodeURIComponent(id)}/content`),
      attachment: (id: string, index: number) => download('reader', account, `/messages/${encodeURIComponent(id)}/attachments/${index}`) },
    composer: {
      send: (draft: DraftInput) => request<boolean>('composer', account, '/send', draft),
      read: (id: string) => request<Draft>('composer', account, `/drafts/${encodeURIComponent(id)}`),
      save: (draft: DraftInput) => request<Draft>('composer', account, '/drafts', draft),
    },
  };
}
export type ReaderService = ReturnType<typeof createServices>['reader'];
export type ComposerService = ReturnType<typeof createServices>['composer'];
export const hostService = {
  sendRequests: () => request<PendingSend[]>('host', 'demo', '/send-requests'),
  decideSend: (id: string, decision: 'approve' | 'reject') => request<PendingSend>('host', 'demo', `/send-requests/${encodeURIComponent(id)}/${decision}`, {}),
  apiKeys: () => request<ApiKey[]>('host', 'demo', '/api-keys'),
  mintApiKey: (input: ApiKeyInput) => request<{key: ApiKey; secret: string}>('host', 'demo', '/api-keys', input),
  revokeApiKey: (id: string) => request<boolean>('host', 'demo', `/api-keys/${encodeURIComponent(id)}`, {}, 'DELETE'),
  accounts: () => request<AccountInfo[]>('host', 'demo', '/accounts'),
  save: (input: AccountSettings & { id?: string; secret: string }) => request<AccountInfo>('host', 'demo', '/accounts', input),
  disconnect: (id: string) => request<boolean>('host', 'demo', `/accounts/${encodeURIComponent(id)}/disconnect`, {}),
  security: () => request<SecurityInfo>('host', 'demo', '/security'),
  securityAction: (action: string, password = '') => request<SecurityInfo>('host', 'demo', '/security', { action, password }),
};

export type Capability = 'account.read' | 'messages.list' | 'messages.read' | 'drafts.list' | 'drafts.read' | 'drafts.write' | 'messages.send' | 'messages.label';
export interface ExtensionState {
  manifest: { id: string; name: string; description: string; author: string; api_version: number; slot: string; capabilities: Capability[] };
  bundled: boolean;
  enabled: boolean;
  grants: Capability[];
}
export interface ExtensionView { title: string; text: string; actions: string[] }
export const extensionHost = {
  install: (manifest: string, module: number[]) => request<boolean>('host', 'demo', '/extensions', { manifest, module }),
  remove: (id: string) => request<boolean>('host', 'demo', `/extensions/${encodeURIComponent(id)}`, {}, 'DELETE'),
  run: (account: string, id: string, action: string) => request<ExtensionView>('host', account, `/extensions/${encodeURIComponent(id)}/run`, { action }),
  list: (account: string) => request<ExtensionState[]>('host', account, '/extensions'),
  update: (account: string, id: string, enabled: boolean, grants: Capability[]) =>
    request<boolean>('host', account, `/extensions/${encodeURIComponent(id)}`, { enabled, grants }),
};

export interface ApiKeyInput { name: string; accounts: string[]; capabilities: Capability[]; expires_at?: number }
export interface ApiKey extends ApiKeyInput { id: string; created_at: number; last_used_at: number | null; revoked_at: number | null }

export interface PendingSend { id: string; requester: string; account: string; from: string; snapshot: DraftInput | null; created_at: number; expires_at: number; status: 'pending' | 'rejected' | 'expired' | 'submitting' | 'sent' | 'uncertain' }
