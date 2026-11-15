// Decides what to notify about and shows it on the shell's toast surface: new mail, send
// requests waiting for approval, and newer Colecto releases (once per version).
import { app, bus, call, clip, storage, toast as show, type Account, type Toast } from '../../../extension-sdk';

interface Message { id: string; sender: string; subject: string }
let accounts: Account[] = [];
let notices = 0;
const opens = new Map<number, { account: string; id: string }>();
// Toasts allow 128-byte titles and 512-byte bodies; mail can be longer.
const toast = (item: Toast) => show({ ...item, title: clip(item.title, 128), ...(item.body ? { body: clip(item.body, 512) } : {}) });
const on = (key: string) => app.settings[key] !== false;
const nameOf = (id: string) => { const account = accounts.find(item => item.id === id); return account?.name || account?.address || 'Demo account'; };

app.on('start', message => { accounts = (message.accounts as Account[]) ?? []; void checkForUpdate(); });
app.on('account', message => { accounts = (message.accounts as Account[]) ?? accounts; });

app.on('host:mail', async message => {
  const { account, ids } = message.data as { account?: string; ids?: string[] };
  if (!on('mail') || typeof account !== 'string' || !Array.isArray(ids) || !ids.length) return;
  // The Host already refreshed, so its saved Inbox has the new messages.
  let single: Message | undefined;
  if (ids.length === 1 && account === app.account?.id) {
    const cached = await call<{ messages: Message[] }>('GET', '/messages/cache', { query: { folder: 'inbox' } }).catch(() => null);
    single = cached?.messages.find(item => item.id === ids[0]);
  }
  // Ids stay short whatever the account and message ids look like; `opens` remembers the target.
  const key = ++notices;
  if (single) opens.set(key, { account, id: single.id });
  toast({
    id: `mail:${key}`, icon: 'mail', system: true,
    title: single ? single.sender || 'New message' : `${ids.length} new message${ids.length === 1 ? '' : 's'}`,
    body: single ? single.subject || '(No subject)' : nameOf(account),
    actions: [single
      ? { id: `open:${key}`, label: 'Open', primary: true }
      : { id: 'show', label: 'Show', primary: true, navigate: 'inbox', account }],
  });
});

app.on('toast_action', async message => {
  const target = opens.get(Number(String(message.action).replace(/^open:/, '')));
  if (!target) return;
  await bus.request('inbox', 'v1/open', { account: target.account, message_id: target.id });
});

app.on('host:approval', message => {
  const { id, account, from, subject } = message.data as { id?: string; account?: string; from?: string; subject?: string };
  if (!on('approvals') || typeof id !== 'string') return;
  // Approvals keep one toast per request, so a repeated event replaces it.
  toast({
    id: `approval:${id.slice(0, 100)}`, icon: 'shield', duration: 0, system: true,
    title: 'Automation wants to send a message',
    body: `“${subject || '(No subject)'}” from ${from || nameOf(account ?? '')}`,
    actions: [{ id: 'review', label: 'Review message', primary: true, review_send: id }, { id: 'reject', label: 'Reject', reject_send: id }],
  });
});

async function checkForUpdate() {
  if (!on('updates')) return;
  try {
    const { current, latest } = await call<{ current: string; latest: { version: string; url: string } | null }>('GET', '/releases/latest');
    if (!latest || await storage.get<string>('notified_release') === latest.version) return;
    toast({ id: 'release', icon: 'download', duration: 15000, title: `Colecto ${latest.version} is available`, body: `You are using ${current}.`,
      actions: [{ id: 'view', label: 'View release', primary: true, url: latest.url }] });
    await storage.set('notified_release', latest.version);
  } catch { /* An update check that fails stays quiet. */ }
}

// Other extensions can ask for a notification; it shows with the requester's ID.
bus.handle('v1/notify', (payload, from) => {
  if (typeof payload?.title === 'string' && typeof payload?.body === 'string') toast({ id: `notify:${++notices}`, title: `${from}: ${payload.title}`, body: payload.body });
  return null;
});
