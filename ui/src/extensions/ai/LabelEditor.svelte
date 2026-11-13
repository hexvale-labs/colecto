<script lang="ts">
  import Icon from '../../Icon.svelte';
  import Select from '../../Select.svelte';
  import { aiHost, type Field, type LabelRule, type Preview } from './api';
  let { name, account, rule, modelReady, onsave, onclose }: { name: string; account: string; rule: LabelRule; modelReady: boolean; onsave: (rule: LabelRule) => Promise<void>; onclose: () => void } = $props();
  const uid = Math.random().toString(36).slice(2);
  // Edits stay local until Save, so half-typed patterns never reach the Host's settings.
  // svelte-ignore state_referenced_locally (the editor starts from the rule it was opened with)
  let draft = $state<LabelRule>(structuredClone($state.snapshot(rule)));
  let preview = $state<Preview | null>(null);
  let previewError = $state('');
  let saving = $state(false);
  let error = $state('');
  const fields: { value: Field; label: string }[] = [{ value: 'any', label: 'Anywhere' }, { value: 'sender', label: 'From' }, { value: 'subject', label: 'Subject' }, { value: 'body', label: 'Body' }];
  let invalid = $derived(!!preview?.errors.some(Boolean));
  function open(node: HTMLDialogElement) { node.showModal(); return { destroy() { node.close(); } }; }
  // Test the rules against the saved Inbox as they change, a moment after typing stops.
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const rules = $state.snapshot(draft);
    clearTimeout(timer);
    if (!rules.rules.length) { preview = null; return; }
    timer = setTimeout(async () => {
      try { preview = await aiHost.preview(account, rules); previewError = ''; }
      catch (e) { previewError = (e as Error).message; }
    }, 250);
    return () => clearTimeout(timer);
  });
  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (invalid) return;
    saving = true; error = '';
    const next = $state.snapshot(draft); next.rules = next.rules.filter(r => r.pattern.trim());
    try { await onsave(next); }
    catch (e) { error = (e as Error).message; }
    finally { saving = false; }
  }
</script>

<dialog class="sheet label-editor" aria-labelledby="label-editor-{uid}" use:open oncancel={(event) => { event.preventDefault(); onclose(); }}>
  <form onsubmit={save}>
    <header class="sheet-head">
      <div><h2 id="label-editor-{uid}">“{name}”</h2><p>Choose which new mail gets this label.</p></div>
      <button type="button" class="icon-button" aria-label="Close" onclick={onclose}><Icon name="close" size={16} /></button>
    </header>
    <div class="sheet-body">
      <div class="setting-row">
        <div><h3>Label new mail automatically</h3></div>
        <label class="switch"><input type="checkbox" bind:checked={draft.enabled} /><span class="sr-only">Label new mail automatically</span></label>
      </div>
      <fieldset class="block">
        <legend>Rules</legend>
        <p class="hint">A message gets the label when any rule matches. Words work as they are, ignoring case: <code>invoice</code> matches “Invoice due”. Use <code>|</code> for “or”, like <code>receipt|invoice</code>.</p>
        {#each draft.rules as current, index (index)}
          {@const problem = preview?.errors[index]}
          <div class="rule">
            <Select label={`Rule ${index + 1}: where to look`} value={current.field} options={fields} onchange={(value) => current.field = value} />
            <input aria-label={`Rule ${index + 1}: text to find`} aria-invalid={!!problem} aria-describedby={problem ? `rule-error-${index}-${uid}` : undefined} placeholder="Text to find" autocomplete="off" spellcheck="false" maxlength="300" bind:value={current.pattern} />
            <button type="button" class="icon-button" aria-label={`Remove rule ${index + 1}`} onclick={() => draft.rules.splice(index, 1)}><Icon name="trash" size={15} /></button>
          </div>
          {#if problem}<p class="rule-error" id="rule-error-{index}-{uid}">{problem}</p>{/if}
        {/each}
        {#if draft.rules.length < 10}<button type="button" class="text-button" onclick={() => draft.rules.push({ field: 'any', pattern: '' })}><Icon name="plus" size={15} />Add a rule</button>{/if}
      </fieldset>
      <fieldset class="block">
        <legend>Decision model</legend>
        <label class="choice"><input type="checkbox" bind:checked={draft.model} />Also ask the decision model about mail no rule matched</label>
        {#if draft.model && !modelReady}<p class="hint">Set up the decision model on the AI page first.</p>{/if}
        <label class="field">What belongs under this label? <span class="optional">Optional, helps the model</span>
          <textarea rows="2" maxlength="500" placeholder="e.g. receipts and order confirmations from shops" bind:value={draft.description}></textarea>
        </label>
      </fieldset>
      <div class="test" aria-live="polite">
        <h3>Test with your Inbox</h3>
        {#if previewError}<p class="rule-error">{previewError}</p>
        {:else if !preview}<p class="hint">Add a rule to see which saved messages it matches. The decision model is not asked here.</p>
        {:else if invalid}<p class="hint">Fix the rule marked above to test it.</p>
        {:else}
          <p class="hint">{preview.checked ? `Matches ${preview.count} of the last ${preview.checked} ${preview.checked === 1 ? 'message' : 'messages'}.` : 'Your Inbox has no saved messages to test with yet.'}</p>
          {#if preview.matches.length}<ul class="matches">{#each preview.matches as match}<li><span>{match.sender}</span>{match.subject || '(No subject)'}</li>{/each}</ul>{/if}
        {/if}
      </div>
      {#if error}<p class="error-box" role="alert">{error}</p>{/if}
    </div>
    <footer class="sheet-foot">
      <button type="button" class="secondary" onclick={onclose}>Cancel</button>
      <button type="submit" class="primary" disabled={saving || invalid}>{saving ? 'Saving…' : 'Save'}</button>
    </footer>
  </form>
</dialog>

<style>
  .label-editor h2 { overflow-wrap: anywhere; }
  .label-editor h3 { font-size: calc(14px * var(--text-scale)); font-weight: 600; }
  .block { display: grid; gap: 10px; min-width: 0; margin: 0; padding: 0; border: 0; }
  .block legend { margin-bottom: 4px; padding: 0; font-size: calc(14px * var(--text-scale)); font-weight: 600; }
  .hint { color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); line-height: 1.5; }
  .hint code { padding: 0 4px; border-radius: 4px; background: var(--control); color: var(--ink-2); }
  .rule { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 8px; }
  .rule :global(.select-trigger) { min-width: 118px; }
  .rule input { height: 36px; padding: 0 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--sunk); color: var(--ink); font-size: calc(14px * var(--text-scale)); outline: none; transition: border-color 140ms ease; }
  .rule input:focus { border-color: var(--mint); }
  .rule input[aria-invalid='true'] { border-color: var(--danger); }
  .rule-error { margin-top: -4px; color: var(--danger); font-size: calc(12.5px * var(--text-scale)); }
  .block > .text-button { justify-self: start; }
  .label-editor textarea { width: 100%; padding: 9px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--sunk); color: var(--ink); font-size: calc(14px * var(--text-scale)); font-weight: 450; resize: vertical; outline: none; }
  .label-editor textarea:focus { border-color: var(--mint); }
  .test { display: grid; gap: 6px; padding-top: 14px; border-top: 1px solid var(--line-soft); }
  .matches { display: grid; gap: 2px; }
  .matches li { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink-2); font-size: calc(13px * var(--text-scale)); }
  .matches span { margin-right: 8px; color: var(--ink-3); }
</style>
