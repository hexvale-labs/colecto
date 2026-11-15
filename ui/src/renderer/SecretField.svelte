<script lang="ts">
  // A write-only key field: the typed value goes from here to the Host and never to the
  // extension's Worker. Once saved, the Host only says that a key exists.
  import { request } from '../lib/api';
  let { label, target, saved, onsaved }: { label: string; target: 'ai.text' | 'ai.decision'; saved: boolean; onsaved: (saved: boolean) => void } = $props();
  const uid = Math.random().toString(36).slice(2);
  let replacing = $state(false);
  let value = $state('');
  let busy = $state(false);
  let error = $state('');
  async function submit(key: string | null) {
    busy = true; error = '';
    try {
      const view = await request<{ keys: { text: boolean; decision: boolean } }>('host', 'demo', '/ai/keys', { kind: target === 'ai.text' ? 'text' : 'decision', key });
      value = ''; replacing = false;
      onsaved(target === 'ai.text' ? view.keys.text : view.keys.decision);
    } catch (e) { error = (e as Error).message; }
    finally { busy = false; }
  }
</script>

<div class="field key-field">
  {#if saved && !replacing}
    <span id="key-{uid}">{label}</span>
    <div class="key-saved" aria-labelledby="key-{uid}" role="group"><span class="key-state">Saved</span>
      <button type="button" class="text-button" disabled={busy} onclick={() => replacing = true}>Replace</button>
      <button type="button" class="text-button danger-text" disabled={busy} onclick={() => void submit(null)}>Remove</button></div>
  {:else}
    <label for="key-{uid}">{label}</label>
    <div class="key-entry">
      <input id="key-{uid}" type="password" autocomplete="off" spellcheck="false" placeholder={saved ? 'New key' : 'Paste the key; services on your computer often need none'} bind:value
        onkeydown={(event) => { if (event.key === 'Enter') { event.preventDefault(); if (value.trim()) void submit(value.trim()); } }} />
      <button type="button" class="secondary" disabled={busy || !value.trim()} onclick={() => void submit(value.trim())}>Save key</button>
      {#if replacing}<button type="button" class="text-button" onclick={() => { replacing = false; value = ''; }}>Cancel</button>{/if}
    </div>
  {/if}
  {#if error}<p class="error-text" role="alert">{error}</p>{/if}
</div>

<style>
  .key-field { max-width: 560px !important; }
  .key-saved { display: flex; align-items: center; gap: 14px; min-height: 38px; }
  .key-state { color: var(--ink); font-size: calc(14px * var(--text-scale)); font-weight: 450; }
  .key-entry { display: flex; align-items: center; gap: 8px; }
  .key-entry input { flex: 1; }
  .key-entry .secondary { height: 38px; }
  .error-text { color: var(--danger); font-weight: 450; }
</style>
