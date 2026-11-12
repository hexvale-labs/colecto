import type { Capability } from './api';
const labels: Record<Capability, string> = {
  'extensions.bus': 'Talk to other extensions by name',
  'account.read': 'Read account connection status', 'messages.list': 'List messages',
  'messages.read': 'Read messages and attachments', 'drafts.list': 'List drafts',
  'drafts.read': 'Read drafts', 'drafts.write': 'Create and edit drafts',
  'messages.send': 'Send mail', 'messages.organize': 'Organize messages', 'messages.label': 'Manage message labels',
  'window.overlay': 'Draw over the workspace', 'app.navigate': 'Navigate the app',
};
export const capabilityLabel = (capability: Capability) => labels[capability];
