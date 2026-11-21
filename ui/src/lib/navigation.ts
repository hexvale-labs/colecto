// The pages Colecto can open. The command runner lists them and automation opens them through
// MCP `app.navigate`; Rust's NAVIGATION_TARGETS mirrors the targets (a unit test keeps them equal).
import type { IconName } from '../Icon.svelte';

export const settingsPages = [
  { id: 'accounts', name: 'Accounts', icon: 'account' },
  { id: 'appearance', name: 'Appearance', icon: 'palette' },
  { id: 'notifications', name: 'Notifications', icon: 'bell' },
  { id: 'servers', name: 'Connection', icon: 'server' },
  { id: 'extensions', name: 'Extensions', icon: 'extension' },
  { id: 'api-keys', name: 'API keys', icon: 'key' },
  { id: 'send-approvals', name: 'Send approvals', icon: 'shield' },
] as const satisfies readonly { id: string; name: string; icon: IconName }[];
export type SettingsPage = typeof settingsPages[number]['id'] | `extension:${string}`;

export type Folder = 'Inbox' | 'Drafts' | 'Sent';

/** Bundled extensions whose workspace panes keep their original layout IDs. */
export const bundledPanes: Record<string, string> = { inbox: 'mailbox', reader: 'reader' };

/** Settings sections that bundled extensions draw, by their old page name. */
const extensionPages = {
  mail: { extension: 'inbox', name: 'Mail' },
  privacy: { extension: 'reader', name: 'Privacy' },
  ai: { extension: 'ai', name: 'AI' },
} as const;
/** The Settings sidebar's order for core and bundled sections. */
const settingsOrder = ['accounts', 'mail', 'appearance', 'notifications', 'privacy', 'servers', 'extensions', 'ai', 'api-keys', 'send-approvals'];
/** The Settings sidebar's order using `SettingsPage` identifiers (`extension:<id>` for bundled sections). */
export const builtInSettingsOrder: readonly string[] = settingsOrder.map(
  page => page in extensionPages ? `extension:${extensionPages[page as keyof typeof extensionPages].extension}` : page,
);
export type Destination = 'inbox' | 'drafts' | 'sent' | 'compose' | `settings/${SettingsPage}` | `settings/${keyof typeof extensionPages}`;

export const destinations: { target: Destination; name: string }[] = [
  { target: 'compose', name: 'Compose a new message' },
  ...(['Inbox', 'Drafts', 'Sent'] as const).map(folder => ({ target: folder.toLowerCase() as Destination, name: `Go to ${folder}` })),
  // Sections bundled extensions own keep their old destinations; all follow the sidebar's order.
  ...[...settingsPages.map(page => ({ page: page.id as string, name: page.name })), ...Object.entries(extensionPages).map(([page, { name }]) => ({ page, name }))]
    .sort((a, b) => settingsOrder.indexOf(a.page) - settingsOrder.indexOf(b.page))
    .map(({ page, name }) => ({ target: `settings/${page}` as Destination, name: `Settings: ${name}` })),
];

export function isDestination(value: unknown): value is Destination {
  return destinations.some(destination => destination.target === value);
}

/** What a destination opens: a folder, the composer, or a Settings page. */
export function resolve(target: Destination): { folder: Folder } | { compose: true } | { settings: SettingsPage } {
  if (target === 'compose') return { compose: true };
  const page = target.slice('settings/'.length);
  if (page in extensionPages) return { settings: `extension:${extensionPages[page as keyof typeof extensionPages].extension}` };
  if (target.startsWith('settings/')) return { settings: target.slice('settings/'.length) as SettingsPage };
  return { folder: (target[0].toUpperCase() + target.slice(1)) as Folder };
}
