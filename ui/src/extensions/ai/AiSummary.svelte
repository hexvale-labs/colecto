<script lang="ts">
  import type { ReaderSlot } from '../../lib/reader-slots';
  import { ai, forget, loadAi, loadSummary, summarize, summary } from './state.svelte';
  let { reader, account }: { reader: ReaderSlot; account: string } = $props();
  void loadAi();
  const uid = Math.random().toString(36).slice(2);
  // A saved summary shows without a new request.
  $effect(() => { if (ai.enabled) void loadSummary(account, reader.id); });
  let current = $derived(summary(account, reader.id));
</script>

{#if ai.enabled && (current.text !== null || current.busy || current.error)}
  <section class="ai-summary" aria-labelledby="ai-summary-{uid}" aria-busy={current.busy}>
    <header>
      <h3 id="ai-summary-{uid}">Summary</h3>
      <div class="ai-summary-actions">
        <button class="ai-summary-button" disabled={current.busy} onclick={() => void summarize(account, reader.id)}>{current.busy ? 'Summarizing…' : 'Regenerate'}</button>
        <button class="ai-summary-button" disabled={current.busy} onclick={() => void forget(account, reader.id)}>Delete</button>
      </div>
    </header>
    {#if current.text !== null}<p class="ai-summary-text" class:stale={current.busy}>{current.text}</p>
    {:else if current.busy}<div class="ai-summary-wait" role="status"><span class="sr-only">Summarizing…</span><span></span><span></span></div>{/if}
    {#if current.error}<p class="ai-summary-error" role="alert">{current.error}</p>{/if}
  </section>
{/if}

<style>
  .ai-summary { max-width: 68ch; margin: 0 0 18px; padding: 12px 14px 14px; border-radius: 10px; background: var(--raise); transition: opacity 180ms var(--ease-out), transform 180ms var(--ease-out); }
  @starting-style { .ai-summary { opacity: 0; transform: translateY(-4px); } }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 6px; }
  h3 { color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); font-weight: 600; }
  .ai-summary-actions { display: flex; gap: 2px; margin-right: -6px; }
  .ai-summary-button { height: 26px; padding: 0 8px; border-radius: 6px; color: var(--ink-2); font-size: calc(12.5px * var(--text-scale)); font-weight: 560; transition: background-color 120ms ease, color 120ms ease, transform 120ms var(--ease-out); }
  .ai-summary-button:hover:not(:disabled) { background: var(--control); color: var(--ink); }
  .ai-summary-button:active:not(:disabled) { transform: scale(0.97); }
  .ai-summary-button:disabled { color: var(--ink-3); }
  .ai-summary-text { color: var(--ink-2); font-size: calc(14px * var(--text-scale)); line-height: 1.6; white-space: pre-wrap; overflow-wrap: anywhere; transition: opacity 160ms ease; }
  .ai-summary-text.stale { opacity: 0.55; }
  .ai-summary-wait { display: grid; gap: 9px; padding: 4px 0 2px; }
  .ai-summary-wait span:not(.sr-only) { height: 10px; border-radius: 5px; background: var(--control); animation: pulse 1.4s ease-in-out infinite; }
  .ai-summary-wait span:last-child { width: 62%; animation-delay: 120ms; }
  .ai-summary-error { margin-top: 8px; color: var(--danger); font-size: calc(13px * var(--text-scale)); }
  @media (prefers-reduced-motion: reduce) { .ai-summary { transition: none; } .ai-summary-wait span { animation: none; } }
</style>
