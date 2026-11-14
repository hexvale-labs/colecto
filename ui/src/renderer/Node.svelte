<script lang="ts">
  // Draws one checked component node (docs/extensions/ui-model.md, "Component vocabulary").
  // Strings are always text: nothing here turns extension data into HTML, CSS, URLs or script.
  import { getContext } from 'svelte';
  import Icon, { type IconName } from '../Icon.svelte';
  import Select from '../Select.svelte';
  import type { Node, Row } from '../runtime/protocol';
  import Frame from './Frame.svelte';
  import Self from './Node.svelte';
  import type { SurfaceContext } from './Surface.svelte';
  let { node }: { node: Node } = $props();
  const surface = getContext<SurfaceContext>('surface');
  const p = <T,>(key: string) => node[key] as T;
  const emit = (kind: string, value?: unknown) => { if (node.id) surface.event(node.id, kind, value); };
  let timer: ReturnType<typeof setTimeout> | undefined;
  function typed(value: string) {
    if (p('report') !== 'input') return;
    clearTimeout(timer);
    timer = setTimeout(() => emit('input', { value }), p<number>('debounce_ms') ?? 0);
  }
  const fieldId = $derived(node.id ? `${surface.prefix}-${node.id}` : undefined);
  function submit(event: SubmitEvent) {
    event.preventDefault();
    const values: Record<string, unknown> = {};
    for (const field of (event.currentTarget as HTMLFormElement).querySelectorAll<HTMLInputElement>('[data-node]')) {
      values[field.dataset.node!] = field.type === 'checkbox' ? field.checked : field.value;
    }
    emit('submit', { values });
  }
  const gaps = (gap: unknown) => `${typeof gap === 'number' ? gap : 8}px`;
</script>

