<script lang="ts">
  // Shown instead of the workspace when the mail service the app is connected to does not
  // answer at startup. It keeps retrying, and offers the controls that can bring the service
  // back from here, because Settings lives inside the workspace that cannot load.
  import { onMount } from 'svelte';
  import { hostService } from './lib/api';
  import type { BackgroundService, Upstream } from './lib/api';
  import mark from './assets/logo-square.svg';
  let { message, onretry }: { message: string; onretry: () => Promise<void> } = $props();
  let upstream = $state<Upstream | null>(null);
  let daemon = $state<BackgroundService | null>(null);
  let busy = $state(false);
  let failure = $state('');
  const viaSocket = $derived(!!upstream?.address.startsWith('unix:'));
  // Only the app's own Host answers these; a browser on a daemon gets neither.
  async function inspect() {
    try { upstream = await hostService.upstream(); } catch { upstream = null; }
    if (viaSocket) { try { daemon = (await hostService.services()).daemon; } catch { daemon = null; } }
  }
  async function retry() {
    if (busy) return;
    busy = true;
    try { await onretry(); await inspect(); } finally { busy = false; }
  }
  async function act(run: () => Promise<unknown>) {
    busy = true; failure = '';
    try { await run(); } catch (e) { failure = (e as Error).message; }
    busy = false;
    if (!failure) await retry(); else await inspect();
  }
  const useThisApp = () => act(async () => { await hostService.setUpstream(null); window.location.reload(); });
  onMount(() => {
    void inspect();
    // The service usually comes back by itself (a restart, a network blip): reconnect then.
    const timer = setInterval(() => void retry(), 4000);
    return () => clearInterval(timer);
  });
</script>

<section class="service-down" aria-labelledby="service-down-title">
  <img src={mark} alt="" width="44" height="44" />
  <h1 id="service-down-title">Can’t reach your mail</h1>
  <p>
    {#if viaSocket}The background mail service is not answering.
    {:else if upstream}The mail server at <span class="selectable">{upstream.address}</span> is not answering.
    {:else}The mail service is not answering.{/if}
    Colecto keeps trying and opens your mail as soon as it is back.
  </p>
  {#if daemon?.installed && !daemon.active}<p>The service is installed but not running. <code class="selectable">journalctl --user -u colecto-daemon</code> shows why.</p>{/if}
  <p class="service-down-detail selectable">{message}</p>
  {#if failure}<p class="error-box" role="alert">{failure}</p>{/if}
  <div class="service-down-actions">
    {#if daemon?.update_to}
      <button class="primary" disabled={busy} onclick={() => act(() => hostService.serviceAction('daemon', { action: 'update' }))}>Update the mail service</button>
    {:else if daemon?.installed && !daemon.active}
      <button class="primary" disabled={busy} onclick={() => act(() => hostService.serviceAction('daemon', { action: 'start' }))}>Start the mail service</button>
    {/if}
    <button class="secondary" disabled={busy} onclick={() => void retry()}>{busy ? 'Trying…' : 'Try again'}</button>
    {#if upstream}<button class="text-button" disabled={busy} onclick={useThisApp}>Use this app’s own mail instead</button>{/if}
  </div>
</section>
