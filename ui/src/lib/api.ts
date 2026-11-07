// Version 1 wire contracts mirror colecto-extension-api. Rust validates all input.
export interface Message { id: string; sender: string; address: string; subject: string; body: string; date: string }
export interface Draft { id: string; recipient: string; subject: string; body: string }
export interface DraftInput { id?: string; recipient: string; subject: string; body: string }
export interface MailItem { id: string; name: string; subject: string; preview: string; date: string }
export interface AccountSettings { name: string; address: string; protocol: 'imap' | 'jmap'; imap_host: string; imap_port: number; smtp_host: string; smtp_port: number; username: string; session_url: string }
export interface AccountInfo { id: string; settings: AccountSettings | null; connected: boolean; configured: boolean }
export interface SecurityInfo { cleanup_pending: boolean; protected: boolean; locked: boolean }

let transientToken = '';
export function setAccessToken(token: string) { transientToken = token; try { if (token) sessionStorage.setItem('colecto.token', token); else sessionStorage.removeItem('colecto.token'); } catch { /* Session storage may be disabled. */ } }
function accessToken() { try { return sessionStorage.getItem('colecto.token') ?? ''; } catch { return transientToken; } }

async function request<T>(extension: string, account: string, path: string, input?: unknown, method?: 'DELETE'): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`/api/v1${path}`, {
      method: method ?? (input === undefined ? 'GET' : 'POST'),
      headers: {
        ...(accessToken() ? { Authorization: `Bearer ${accessToken()}` } : {}),
        'x-colecto-extension': extension, 'x-colecto-account': account,
        ...(input === undefined ? {} : { 'content-type': 'application/json' }),
      },
      ...(input === undefined ? {} : { body: JSON.stringify(input) }),
    });
  } catch {
    throw new Error('Cannot reach Colecto. Check that your mail service is running, then try again.');
  }
  if (response.status === 401) window.dispatchEvent(new Event('colecto-auth-required'));
  const data = await response.json().catch(() => null);
  if (!response.ok) throw new Error(data?.error ?? `Request failed (${response.status}). Please try again.`);
  return data as T;
}

// Supplied service clients capture an immutable account, including during async work.
export function createServices(account: string) {
  return {
    inbox: {
      connected: () => request<boolean>('inbox', account, '/account'),
      messages: (folder: 'inbox' | 'sent' = 'inbox', query = '') => { const params = new URLSearchParams(); if (folder !== 'inbox') params.set('folder', folder); if (query) params.set('q', query); return request<Message[]>('inbox', account, `/messages${params.size ? `?${params}` : ''}`); },
      drafts: () => request<Draft[]>('inbox', account, '/drafts'),
    },
    reader: { read: (id: string) => request<Message>('reader', account, `/messages/${encodeURIComponent(id)}`) },
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
  accounts: () => request<AccountInfo[]>('host', 'demo', '/accounts'),
  save: (input: AccountSettings & { id?: string; secret: string }) => request<AccountInfo>('host', 'demo', '/accounts', input),
  disconnect: (id: string) => request<boolean>('host', 'demo', `/accounts/${encodeURIComponent(id)}/disconnect`, {}),
  security: () => request<SecurityInfo>('host', 'demo', '/security'),
  securityAction: (action: string, password = '') => request<SecurityInfo>('host', 'demo', '/security', { action, password }),
};

export type Capability = 'account.read' | 'messages.list' | 'messages.read' | 'drafts.list' | 'drafts.read' | 'drafts.write' | 'messages.send';
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
