// Executable code is a build-time allowlist. Manifests never select import URLs.
import Mailbox from './inbox/Mailbox.svelte';
import Reader from './reader/Reader.svelte';
import Composer from './composer/Composer.svelte';
export const mailExtensions = { inbox: Mailbox, reader: Reader, composer: Composer };
