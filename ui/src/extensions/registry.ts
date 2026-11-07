// Executable code is a build-time allowlist. Manifests never select import URLs.
import Mailbox from './Mailbox.svelte';
import Reader from './Reader.svelte';
import Composer from './Composer.svelte';
export const mailExtensions = { inbox: Mailbox, reader: Reader, composer: Composer };
