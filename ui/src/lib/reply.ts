import type { Draft, Message, MessageContent } from './api';

export type ReplyMode = 'reply' | 'all' | 'forward';

const bare = (entry: string) => (entry.match(/<([^>]+)>/)?.[1] ?? entry).trim().toLowerCase();
// Split on commas outside quoted display names.
const split = (list: string) => (list.match(/(?:"[^"]*"|[^,])+/g) ?? []).map(entry => entry.trim()).filter(Boolean);
const prefixed = (prefix: string, subject: string) => new RegExp(`^${prefix}:`, 'i').test(subject.trim()) ? subject : `${prefix}: ${subject}`;
const encoder = new TextEncoder();
// The server accepts 100,000 UTF-8 bytes per body; keep 20,000 free for the owner's own text.
const PREFILL_BYTES = 80_000;
function limitedBody(header: string, text: string, quoted = false): string {
  let result = '';
  let bytes = 0;
  // Iterate code points so truncation never separates an emoji's surrogate pair.
  for (const point of header + (quoted ? '> ' : '') + text.replace(/\n/g, quoted ? '\n> ' : '\n')) {
    const size = encoder.encode(point).length;
    if (bytes + size > PREFILL_BYTES) break;
    result += point; bytes += size;
  }
  return result;
}

/** Prefill an unsaved composer draft from a message the owner is reading. */
export function replyDraft(mode: ReplyMode, message: Message, content: MessageContent | null, self: string): Draft {
  const text = (content?.text || message.body);
  const from = message.sender && message.sender !== message.address ? `${message.sender} <${message.address}>` : message.address;
  if (mode === 'forward') {
    return {
      id: '', recipient: '', subject: prefixed('Fwd', message.subject),
      body: limitedBody(`\n\n---------- Forwarded message ----------\nFrom: ${from}\nDate: ${message.date}\nSubject: ${message.subject}\n${content?.to ? `To: ${content.to}\n` : ''}\n`, text),
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
    body: limitedBody(`\n\nOn ${message.date}, ${from} wrote:\n`, text, true),
    in_reply_to: id, references: [content?.references ?? '', id].join(' ').trim(),
  };
}
