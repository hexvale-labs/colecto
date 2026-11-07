<script lang="ts">
  import { onMount } from 'svelte';
  import { extensionHost } from './lib/api';
  import type { ExtensionView } from './lib/api';
  let { id, account, onback }: { id: string; account: string; onback: () => void } = $props();
  let view = $state<ExtensionView | null>(null);
  let busy = $state(false);
  let error = $state('');
  let generation = 0;
  async function run(action: string) {
    if (busy) return;
    const request = ++generation;
    busy = true; error = '';
    try { const next = await extensionHost.run(account, id, action); if (request === generation) view = next; }
    catch (e) { if (request === generation) error = (e as Error).message; }
    finally { if (request === generation) busy = false; }
  }
  onMount(() => { void run('open'); return () => { ++generation; }; });
</script>
<section class="settings-page" aria-label="Extension panel" aria-busy={busy}>
  <header class="settings-header"><div><div class="eyebrow">EXTENSION · {id}</div><h1>{view?.title ?? 'Loading extension…'}</h1></div><button class="secondary" onclick={onback}>Back to mail</button></header>
  {#if error}<p class="error-box" role="alert">{error}</p><button class="secondary" disabled={busy} onclick={() => void run('open')}>Retry extension</button>{/if}
  {#if view}<p class="extension-text">{view.text}</p><div class="extension-actions">{#each view.actions as action}<button class="secondary" disabled={busy} onclick={() => void run(action)}>{action}</button>{/each}</div>{/if}
  {#if busy}<p role="status">Running extension…</p>{/if}
</section>
<style>.extension-text { white-space: pre-wrap; overflow-wrap: anywhere; } .extension-actions { display: flex; flex-wrap: wrap; gap: 8px; }</style>
