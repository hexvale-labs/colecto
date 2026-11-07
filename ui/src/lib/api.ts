// Version 1 wire contracts mirror colecto-extension-api. Rust validates all input.
export interface Message { id: string; sender: string; address: string; subject: string; body: string; date: string }
export interface Draft { id: string; recipient: string; subject: string; body: string }
export interface DraftInput { id?: string; recipient: string; subject: string; body: string }
export interface MailItem { id: string; name: string; subject: string; preview: string; date: string }

async function request<T>(extension: string, path: string, input?: DraftInput): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`/api/v1${path}`, {
      method: input ? 'POST' : 'GET',
      headers: {
        'x-colecto-extension': extension, 'x-colecto-account': 'demo',
        ...(input ? { 'content-type': 'application/json' } : {}),
      },
      ...(input ? { body: JSON.stringify(input) } : {}),
    });
  } catch {
    throw new Error('Cannot reach Colecto. Check that the local demo is running, then try again.');
  }
  const data = await response.json().catch(() => null);
  if (!response.ok) throw new Error(data?.error ?? `Request failed (${response.status}). Please try again.`);
  return data as T;
}

// Supplied service clients are the only mail access used by bundled UI modules.
export const inboxService = {
  connected: () => request<boolean>('inbox', '/account'),
  messages: () => request<Message[]>('inbox', '/messages'),
  drafts: () => request<Draft[]>('inbox', '/drafts'),
};
export const readerService = { read: (id: string) => request<Message>('reader', `/messages/${encodeURIComponent(id)}`) };
export const composerService = {
  send: (draft: DraftInput) => request<boolean>('composer', '/send', draft),
  read: (id: string) => request<Draft>('composer', `/drafts/${encodeURIComponent(id)}`),
  save: (draft: DraftInput) => request<Draft>('composer', '/drafts', draft),
};
