// The pages Colecto can open. The command runner lists them and automation opens them through
// MCP `app.navigate`; Rust's NAVIGATION_TARGETS mirrors the targets (a unit test keeps them equal).
import type { IconName } from '../Icon.svelte';

export const settingsPages = [
  { id: 'accounts', name: 'Accounts', icon: 'account' },
  { id: 'appearance', name: 'Appearance', icon: 'palette' },
  { id: 'notifications', name: 'Notifications', icon: 'bell' },
  { id: 'privacy', name: 'Privacy', icon: 'image' },
  { id: 'servers', name: 'Connection', icon: 'server' },
  { id: 'extensions', name: 'Extensions', icon: 'extension' },
  { id: 'ai', name: 'AI', icon: 'cpu' },
  { id: 'api-keys', name: 'API keys', icon: 'key' },
  { id: 'send-approvals', name: 'Send approvals', icon: 'shield' },
] as const satisfies readonly { id: string; name: string; icon: IconName }[];
export type SettingsPage = typeof settingsPages[number]['id'] | `extension:${string}`;

export type Folder = 'Inbox' | 'Drafts' | 'Sent';
export type Destination = 'inbox' | 'drafts' | 'sent' | 'compose' | `settings/${SettingsPage}`;

export const destinations: { target: Destination; name: string }[] = [
  { target: 'compose', name: 'Compose a new message' },
  ...(['Inbox', 'Drafts', 'Sent'] as const).map(folder => ({ target: folder.toLowerCase() as Destination, name: `Go to ${folder}` })),
  ...settingsPages.map(page => ({ target: `settings/${page.id}` as Destination, name: `Settings: ${page.name}` })),
];

export function isDestination(value: unknown): value is Destination {
  return destinations.some(destination => destination.target === value);
}

/** What a destination opens: a folder, the composer, or a Settings page. */
export function resolve(target: Destination): { folder: Folder } | { compose: true } | { settings: SettingsPage } {
  if (target === 'compose') return { compose: true };
  if (target.startsWith('settings/')) return { settings: target.slice('settings/'.length) as SettingsPage };
  return { folder: (target[0].toUpperCase() + target.slice(1)) as Folder };
}
