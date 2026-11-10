import type { Draft, Message, MessageContent } from './api';

export type ReplyMode = 'reply' | 'all' | 'forward';

const bare = (entry: string) => (entry.match(/<([^>]+)>/)?.[1] ?? entry).trim().toLowerCase();
// Split on commas outside quoted display names.
const split = (list: string) => (list.match(/(?:"[^"]*"|[^,])+/g) ?? []).map(entry => entry.trim()).filter(Boolean);
const prefixed = (prefix: string, subject: string) => new RegExp(`^${prefix}:`, 'i').test(subject.trim()) ? subject : `${prefix}: ${subject}`;
const quote = (text: string) => text.split('\n').map(line => `> ${line}`).join('\n');

/** Prefill an unsaved composer draft from a message the owner is reading. */
export function replyDraft(mode: ReplyMode, message: Message, content: MessageContent | null, self: string): Draft {
  const text = (content?.text || message.body).slice(0, 80_000);
  const from = message.sender && message.sender !== message.address ? `${message.sender} <${message.address}>` : message.address;
  if (mode === 'forward') {
    return {
      id: '', recipient: '', subject: prefixed('Fwd', message.subject),
      body: `\n\n---------- Forwarded message ----------\nFrom: ${from}\nDate: ${message.date}\nSubject: ${message.subject}\n${content?.to ? `To: ${content.to}\n` : ''}\n${text}`,
      ...(content?.attachments.length ? { forward_of: message.id } : {}),
    };
  }
  const me = bare(self);
  const seen = new Set([me]);
  const unique = (entries: string[]) => entries.filter(entry => { const key = bare(entry); if (!key || seen.has(key)) return false; seen.add(key); return true; });
  const to = unique(split(content?.reply_to || from));
  const cc = mode === 'all' ? unique([...split(content?.to ?? ''), ...split(content?.cc ?? '')]) : [];
  const id = content?.message_id ?? '';
  return {
    id: '', recipient: to.join(', '), cc: cc.join(', '), subject: prefixed('Re', message.subject),
    body: `\n\nOn ${message.date}, ${from} wrote:\n${quote(text)}`,
    in_reply_to: id, references: [content?.references ?? '', id].join(' ').trim(),
  };
}
