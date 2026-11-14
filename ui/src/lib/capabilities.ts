import type { Capability } from './api';
const labels: Record<Capability, string> = {
  'extensions.bus': 'Talk to other extensions by name',
  'account.read': 'Read account connection status', 'messages.list': 'List messages',
  'messages.read': 'Read messages and attachments', 'drafts.list': 'List drafts',
  'drafts.read': 'Read drafts', 'drafts.write': 'Create and edit drafts',
  'messages.send': 'Send mail', 'messages.organize': 'Organize messages', 'messages.label': 'Manage message labels',
  'app.navigate': 'Navigate the app',
  'ui.frame': 'Show web content in a sealed frame', 'ui.frame.remote': 'Load images from the internet in its frames (servers can learn when, and possibly what, it shows)',
  'ui.commands': 'Add commands to the command runner', 'ui.app_actions': 'Add buttons to the top bar',
  'ui.toasts': 'Show notifications', 'ui.contribute': 'Add content inside other extensions',
  'approvals.notify': 'See send requests waiting for your approval', 'releases.read': 'See when a new Colecto release is out',
  'ai.text': 'Use your AI text model', 'ai.decide': 'Use your AI decision model', 'ai.configure': 'Change AI settings and keys',
};
export const capabilityLabel = (capability: Capability) => labels[capability];
