<script lang="ts">
  // What to notify about. The toast surface itself is core UI (shell/Toasts.svelte).
  import { onMount } from 'svelte';
  import { extensionBus, extensionName } from '../../lib/bus';
  import { notify } from '../../shell/toasts.svelte';
  import { checkForUpdate } from './updates';
  const bus = extensionBus('notifications');
  onMount(() => {
    void checkForUpdate().catch(() => { /* An update check that fails stays quiet. */ });
    return bus.handle('v1/notify', async (p, from) => {
      if (typeof p?.title === 'string' && typeof p?.body === 'string') notify({ title: `${await extensionName(from)}: ${p.title}`, body: p.body });
      return null;
    });
  });
</script>
