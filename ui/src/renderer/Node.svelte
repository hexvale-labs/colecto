<script lang="ts">
  // Draws one checked component node (docs/extensions/ui-model.md, "Component vocabulary").
  // Strings are always text: nothing here turns extension data into HTML, CSS, URLs or script.
  // Presets reproduce the app's own layouts, so extension panes look like the rest of Colecto.
  import { getContext } from 'svelte';
  import Icon, { type IconName } from '../Icon.svelte';
  import Select from '../Select.svelte';
  import mark from '../assets/logo-square.svg';
  import { hue, initial } from '../lib/initial';
  import { copyText } from '../lib/copy';
  import { touchSlider } from '../lib/slider';
  import type { Node, Row } from '../runtime/protocol';
  import Frame from './Frame.svelte';
  import MenuButton from './MenuButton.svelte';
  import SecretField from './SecretField.svelte';
  import Slot from './Slot.svelte';
  import ActionRef from './ActionRef.svelte';
  import List from './List.svelte';
  import { runtimes } from '../runtime/runtimes.svelte';
  import Self from './Node.svelte';
  import type { SurfaceContext } from './Surface.svelte';
  let { node }: { node: Node } = $props();
  const surface = getContext<SurfaceContext>('surface');
  const p = <T,>(key: string) => node[key] as T;
  const emit = (kind: string, value?: unknown) => { if (node.id) surface.event(node.id, kind, value); };
  let timer: ReturnType<typeof setTimeout> | undefined;
  // Values this field sent that the extension has not yet drawn back. A render that still
  // carries one of them is an echo of older typing and must not rewind the field.
  const sent = new Set<string>();
  function typed(value: string) {
    sent.add(value);
    if (p('report') !== 'input') return;
    clearTimeout(timer);
    timer = setTimeout(() => emit('input', { value }), p<number>('debounce_ms') ?? 0);
  }
  /** A tab strip only scrolls sideways, so a vertical wheel scrolls it sideways too (#131). */
  function sideways(strip: HTMLElement) {
    const wheel = (event: WheelEvent) => {
      if (event.ctrlKey || Math.abs(event.deltaX) >= Math.abs(event.deltaY) || strip.scrollWidth <= strip.clientWidth) return;
      event.preventDefault();
      strip.scrollLeft += event.deltaMode === 1 ? event.deltaY * 16 : event.deltaMode === 2 ? event.deltaY * strip.clientWidth : event.deltaY;
    };
    strip.addEventListener('wheel', wheel, { passive: false });
    return { destroy: () => strip.removeEventListener('wheel', wheel) };
  }
  /** Right-click, or the Menu key or Shift+F10, on a tab of a strip with `close_all`: close it
   *  or every tab of this strip, through the same events as the strip's buttons (#131). */
  function tabMenu(event: MouseEvent | KeyboardEvent, item: { id: string; closable?: boolean; disabled?: boolean }) {
    const key = event instanceof KeyboardEvent;
    if (key && !(event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey))) return;
    if (!p('close_all') || !surface.localMenu || item.disabled) return;
    event.preventDefault();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const [x, y] = key ? [rect.left + 12, rect.bottom] : [(event as MouseEvent).clientX, (event as MouseEvent).clientY];
    surface.localMenu(x, y, 'Tab actions', [
      ...(item.closable !== false ? [{ id: 'close', label: 'Close tab', icon: 'close' as const }] : []),
      { id: 'close_all', label: 'Close all tabs' },
    ], choice => choice === 'close' ? emit('close', { item: item.id }) : emit('close_all'));
  }
  /** Fit a textarea's height to its text, without a scrollbar. */
  function grow(field: HTMLTextAreaElement) {
    const fit = () => { field.style.height = 'auto'; field.style.height = `${field.scrollHeight + field.offsetHeight - field.clientHeight}px`; };
    const observer = new ResizeObserver(fit);
    observer.observe(field);
    field.addEventListener('input', fit);
    queueMicrotask(fit);
    return { destroy() { observer.disconnect(); field.removeEventListener('input', fit); } };
  }
  function controlled(field: HTMLInputElement | HTMLTextAreaElement, value: string) {
    field.value = value;
    return {
      update(next: string) {
        if (sent.has(next) && document.activeElement === field) { if (next === field.value) sent.clear(); return; }
        sent.clear();
        if (field.value !== next) field.value = next;
      },
    };
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
  const gaps = (gap: unknown) => (typeof gap === 'number' ? `${gap}px` : undefined);
  // Copy buttons show their own result for a moment; the extension only hears whether it worked.
  let copied = $state<boolean | null>(null);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  async function press() {
    const text = p<string>('copy');
    if (typeof text === 'string') {
      copied = await copyText(text);
      clearTimeout(copiedTimer); copiedTimer = setTimeout(() => copied = null, 1600);
      emit('copied', { ok: copied });
    } else if (!p('submit')) emit('press');
  }
  const classes: Record<string, string> = { primary: 'primary', secondary: 'secondary', tool: 'tool', text: 'text-button', icon: 'icon-button', chip: 'chip', chip_remove: 'chip-remove', plain_remove: 'chip-remove plain', compose: 'compose-button', compose_icon: 'compose-icon' };
  const stackClass: Record<string, string> = { pane: 'mail-pane', pane_header: 'mail-head', chip_bar: 'label-bar', chip_group: 'chip-group', note: 'sort-note', body_controls: 'body-controls', compose_footer: 'compose-footer', actions: 'bus-actions', attachments_block: 'compose-attachments', welcome: 'welcome' };
  const size = (format: unknown, value: number, unit?: string) => format === 'percent' ? `${Math.round(value * 100)}%` : `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(value)}${unit ? ` ${unit}` : ''}`;
  let rangeDraft = $state<number | null>(null);
  async function pick(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    input.value = '';
    if (!files.length) return;
    // A file that cannot be read is reported, never sent empty.
    const buffers: ArrayBuffer[] = [];
    const failed: boolean[] = [];
    for (const file of files) {
      const bytes = await file.arrayBuffer().catch(() => null);
      failed.push(!bytes); buffers.push(bytes ?? new ArrayBuffer(0));
    }
    surface.files(node.id!, files.map((file, index) => ({ name: file.name, type: file.type, size: file.size, ...(failed[index] ? { failed: true } : {}) })), buffers);
  }
</script>

{#snippet children()}{#each node.children ?? [] as child, index (child.id ?? `${child.type}:${index}`)}<Self node={child} />{/each}{/snippet}

{#if node.type === 'stack'}
  {@const preset = p<string>('preset')}
  {#if preset === 'footer'}
    <footer class="list-footer">{@render children()}</footer>
  {:else if preset === 'chip_group'}
    <span class="chip-group">{@render children()}</span>
  {:else if preset === 'actions'}
    <div class="bus-actions" role="toolbar" aria-label="Extension actions">{@render children()}</div>
  {:else if preset}
    <div class={stackClass[preset]}>{@render children()}</div>
  {:else}
    <div class="x-stack" data-direction={p('direction') ?? 'column'} data-align={p('align')} data-justify={p('justify')} data-scroll={p('scroll')} class:wrap={p('wrap')} style:gap={gaps(p('gap'))}>{@render children()}</div>
  {/if}
{:else if node.type === 'section'}
  {@const preset = p<string>('preset')}
  <section class={preset === 'mailbox' ? 'mailbox' : preset === 'reader' ? 'detail reader' : preset === 'composer' ? 'detail composer' : preset === 'summary' ? 'x-summary' : undefined} data-density={p('density')} aria-label={p('label')} aria-labelledby={p<string>('labelled_by') ? `${surface.prefix}-${p<string>('labelled_by')}` : undefined} aria-busy={p('busy') ?? undefined}>{@render children()}</section>
{:else if node.type === 'article'}
  <article class="letter" aria-label={p('label')}>{@render children()}</article>
{:else if node.type === 'group'}
  {@const preset = p<string>('preset')}
  <div role={p('role') ?? 'group'} aria-label={p('label')} class={preset === 'respond' ? 'toolbar-actions' : preset === 'actions' ? 'label-bar' : 'x-group'}>{@render children()}</div>
{:else if node.type === 'toolbar'}
  <header class="detail-toolbar">{@render children()}</header>
{:else if node.type === 'form'}
  {@const preset = p<string>('preset')}
  <form class={p('role') === 'search' ? 'mail-search' : preset === 'chip_input' ? 'label-new' : preset === 'compose' ? 'compose-form' : undefined} role={p('role')} aria-label={p('label')} onsubmit={submit}>{@render children()}</form>
{:else if node.type === 'fieldset'}
  {@const preset = p<string>('preset')}
  <fieldset class={preset === 'chips' ? 'message-labels' : preset === 'compose_fields' ? 'compose-fields' : 'settings-group'} disabled={p('disabled')}>
    {#if p('legend')}<legend class={p('legend_hidden') ? 'sr-only' : 'group-label'}>{p('legend')}</legend>{/if}
    {#if preset === 'chips'}<span class="labels-mark" aria-hidden="true"><Icon name="tag" size={14} /></span>{/if}
    {@render children()}
  </fieldset>
{:else if node.type === 'grid'}
  <div class="form-grid">{@render children()}</div>
{:else if node.type === 'nav'}
  {#if !(p('placement') === 'bottom' && runtimes.compact && !surface.lifted)}<nav class="folder-nav" aria-label={p('label')}>
    {#each p<{ id: string; label: string; icon?: IconName; current?: boolean; count?: number; disabled?: boolean }[]>('items') as item (item.id)}
      <button class:active={item.current} aria-current={item.current ? 'page' : undefined} disabled={item.disabled} onclick={() => emit('select', { item: item.id })}>{#if item.icon}<Icon name={item.icon} />{/if}<span>{item.label}</span>{#if item.count}<span class="count" aria-hidden="true">{item.count}</span>{/if}</button>
    {/each}
  </nav>{/if}
{:else if node.type === 'tabs'}
  {@const items = p<{ id: string; label: string; title?: string; active?: boolean; closable?: boolean; disabled?: boolean }[]>('items')}
  {#if items.length}<nav class="tab-strip" aria-label={p('label')} use:sideways>
    {#each items as item (item.id)}<div class="message-tab" class:active={item.active}>
      <button class="tab-title" aria-pressed={!!item.active} disabled={item.disabled} title={item.title ?? item.label} onclick={() => emit('select', { item: item.id })} oncontextmenu={(event) => tabMenu(event, item)} onkeydown={(event) => tabMenu(event, item)}>{item.label}</button>
      {#if item.closable !== false}<button class="tab-close" aria-label={`Close ${item.label}`} disabled={item.disabled} onclick={() => emit('close', { item: item.id })} oncontextmenu={(event) => tabMenu(event, item)}><Icon name="close" size={13} /></button>{/if}
    </div>{/each}
    {#if p('close_all') && items.length > 1}<button class="tab-close-all" aria-label="Close all message tabs" title="Close all tabs" onclick={() => emit('close_all')}><Icon name="close" size={13} /><span>Close all</span></button>{/if}
  </nav>{/if}
{:else if node.type === 'heading'}
  {@const style = p<string>('style')}
  <svelte:element this={`h${p<number>('level') ?? 2}`} id={fieldId} class={style === 'folder' ? 'folder-title' : style === 'day' ? 'day' : style === 'toolbar' ? undefined : 'x-heading'}>{p('text')}</svelte:element>
{:else if node.type === 'text'}
  {@const preset = p<string>('preset')}
  {@const spans = p<{ text: string; style?: string }[]>('spans')}
  {#if preset === 'crumb'}
    <span class="toolbar-crumb">{p('text')}</span>
  {:else if preset === 'list_status'}
    <span class="list-status" class:offline={p('tone') === 'danger'} aria-live="polite" title={p('title')}><span class="status-text">{spans?.[0]?.text ?? p('text')}</span>{#if spans?.[1]?.text}<span class="status-detail">{spans[1].text}</span>{/if}</span>
  {:else if preset === 'body'}
    <!-- Blank lines become paragraphs, so paragraph spacing can widen them; by default the
         gap is one line, as before. -->
    <div class="letter-body">{#each String(p('text') ?? '').split(/\n[ \t]*\n/) as paragraph, index (index)}<p>{paragraph}</p>{/each}</div>
  {:else if preset === 'note'}
    <p class="content-note" role={p('role')}>{p('text')}</p>
  {:else if preset === 'error'}
    <p class="error-box" role="alert">{p('text')}</p>
  {:else if preset === 'save_state'}
    <span class="save-state" class:pending={p('pending')} role="status">{p('text')}</span>
  {:else if preset === 'body_note'}
    <span class="body-note"><Icon name="image" />{p('text')}</span>
  {:else if preset === 'from'}
    <div class="input-row"><span class="row-label">{spans?.[0]?.text}</span><strong class="sender">{spans?.[1]?.text}</strong></div>
  {:else}
    <p id={fieldId} class="x-text" class:sr-only={p('hidden_visually')} data-tone={p('tone')} data-size={p('size')} data-wrap={p('wrap')} role={p('role')} aria-live={p('live')} title={p('title')}>
      {#if spans}{#each spans as span, index (index)}{#if span.style === 'strong'}<strong>{span.text}</strong>{:else if span.style === 'code'}<code>{span.text}</code>{:else if span.style === 'kbd'}<kbd>{span.text}</kbd>{:else if span.style === 'muted'}<span class="x-muted">{span.text}</span>{:else}{span.text}{/if}{/each}{:else}{p('text')}{/if}
    </p>
  {/if}
{:else if node.type === 'badge'}
  <span class="x-badge" data-tone={p('tone')}>{p('text')}</span>
{:else if node.type === 'banner'}
  {@const tone = p<string>('tone') ?? 'info'}
  <p class="reader-banner {tone}" role={tone === 'info' ? 'status' : 'alert'}><Icon name={tone === 'info' ? 'info' : 'alert'} size={15} /><span>{p('text')}</span>{#each p<Node[]>('actions') ?? [] as action, index (action.id ?? index)}<Self node={action} />{/each}</p>
{:else if node.type === 'avatar'}
  <span class="avatar" style:--hue={hue(p<string>('seed') ?? p<string>('name'))} aria-hidden="true">{initial(p<string>('name'))}</span>
{:else if node.type === 'icon'}
  <Icon name={p<IconName>('name')} size={p<number>('size') ?? 16} />
{:else if node.type === 'message_header'}
  <h2>{p<string>('subject') || '(No subject)'}</h2>
  <div class="sender-row">
    <span class="avatar" style:--hue={hue(p<string>('address') || p<string>('sender') || '')} aria-hidden="true">{initial(p<string>('sender') || p<string>('address') || '')}</span>
    <div class="sender-text"><strong>{p<string>('sender') || p<string>('address') || 'Unknown sender'}</strong>{#if p('address') && p('address') !== p('sender')}<span class="address">{p('address')}</span>{/if}</div>
    {#if p('date')}<time class="sent-at" title={p('date_title')}>{p('date')}</time>{/if}
  </div>
{:else if node.type === 'attachments'}
  <ul class="attachments" aria-label={p('label')}>
    {#each p<{ index: number; name: string; detail?: string; removable?: boolean }[]>('items') as file (file.index)}
      <li>{#if file.removable}
        <span class="attachment"><Icon name="attach" /><span class="attachment-name">{file.name}</span><button class="icon-button" aria-label={`Remove ${file.name}`} type="button" onclick={() => emit('remove', { index: file.index })}><Icon name="close" size={13} /></button></span>
      {:else}
        <button class="attachment" disabled={p('disabled')} aria-label={`Download ${file.name}`} onclick={() => emit('download', { index: file.index })}><Icon name="attach" /><span class="attachment-name">{file.name}</span><small>{file.detail}</small></button>
      {/if}</li>
    {/each}
  </ul>
{:else if node.type === 'button'}
  {@const variant = p<string>('variant') ?? 'secondary'}
  {@const iconOnly = p('icon_only') || variant === 'icon' || variant === 'chip_remove' || variant === 'plain_remove' || variant === 'compose_icon'}
  {@const icon = typeof p('copy') === 'string' && copied !== null ? (copied ? 'check' : 'copy') : p<IconName>('icon')}
  <button type={p('submit') ? 'submit' : 'button'} class="{classes[variant]}{p('icon') === 'back' && variant === 'icon' ? ' back' : ''}{p('icon') === 'refresh' && variant === 'icon' ? ' refresh' : ''}{p('push') ? (typeof p('copy') === 'string' ? ' copy-tool push' : ' push') : typeof p('copy') === 'string' ? ' copy-tool' : ''}" class:spinning={p('busy')} data-icon={p('icon')} disabled={p('disabled')} aria-pressed={p('pressed')} aria-label={iconOnly || p('title') ? p('label') : undefined} title={p('title')} data-command={node.id} onclick={press}>
    {#if icon && variant !== 'primary'}<Icon name={icon} size={variant === 'icon' && p('icon') === 'back' ? 18 : variant === 'icon' && p('icon') === 'refresh' ? 15 : variant === 'chip_remove' || variant === 'plain_remove' ? 12 : undefined} />{/if}
    {#if !iconOnly}{#if p('live')}<span aria-live="polite">{typeof p('copy') === 'string' ? (copied === null ? 'Copy' : copied ? 'Copied' : 'Copy failed') : p('label')}</span>{:else if variant === 'primary' || variant === 'secondary' || variant === 'text' || variant === 'chip'}{p('label')}{:else}<span>{p('label')}</span>{/if}{#if p('detail')}<small>{p('detail')}</small>{/if}{/if}
    {#if icon && variant === 'primary'}<Icon name={icon} size={14} />{/if}
  </button>
{:else if node.type === 'menu_button'}
  <MenuButton label={p<string>('label')} icon={p<IconName>('icon')} items={p('items')} disabled={p('disabled')} onselect={(item) => emit('select', { item })} />
{:else if node.type === 'action_ref'}
  <ActionRef action={p<string>('action')} labelled={p<boolean>('labelled')} />
{:else if node.type === 'input'}
  {#if p('kind') === 'search' && p('label_hidden')}
    <Icon name="search" />
    <label class="sr-only" for={fieldId}>{p('label')}</label>
    <!-- A one-line field that grows to show a long query instead of scrolling sideways (#103).
         Enter still searches, and line breaks never reach the value. -->
    <textarea id={fieldId} data-node={node.id} rows="1" use:controlled={p<string>('value') ?? ''} use:grow placeholder={p('placeholder')} maxlength={p('maxlength')} enterkeyhint={p('enterkeyhint')} autocapitalize={p('autocapitalize')} spellcheck="false" disabled={p('disabled')}
      onkeydown={(event) => { if (event.key === 'Enter' && !event.isComposing) { event.preventDefault(); event.currentTarget.form?.requestSubmit(); } }}
      oninput={(event) => { const field = event.currentTarget; if (/[\r\n]/.test(field.value)) field.value = field.value.replace(/[\r\n]+/g, ' '); typed(field.value); }}
      onchange={(event) => { if (p('report') !== 'input') emit('change', { value: event.currentTarget.value }); }}></textarea>
  {:else if p('preset') === 'chip'}
    <input id={fieldId} data-node={node.id} aria-label={p('label')} placeholder={p('placeholder')} maxlength={p('maxlength')} use:controlled={p<string>('value') ?? ''} disabled={p('disabled')} oninput={(event) => typed(event.currentTarget.value)} />
  {:else if p('preset') === 'row'}
    <div class="input-row"><label for={fieldId}>{p('label')}</label><input id={fieldId} data-node={node.id} type={p('kind') === 'email' ? 'text' : p('kind') ?? 'text'} use:controlled={p<string>('value') ?? ''} placeholder={p('placeholder')} maxlength={p('maxlength')} autocomplete={p('autocomplete')} inputmode={p('inputmode')} enterkeyhint={p('enterkeyhint')} spellcheck={p('spellcheck')} autocapitalize={p('autocapitalize')} disabled={p('disabled')}
      oninput={(event) => typed(event.currentTarget.value)} onchange={(event) => { if (p('report') !== 'input') emit('change', { value: event.currentTarget.value }); }} /></div>
  {:else}
    <label class="field x-field">
      <span class:sr-only={p('label_hidden')}>{p('label')}</span>
      <input id={fieldId} data-node={node.id} type={p('kind') ?? 'text'} use:controlled={p<string>('value') ?? ''} placeholder={p('placeholder')} maxlength={p('maxlength')} min={p('min')} max={p('max')} step={p('step')} autocomplete={p('autocomplete')} inputmode={p('inputmode')} enterkeyhint={p('enterkeyhint')} spellcheck={p('spellcheck')} autocapitalize={p('autocapitalize')} disabled={p('disabled')} aria-invalid={p('error') ? true : undefined}
        oninput={(event) => typed(event.currentTarget.value)} onchange={(event) => { if (p('report') !== 'input') emit('change', { value: event.currentTarget.value }); }} />
      {#if p('error')}<span class="x-error" role="alert">{p('error')}</span>{/if}
    </label>
  {/if}
{:else if node.type === 'textarea'}
  {#if p('preset') === 'body'}
    <label class="sr-only" for={fieldId}>{p('label')}</label><textarea id={fieldId} data-node={node.id} placeholder={p('placeholder')} maxlength={p('maxlength')} use:controlled={p<string>('value') ?? ''} disabled={p('disabled')} oninput={(event) => typed(event.currentTarget.value)} onchange={(event) => { if (p('report') !== 'input') emit('change', { value: event.currentTarget.value }); }}></textarea>
  {:else}
    <label class="field x-field">
      <span class:sr-only={p('label_hidden')}>{p('label')}</span>
      <textarea id={fieldId} data-node={node.id} use:controlled={p<string>('value') ?? ''} placeholder={p('placeholder')} maxlength={p('maxlength')} rows={p('rows')} disabled={p('disabled')}
        oninput={(event) => typed(event.currentTarget.value)} onchange={(event) => { if (p('report') !== 'input') emit('change', { value: event.currentTarget.value }); }}></textarea>
    </label>
  {/if}
{:else if node.type === 'toggle'}
  {@const style = p<string>('style')}
  {#if style === 'chip'}
    <label class="chip toggle"><input type="checkbox" data-node={node.id} aria-label={p('label_hidden') ? p('label') : undefined} checked={p('checked')} disabled={p('disabled')} onchange={(event) => emit('toggle', { checked: event.currentTarget.checked })} /><Icon name="check" size={12} />{p('label_hidden') ? (p<string>('label')).replace(/^Label /, '') : p('label')}</label>
  {:else if style === 'checkbox'}
    <label class="choice"><input type="checkbox" data-node={node.id} checked={p('checked')} disabled={p('disabled')} onchange={(event) => emit('toggle', { checked: event.currentTarget.checked })} /><span class:sr-only={p('label_hidden')}>{p('label')}</span></label>
  {:else if p('description')}
    <div class="setting setting-row">
      <div class="setting-text"><h3 id={fieldId ? `${fieldId}-label` : undefined}>{p('label')}</h3><p>{p('description')}</p></div>
      <label class="switch"><input type="checkbox" data-node={node.id} aria-labelledby={fieldId ? `${fieldId}-label` : undefined} checked={p('checked')} disabled={p('disabled')} onchange={(event) => emit('toggle', { checked: event.currentTarget.checked })} /></label>
    </div>
  {:else}
    <label class="switch"><input type="checkbox" role="switch" data-node={node.id} checked={p('checked')} disabled={p('disabled')} onchange={(event) => emit('toggle', { checked: event.currentTarget.checked })} /><span class:sr-only={p('label_hidden')}>{p('label')}</span></label>
  {/if}
{:else if node.type === 'select'}
  <Select label={p<string>('label')} value={p<string>('value') ?? ''} options={p<{ value: string; label: string; hint?: string }[]>('options')} disabled={p('disabled')} onchange={(value) => emit('change', { value })} />
{:else if node.type === 'segmented'}
  {@const options = p<{ value: string; label: string; hint?: string }[]>('options')}
  <fieldset class="segmented-field" disabled={p('disabled')}>
    <legend>{p('label')}</legend>
    <!-- Hints show as tooltips: after a moment on hover, at once on keyboard focus. -->
    <div class="segmented" class:fill={options.some(option => option.hint)} class:x-hinted={options.some(option => option.hint)}>{#each options as option (option.value)}<label><input type="radio" name={fieldId} value={option.value} aria-label={option.hint ? option.label : undefined} aria-describedby={option.hint ? `${fieldId}-${option.value}` : undefined} checked={p('value') === option.value} onchange={() => emit('change', { value: option.value })} />{option.label}{#if option.hint}<span class="tip" role="tooltip" id={`${fieldId}-${option.value}`}>{option.hint}</span>{/if}</label>{/each}</div>
  </fieldset>
{:else if node.type === 'range'}
  {@const min = p<number>('min')}{@const max = p<number>('max')}{@const value = rangeDraft ?? p<number>('value') ?? min}
  <div class="field x-range">
    <span class="range-head"><label for={fieldId}>{p('label')}</label><output for={fieldId}>{size(p('format'), value, p('unit'))}</output></span>
    <input id={fieldId} data-node={node.id} class="slider" type="range" use:touchSlider {min} {max} step={p('step') ?? 'any'} {value} disabled={p('disabled')} style:--fill={`${((value - min) / (max - min || 1)) * 100}%`} aria-valuetext={size(p('format'), value, p('unit'))}
      oninput={(event) => rangeDraft = Number(event.currentTarget.value)} onchange={(event) => { const next = Number(event.currentTarget.value); rangeDraft = null; emit('change', { value: next }); }} />
    {#if p('low') || p('high')}<span class="range-ends" aria-hidden="true"><span>{p('low')}</span><span>{p('high')}</span></span>{/if}
  </div>
{:else if node.type === 'file_picker'}
  <label class="tool attach-button" class:disabled={p('disabled')}>{#if p('icon')}<Icon name={p<IconName>('icon')} />{/if}<span>{p('label')}</span><input class="sr-only" type="file" multiple={p('multiple')} disabled={p('disabled')} onchange={pick} /></label>
{:else if node.type === 'secret_field'}
  <SecretField label={p<string>('label')} target={p<'ai.text' | 'ai.decision'>('target')} saved={p<boolean>('saved')} onsaved={(saved) => emit('saved', { saved })} />
{:else if node.type === 'list'}
  {#if p('mode') === 'virtual'}
    <List rows={p<Row[]>('rows')} selected={p<string>('selected')} arrived={p<string[]>('arrived') ?? []} more={p('more')} onend={() => emit('end')} onselect={(key) => emit('select', { key })} onmenu={p('context_menu') && node.id && surface.menu ? (key, x, y) => surface.menu!(node.id!, key, x, y) : undefined} empty={children} />
  {:else}
    <ul class="x-list" aria-label={p('label')}>{#each p<Row[]>('rows') as row (row.key)}<li data-kind={row.kind}><Self node={row.node} /></li>{/each}</ul>
  {/if}
{:else if node.type === 'empty_state'}
  {@const preset = p<string>('preset')}
  {@const hint = p<{ text: string; style?: string }[]>('hint')}
  {#if preset === 'welcome'}
    <section class="welcome">
      {#if p('image')}<img src={mark} alt="" width="48" height="48" />{/if}
      {#if p('title')}<h2>{p('title')}</h2>{/if}{#if p('text')}<p>{p('text')}</p>{/if}
      {#if hint}<p class="hint">{#each hint as span, index (index)}{#if span.style === 'kbd'}<kbd>{span.text}</kbd>{:else}{span.text}{/if}{/each}</p>{/if}
    </section>
  {:else}
    <div class={preset === 'error' ? 'list-state error' : preset === 'list' ? 'list-state' : 'detail-state'}>
      {#if p('title')}<h2>{p('title')}</h2>{/if}{#if p('text')}<p role={preset === 'error' || (preset !== 'list' && p('actions')) ? 'alert' : undefined}>{p('text')}</p>{/if}
      {#if hint}<p>{#each hint as span, index (index)}{span.text}{/each}</p>{/if}
      {#each p<Node[]>('actions') ?? [] as action, index (action.id ?? index)}<Self node={action} />{/each}
    </div>
  {/if}
{:else if node.type === 'loading'}
  <div class="detail-state" role="status">{p('text') ?? 'Loading…'}</div>
{:else if node.type === 'skeleton'}
  <div class="body-skeleton" aria-hidden="true">{#each Array(p<number>('lines') ?? 3) as _, index (index)}<span></span>{/each}</div>
{:else if node.type === 'frame'}
  <Frame title={p<string>('title')} doc={p<string>('doc')} height={p('height')} designed={p<boolean>('designed')} remote={p<boolean>('remote_images') && surface.may('ui.frame.remote')} />
{:else if node.type === 'slot'}
  <Slot name={p<string>('name')} context={p('context') ?? null} />
{/if}

<style>
  .x-stack { display: flex; flex-direction: column; min-width: 0; }
  .x-stack[data-direction='row'] { flex-direction: row; align-items: center; }
  /* A field in a row takes the free width. Aligned to the start, a field's error drops below it
     without moving its neighbours, so they take the field's height to stay level (#118). */
  :global(.x-stack[data-direction='row']:has(> .x-field)) { width: 100%; }
  :global(.x-stack[data-direction='row'] > .x-field) { flex: 1 1 10rem; min-width: 0; }
  :global(.x-stack[data-direction='row'][data-align='start'] > .select .select-trigger), :global(.x-stack[data-direction='row'][data-align='start'] > .icon-button) { height: 38px; }
  /* Rows of a short list (labels in Settings → AI) read like the account rows. */
  :global(.x-list > li:has(> .x-stack[data-direction='row'])) { display: flex; align-items: center; min-height: 56px; padding: 8px 0; }
  :global(.x-list > li:has(> .x-stack[data-direction='row']) + li) { border-top: 1px solid var(--line-soft); }
  :global(.x-list > li > .x-stack[data-direction='row']) { flex: 1; }
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
  .x-heading { color: var(--ink); letter-spacing: calc(-0.01em + var(--letter-spacing)); }
  .x-text { color: var(--ink); font-size: calc(14px * var(--text-scale)); line-height: calc(1.55 * var(--line-scale)); overflow-wrap: anywhere; }
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
  .x-range { max-width: 440px; }
  .x-range input.slider { height: 20px; padding: 0; border: 0; background: transparent; }
  .range-head { display: flex; justify-content: space-between; }
  .range-head output { color: var(--ink); font-variant-numeric: tabular-nums; }
  .range-ends { display: flex; justify-content: space-between; color: var(--ink-3); font-size: calc(12px * var(--text-scale)); font-weight: 450; }
  button small { color: var(--ink-3); font-weight: 450; }
  .x-hinted { flex-wrap: wrap; }
  .x-hinted label { position: relative; flex: 1 1 auto; justify-content: center; padding: 0 8px; }
  .tip { position: absolute; bottom: calc(100% + 8px); left: 50%; z-index: 1; width: max-content; max-width: 280px; padding: 6px 9px; border: 1px solid var(--line); border-radius: 7px; background: var(--float); color: var(--ink-2); font-size: calc(12.5px * var(--text-scale)); line-height: calc(1.4 * var(--line-scale)); white-space: normal; text-align: center; pointer-events: none; visibility: hidden; opacity: 0; transform: translate(-50%, 2px); transition: opacity 120ms ease, transform 120ms var(--ease-out), visibility 0s 120ms; }
  @media (hover: hover) and (pointer: fine) { .x-hinted label:hover .tip { visibility: visible; opacity: 1; transform: translate(-50%, 0); transition-delay: 350ms; } }
  .x-hinted label:has(input:focus-visible) .tip { visibility: visible; opacity: 1; transform: translate(-50%, 0); transition-delay: 0ms; }
  .x-hinted label:first-child .tip { left: 0; transform: translate(0, 2px); }
  .x-hinted label:first-child:has(input:focus-visible) .tip { transform: none; }
  @media (hover: hover) and (pointer: fine) { .x-hinted label:first-child:hover .tip { transform: none; } }
  .x-hinted label:last-child .tip { left: auto; right: 0; transform: translate(0, 2px); }
  .x-hinted label:last-child:has(input:focus-visible) .tip { transform: none; }
  @media (hover: hover) and (pointer: fine) { .x-hinted label:last-child:hover .tip { transform: none; } }
  @media (prefers-reduced-motion: reduce) { .tip { transition: none; } }
</style>
