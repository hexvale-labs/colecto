import { expect, test } from 'bun:test';
import { replyDraft } from '../src/lib/reply.ts';

const message = { id: 'imap-1-2', sender: 'Ann', address: 'ann@example.test', subject: 'Plans', body: 'Lunch?\nTomorrow', date: 'Nov 9' };
const content = { text: 'Lunch?\nTomorrow', html: null, attachments: [{ index: 0, name: 'a.pdf', content_type: 'application/pdf', size: 3 }], message_id: '<m2@x>', references: '<m1@x>', reply_to: '', to: 'Me <ME@example.test>, "Doe, Bob" <bob@example.test>', cc: 'ann@example.test, cat@example.test' };

test('reply all threads, quotes, and drops the owner and duplicates', () => {
  const draft = replyDraft('all', message, content, 'me@example.test');
  expect(draft.recipient).toBe('Ann <ann@example.test>');
  expect(draft.cc).toBe('"Doe, Bob" <bob@example.test>, cat@example.test');
  expect(draft.subject).toBe('Re: Plans');
  expect(draft.in_reply_to).toBe('<m2@x>');
  expect(draft.references).toBe('<m1@x> <m2@x>');
  expect(draft.body).toContain('> Lunch?\n> Tomorrow');
  expect(replyDraft('reply', { ...message, subject: 'RE: Plans' }, { ...content, reply_to: 'list@example.test' }, 'me@example.test')).toMatchObject({ recipient: 'list@example.test', cc: '', subject: 'RE: Plans' });
});

test('forward keeps attachments by reference and works offline', () => {
  expect(replyDraft('forward', message, content, 'me@example.test')).toMatchObject({ recipient: '', subject: 'Fwd: Plans', forward_of: 'imap-1-2' });
  const offline = replyDraft('reply', message, null, 'me@example.test');
  expect(offline.recipient).toBe('Ann <ann@example.test>');
  expect(offline.in_reply_to).toBe('');
});
