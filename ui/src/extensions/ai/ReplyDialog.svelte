<script lang="ts" module>
  import type { Tone } from './api';
  export const tones: { value: Tone; label: string; hint: string }[] = [
    { value: 'casual', label: 'Casual', hint: 'Relaxed and friendly.' },
    { value: 'lazy', label: 'Lazy', hint: 'all lowercase, short, the odd “lol”.' },
    { value: 'professional', label: 'Professional', hint: 'Formal and polished, no slang.' },
    { value: 'roast', label: 'Roast', hint: 'Playful comedian jabs, never cruel.' },
    { value: 'troll', label: 'Troll', hint: 'Chaotic replies for scams and ragebait.' },
  ];
</script>
<script lang="ts">
  import type { ReaderSlot } from '../../lib/reader-slots';
  import Icon from '../../Icon.svelte';
  import { aiService } from './api';
  let { open = $bindable(), reader, account, onclose }: { open: boolean; reader: ReaderSlot; account: string; onclose: () => void } = $props();
  const uid = Math.random().toString(36).slice(2);
  let dialog = $state<HTMLDialogElement>();
  // Kept while the message stays open, so closing the dialog never loses what was written.
  let notes = $state('');
  let tone = $state<Tone>('casual');
  let draft = $state('');
  let busy = $state(false);
  let error = $state('');
  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  });
  function close() { open = false; onclose(); }
  async function generate(event?: SubmitEvent) {
    event?.preventDefault();
    if (busy || !notes.trim()) return;
    busy = true; error = '';
    try { draft = (await aiService(account).reply(reader.id, notes, tone)).text; }
    catch (e) { error = (e as Error).message; }
    finally { busy = false; }
  }
  function use() { reader.reply?.(draft.trim() + '\n'); open = false; }
</script>

<dialog bind:this={dialog} class="sheet ai-reply" aria-labelledby="ai-reply-title-{uid}" oncancel={(event) => { event.preventDefault(); close(); }}>
  <form onsubmit={generate} aria-busy={busy}>
    <header class="sheet-head">
      <div><h2 id="ai-reply-title-{uid}">Draft a reply</h2><p>Write the gist in any language or wording. The AI turns it into a reply to “{reader.message.subject || 'this message'}”.</p></div>
      <button type="button" class="icon-button" aria-label="Close" onclick={close}><Icon name="close" size={16} /></button>
    </header>
    <div class="sheet-body">
      <label class="field">What do you want to say?
        <textarea bind:value={notes} rows="3" maxlength="4000" placeholder="e.g. thanks, but I can't make it on friday" readonly={busy}></textarea>
      </label>
      <fieldset class="segmented-field">
        <legend>Tone</legend>
        <div class="segmented fill tones">
          {#each tones as option (option.value)}
            <label><input type="radio" name="tone-{uid}" value={option.value} bind:group={tone} aria-label={option.label} aria-describedby="tone-{option.value}-{uid}" />{option.label}<span class="tip" role="tooltip" id="tone-{option.value}-{uid}">{option.hint}</span></label>
          {/each}
        </div>
      </fieldset>
      {#if draft || busy}
        <label class="field">Reply
          <textarea class="draft" bind:value={draft} rows="8" readonly={busy} placeholder={busy ? 'Writing…' : ''}></textarea>
        </label>
      {/if}
      {#if error}<p class="error-box" role="alert">{error}</p>{/if}
    </div>
    <footer class="sheet-foot">
      {#if draft && !busy}
        <button type="submit" class="secondary">Regenerate</button>
        <button type="button" class="primary" onclick={use}>Use as reply</button>
      {:else}
        <button type="button" class="secondary" onclick={close}>Cancel</button>
        <button type="submit" class="primary" disabled={busy || !notes.trim()}>{busy ? 'Writing…' : 'Generate'}</button>
      {/if}
    </footer>
  </form>
</dialog>

<style>
  .ai-reply :global(.sheet-head p) { overflow-wrap: anywhere; }
  .ai-reply textarea { width: 100%; padding: 10px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--sunk); color: var(--ink); font-size: calc(14px * var(--text-scale)); font-weight: 450; line-height: 1.55; resize: vertical; outline: none; transition: border-color 140ms ease; }
  .ai-reply textarea:focus { border-color: var(--mint); }
  .ai-reply textarea[readonly] { color: var(--ink-2); }
  .ai-reply .draft { min-height: 160px; }
  .tones { flex-wrap: wrap; }
  .tones label { flex: 1 1 auto; justify-content: center; padding: 0 8px; }
  /* Hover waits a moment so passing over the pill stays quiet; keyboard focus shows it at once. */
  .tip { position: absolute; bottom: calc(100% + 8px); left: 50%; z-index: 1; width: max-content; max-width: 280px; padding: 6px 9px; border: 1px solid var(--line); border-radius: 7px; background: var(--float); color: var(--ink-2); font-size: calc(12.5px * var(--text-scale)); line-height: 1.4; white-space: normal; text-align: center; pointer-events: none; visibility: hidden; opacity: 0; transform: translate(-50%, 2px); transition: opacity 120ms ease, transform 120ms var(--ease-out), visibility 0s 120ms; }
  @media (hover: hover) and (pointer: fine) { .tones label:hover .tip { visibility: visible; opacity: 1; transform: translate(-50%, 0); transition-delay: 350ms; } }
  .tones label:has(input:focus-visible) .tip { visibility: visible; opacity: 1; transform: translate(-50%, 0); transition-delay: 0ms; }
  .tones label:first-child .tip { left: 0; transform: translate(0, 2px); }
  .tones label:first-child:has(input:focus-visible) .tip { transform: none; }
  @media (hover: hover) and (pointer: fine) { .tones label:first-child:hover .tip { transform: none; } }
  .tones label:last-child .tip { left: auto; right: 0; transform: translate(0, 2px); }
  .tones label:last-child:has(input:focus-visible) .tip { transform: none; }
  @media (hover: hover) and (pointer: fine) { .tones label:last-child:hover .tip { transform: none; } }
  @media (prefers-reduced-motion: reduce) { .tip { transition: none; } }
</style>
