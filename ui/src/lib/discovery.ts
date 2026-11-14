// Account setup helpers: what the Host's settings lookup found, how it fills the wizard
// without overwriting the owner's own edits, and the checks shown on the review page.
import type { AccountSettings } from './api';

export type Security = 'tls' | 'starttls';
export interface DiscoveredServer { host: string; port: number; security: Security }
export type DiscoverySource = 'provider_list' | 'autoconfig' | 'ispdb' | 'srv' | 'mx' | 'jmap_well_known';
/** `POST /accounts/discover`; `null` when nothing usable was found. */
export interface Discovery {
  sources: DiscoverySource[]; provider: string | null;
  imap: DiscoveredServer | null; smtp: DiscoveredServer | null; jmap: string | null; username: string | null;
  app_password: boolean; limitation: 'oauth_only' | 'bridge' | 'no_access' | null;
}

/** The wizard's server fields; JMAP keeps host and port apart until it is saved. */
export interface ServerFields {
  protocol: AccountSettings['protocol'];
  imap_host: string; imap_port: number; imap_starttls: boolean;
  smtp_host: string; smtp_port: number;
  jmap_host: string; jmap_port: number;
  username: string;
}
export type Field = keyof ServerFields;
export const blankServers = (): Omit<ServerFields, 'username'> => ({ protocol: 'imap', imap_host: '', imap_port: 993, imap_starttls: false, smtp_host: '', smtp_port: 587, jmap_host: '', jmap_port: 443 });

export const domainOf = (address: string) => address.trim().split('@').pop()?.toLowerCase() ?? '';

/** Splits a JMAP URL into what the form shows: the host plus any path, and the port. Plain HTTP keeps its scheme. */
export function splitJmap(url: string): { host: string; port: number } {
  if (!url) return { host: '', port: 443 };
  try {
    const parsed = new URL(url);
    const rest = parsed.pathname === '/' && !parsed.search ? '' : parsed.pathname + parsed.search;
    const scheme = parsed.protocol === 'https:' ? '' : `${parsed.protocol}//`;
    return { host: scheme + parsed.hostname + rest, port: Number(parsed.port) || (parsed.protocol === 'https:' ? 443 : 80) };
  } catch { return { host: url, port: 443 }; }
}
/** The session URL saved for a JMAP account. Any port typed into the host box is replaced by the chosen one. */
export function joinJmap(host: string, port: number): string {
  const text = host.trim();
  if (!text) return '';
  const scheme = /^http:\/\//i.test(text) ? 'http:' : 'https:';
  try {
    const url = new URL(`${scheme}//${text.replace(/^https?:\/\//i, '')}`);
    url.port = String(port);
    const value = url.href;
    return url.pathname === '/' && !url.search ? value.slice(0, -1) : value;
  } catch { return text; }
}

/** The values a lookup supplies, as wizard fields. */
export function discoveredFields(found: Discovery, address: string): Partial<ServerFields> {
  const fields: Partial<ServerFields> = {};
  if (found.imap && found.smtp) {
    Object.assign(fields, { protocol: 'imap', imap_host: found.imap.host, imap_port: found.imap.port, imap_starttls: found.imap.security === 'starttls', smtp_host: found.smtp.host, smtp_port: found.smtp.port });
  } else if (found.jmap) fields.protocol = 'jmap';
  if (found.jmap) Object.assign(fields, { jmap_host: splitJmap(found.jmap).host, jmap_port: splitJmap(found.jmap).port });
  fields.username = found.username ?? address.trim();
  return fields;
}

/**
 * Applies a lookup for a (new) address. Fields the owner edited are never touched, and
 * when nothing is found every server field stays as it was; only an untouched username
 * follows the address.
 */
export function applyLookup(fields: ServerFields, found: Discovery | null, address: string, touched: ReadonlySet<Field>) {
  const values: Partial<ServerFields> = found ? discoveredFields(found, address) : { username: address.trim() };
  for (const [field, value] of Object.entries(values) as [Field, never][]) if (!touched.has(field)) fields[field] = value;
}

const sourceNames: Record<DiscoverySource, (domain: string) => string> = {
  provider_list: () => 'Colecto’s built-in provider list',
  autoconfig: domain => `Thunderbird Autoconfig published by ${domain}`,
  ispdb: () => 'Thunderbird’s provider database (ISPDB)',
  srv: () => 'DNS SRV records (RFC 6186)',
  mx: () => 'your domain’s MX record',
  jmap_well_known: () => 'JMAP discovery (RFC 8620)',
};
/** One sentence naming every standard a lookup used. */
export function sourceSentence(found: Discovery, domain: string) {
  const names = found.sources.map(source => sourceNames[source](domain));
  return `Filled in using ${new Intl.ListFormat('en', { type: 'conjunction' }).format(names)}.`;
}

/** Likely mistakes worth a second look, per review card. They never block saving. */
export function reviewWarnings(fields: ServerFields): { imap: string[]; smtp: string[]; jmap: string[] } {
  const imap: string[] = [], smtp: string[] = [], jmap: string[] = [];
  if (!fields.imap_starttls && fields.imap_port === 143) imap.push('Port 143 normally uses STARTTLS, not TLS.');
  if (fields.imap_starttls && fields.imap_port === 993) imap.push('Port 993 normally uses TLS, not STARTTLS.');
  if (/^(smtp|mail\.smtp|submission)\./i.test(fields.imap_host)) imap.push('This looks like an outgoing (SMTP) server.');
  if (/^(imap|pop3?)\./i.test(fields.smtp_host)) smtp.push('This looks like an incoming server, not SMTP.');
  if (fields.smtp_port === 25) smtp.push('Many networks block port 25 for apps. Providers usually offer 587 or 465.');
  if (fields.smtp_port === 993 || fields.smtp_port === 143) smtp.push(`Port ${fields.smtp_port} is an IMAP port.`);
  if (/^http:\/\//i.test(fields.jmap_host)) jmap.push('Plain HTTP only works for a server on this device.');
  return { imap, smtp, jmap };
}
