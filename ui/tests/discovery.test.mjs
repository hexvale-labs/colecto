import { expect, test } from 'bun:test';
import { applyLookup, blankServers, joinJmap, reviewWarnings, sourceSentence, splitJmap } from '../src/lib/discovery.ts';

const found = { sources: ['mx', 'ispdb'], provider: 'Google', jmap: null, username: 'jo@example.org', app_password: false, limitation: null,
  imap: { host: 'imap.gmail.com', port: 993, security: 'tls' }, smtp: { host: 'smtp.gmail.com', port: 587, security: 'starttls' } };

test('a lookup never overwrites fields the owner edited', () => {
  const fields = { ...blankServers(), username: '', smtp_host: 'smtp.mine.example' };
  applyLookup(fields, found, 'jo@example.org', new Set(['smtp_host']));
  expect(fields).toMatchObject({ imap_host: 'imap.gmail.com', smtp_host: 'smtp.mine.example', smtp_port: 587, username: 'jo@example.org' });
});

test('finding nothing keeps every server field and only moves an untouched username', () => {
  const fields = { ...blankServers(), imap_host: 'imap.old.example', username: 'jo@old.example' };
  applyLookup(fields, null, 'jo@new.example', new Set());
  expect(fields).toMatchObject({ imap_host: 'imap.old.example', username: 'jo@new.example' });
  applyLookup(fields, null, 'sam@new.example', new Set(['username']));
  expect(fields.username).toBe('jo@new.example');
});

test('a JMAP-only lookup switches the protocol and splits the URL', () => {
  const fields = { ...blankServers(), username: '' };
  applyLookup(fields, { ...found, imap: null, smtp: null, jmap: 'https://api.example.org/jmap/session' }, 'jo@example.org', new Set());
  expect(fields).toMatchObject({ protocol: 'jmap', jmap_host: 'api.example.org/jmap/session', jmap_port: 443 });
});

test('JMAP URLs round-trip with any port', () => {
  expect(joinJmap('mail.example.org', 443)).toBe('https://mail.example.org');
  expect(joinJmap('https://mail.example.org:9000/jmap', 8443)).toBe('https://mail.example.org:8443/jmap');
  expect(joinJmap('http://localhost', 8080)).toBe('http://localhost:8080');
  expect(splitJmap('http://localhost:8080')).toEqual({ host: 'http://localhost', port: 8080 });
  expect(splitJmap('https://mail.example.org/.well-known/jmap')).toEqual({ host: 'mail.example.org/.well-known/jmap', port: 443 });
});

test('the note names every standard used, and the review flags likely mistakes', () => {
  expect(sourceSentence(found, 'example.org')).toBe('Filled in using your domain’s MX record and Thunderbird’s provider database (ISPDB).');
  const warnings = reviewWarnings({ ...blankServers(), username: '', imap_port: 143, smtp_port: 25, smtp_host: 'imap.example.org' });
  expect(warnings.imap).toHaveLength(1);
  expect(warnings.smtp).toHaveLength(2);
  expect(reviewWarnings({ ...blankServers(), username: '' })).toEqual({ imap: [], smtp: [], jmap: [] });
});