{#snippet children()}{#each node.children ?? [] as child, index (child.id ?? index)}<Self node={child} />{/each}{/snippet}
{#snippet button(b: Node)}<Self node={b} />{/snippet}

{#if node.type === 'stack'}
  <div class="x-stack" data-direction={p('direction') ?? 'column'} data-align={p('align')} data-justify={p('justify')} data-scroll={p('scroll')} class:wrap={p('wrap')} style:gap={gaps(p('gap'))}>{@render children()}</div>
{:else if node.type === 'section'}
  <section aria-label={p('label')} aria-labelledby={p<string>('labelled_by') ? `${surface.prefix}-${p<string>('labelled_by')}` : undefined} aria-busy={p('busy') ?? undefined}>{@render children()}</section>
{:else if node.type === 'group'}
  <div role={p('role') ?? 'group'} aria-label={p('label')} class="x-group">{@render children()}</div>
{:else if node.type === 'toolbar'}
  <header class="detail-toolbar">{@render children()}</header>
{:else if node.type === 'form'}
  <form role={p('role')} aria-label={p('label')} onsubmit={submit}>{@render children()}</form>
{:else if node.type === 'heading'}
  <svelte:element this={`h${p<number>('level') ?? 2}`} id={fieldId} class="x-heading" data-style={p('style')}>{p('text')}</svelte:element>
{:else if node.type === 'text'}
  <p id={fieldId} class="x-text" class:sr-only={p('hidden_visually')} data-tone={p('tone')} data-size={p('size')} data-wrap={p('wrap')} role={p('role')} aria-live={p('live')} title={p('title')}>
    {#if p('spans')}{#each p<{ text: string; style?: string }[]>('spans') as span, index (index)}{#if span.style === 'strong'}<strong>{span.text}</strong>{:else if span.style === 'code'}<code>{span.text}</code>{:else if span.style === 'kbd'}<kbd>{span.text}</kbd>{:else if span.style === 'muted'}<span class="x-muted">{span.text}</span>{:else}{span.text}{/if}{/each}{:else}{p('text')}{/if}
  </p>
{:else if node.type === 'badge'}
  <span class="x-badge" data-tone={p('tone')}>{p('text')}</span>
{:else if node.type === 'banner'}
  {@const tone = p<string>('tone') ?? 'info'}
  <div class="reader-banner {tone}" role={tone === 'info' ? 'status' : 'alert'}><Icon name={tone === 'info' ? 'info' : 'alert'} size={15} /><span>{p('text')}</span>{#each p<Node[]>('actions') ?? [] as action, index (action.id ?? index)}{@render button(action)}{/each}</div>
{:else if node.type === 'icon'}
  <Icon name={p<IconName>('name')} size={p<number>('size') ?? 16} />
{:else if node.type === 'button'}
  {@const variant = p<string>('variant') ?? 'secondary'}
  <button type={p('type') ?? 'button'} class={variant === 'icon' ? 'icon-button' : variant === 'text' ? 'text-button' : variant} class:spinning={p('busy')} disabled={p('disabled')} aria-pressed={p('pressed')} aria-label={p('icon_only') || variant === 'icon' ? p('label') : undefined} title={p('title') ?? (p('icon_only') || variant === 'icon' ? p('label') : undefined)} onclick={() => emit('press')}>
    {#if p('icon')}<Icon name={p<IconName>('icon')} />{/if}{#if !p('icon_only') && variant !== 'icon'}<span>{p('label')}</span>{#if p('detail')}<small>{p('detail')}</small>{/if}{/if}
  </button>
{:else if node.type === 'input'}
  <label class="field x-field">
    <span class:sr-only={p('label_hidden')}>{p('label')}</span>
    <input id={fieldId} data-node={node.id} type={p('kind') ?? 'text'} value={p<string>('value') ?? ''} placeholder={p('placeholder')} maxlength={p('maxlength')} autocomplete={p('autocomplete')} inputmode={p('inputmode')} enterkeyhint={p('enterkeyhint')} spellcheck={p('spellcheck')} autocapitalize={p('autocapitalize')} disabled={p('disabled')} aria-invalid={p('error') ? true : undefined}
      oninput={(event) => typed(event.currentTarget.value)} onchange={(event) => { if (p('report') !== 'input') emit('change', { value: event.currentTarget.value }); }} />
    {#if p('error')}<span class="x-error" role="alert">{p('error')}</span>{/if}
  </label>
{:else if node.type === 'textarea'}
  <label class="field x-field">
    <span class:sr-only={p('label_hidden')}>{p('label')}</span>
    <textarea id={fieldId} data-node={node.id} value={p<string>('value') ?? ''} placeholder={p('placeholder')} maxlength={p('maxlength')} rows={p('rows')} disabled={p('disabled')}
      oninput={(event) => typed(event.currentTarget.value)} onchange={(event) => { if (p('report') !== 'input') emit('change', { value: event.currentTarget.value }); }}></textarea>
  </label>
{:else if node.type === 'toggle'}
  {#if p('style') === 'checkbox'}
    <label class="choice"><input type="checkbox" data-node={node.id} checked={p('checked')} disabled={p('disabled')} onchange={(event) => emit('toggle', { checked: event.currentTarget.checked })} /><span class:sr-only={p('label_hidden')}>{p('label')}</span></label>
  {:else}
    <label class="switch"><input type="checkbox" role="switch" data-node={node.id} checked={p('checked')} disabled={p('disabled')} onchange={(event) => emit('toggle', { checked: event.currentTarget.checked })} /><span class:sr-only={p('label_hidden')}>{p('label')}</span></label>
  {/if}
{:else if node.type === 'select'}
  <Select label={p<string>('label')} value={p<string>('value') ?? ''} options={p<{ value: string; label: string; hint?: string }[]>('options')} disabled={p('disabled')} onchange={(value) => emit('change', { value })} />
{:else if node.type === 'list'}
  <ul class="x-list" aria-label={p('label')}>{#each p<Row[]>('rows') as row (row.key)}<li data-kind={row.kind}><Self node={row.node} /></li>{/each}</ul>
{:else if node.type === 'empty_state'}
  <div class="list-state">{#if p('title')}<h2>{p('title')}</h2>{/if}{#if p('text')}<p>{p('text')}</p>{/if}{#if p('hint')}<p class="hint">{#each p<{ text: string; style?: string }[]>('hint') as span, index (index)}{#if span.style === 'kbd'}<kbd>{span.text}</kbd>{:else}{span.text}{/if}{/each}</p>{/if}</div>
{:else if node.type === 'loading'}
  <div class="list-state" role="status">{p('text') ?? 'Loading…'}</div>
{:else if node.type === 'frame'}
  <Frame title={p<string>('title')} doc={p<string>('doc')} height={p('height')} designed={p<boolean>('designed')} />
{/if}

<style>
  .x-stack { display: flex; flex-direction: column; min-width: 0; }
  .x-stack[data-direction='row'] { flex-direction: row; align-items: center; }
  .x-stack.wrap { flex-wrap: wrap; }
  .x-stack[data-align='start'] { align-items: flex-start; }
  .x-stack[data-align='center'] { align-items: center; }
  .x-stack[data-align='end'] { align-items: flex-end; }
  .x-stack[data-align='stretch'] { align-items: stretch; }
  .x-stack[data-align='baseline'] { align-items: baseline; }
  .x-stack[data-justify='center'] { justify-content: center; }
  .x-stack[data-justify='end'] { justify-content: flex-end; }
  .x-stack[data-justify='between'] { justify-content: space-between; }
  .x-stack[data-scroll='y'] { overflow-y: auto; min-height: 0; }
  .x-stack[data-scroll='x'] { overflow-x: auto; }
  .x-group { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .x-heading { color: var(--ink); letter-spacing: -0.01em; }
  .x-text { color: var(--ink); font-size: calc(14px * var(--text-scale)); line-height: 1.55; overflow-wrap: anywhere; }
  .x-text[data-size='sm'] { font-size: calc(12.5px * var(--text-scale)); }
  .x-text[data-tone='muted'] { color: var(--ink-3); }
  .x-text[data-tone='danger'] { color: var(--danger); }
  .x-text[data-tone='ok'] { color: var(--status-online); }
  .x-text[data-wrap='pre'] { white-space: pre-wrap; }
  .x-muted { color: var(--ink-3); }
  .x-badge { display: inline-grid; place-items: center; min-width: 18px; height: 18px; padding: 0 6px; border-radius: 999px; background: var(--control); color: var(--ink-2); font-size: calc(11px * var(--text-scale)); font-weight: 600; font-variant-numeric: tabular-nums; }
  .x-badge[data-tone='danger'] { background: var(--danger-bg); color: var(--danger); }
  .x-field { margin-top: 0; }
  .x-field textarea { width: 100%; padding: 10px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--sunk); color: var(--ink); font: inherit; font-size: calc(14px * var(--text-scale)); outline: none; resize: vertical; }
  .x-field textarea:focus { border-color: var(--mint); }
  .x-error { color: var(--danger); font-weight: 450; }
  .x-list { display: grid; gap: 2px; margin: 0; padding: 0; list-style: none; }
  button small { color: var(--ink-3); font-weight: 450; }
</style>
