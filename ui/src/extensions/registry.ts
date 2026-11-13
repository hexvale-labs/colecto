// Executable code is a build-time allowlist. Manifests never select import URLs.
import Mailbox from './inbox/Mailbox.svelte';
import Reader from './reader/Reader.svelte';
import Composer from './composer/Composer.svelte';
export const mailExtensions = { inbox: Mailbox, reader: Reader, composer: Composer };
import Notifications from './notifications/Toasts.svelte';
// The mail shell places these in the reader's slots and in Settings → AI.
import AiActions from './ai/AiActions.svelte';
import AiSummary from './ai/AiSummary.svelte';
import AiSettings from './ai/AiSettings.svelte';
export const aiExtension = { actions: AiActions, summary: AiSummary, settings: AiSettings };
export const overlayExtensions = { notifications: Notifications };
