<script lang="ts" module>
  export interface Option<V> { value: V; label: string; hint?: string; group?: string; family?: string }
</script>
<script lang="ts" generics="T extends string">
  import { tick, type Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import { claimBack } from './lib/back';
  let { label, value, options, onchange, disabled = false, placement = 'below', trigger, onopen }: {
    label: string; value: T; options: Option<T>[]; onchange: (value: T) => void;
    onopen?: () => void; disabled?: boolean; placement?: 'below' | 'above'; trigger?: Snippet<[Option<T> | undefined]>;
  } = $props();
  const id = `listbox-${Math.random().toString(36).slice(2)}`;
  let open = $state(false);
  let active = $state(0);
  let button: HTMLButtonElement;
  let list = $state<HTMLElement>();
  let current = $derived(options.find(option => option.value === value));
  const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  // Closing is quicker than opening and shrinks back toward the trigger.
  function leave(node: HTMLElement) {
    const from = placement === 'above' ? 4 : -4;
    return { duration: reduced ? 0 : 120, easing: (t: number) => t * (2 - t), css: (t: number) => `opacity:${t};transform:scale(${0.97 + 0.03 * t}) translateY(${(1 - t) * from}px)` };
  }
  async function show() {
    if (disabled || !options.length) return;
    onopen?.();
    active = Math.max(0, options.findIndex(option => option.value === value));
    open = true; await tick(); list?.focus();
    document.getElementById(`${id}-${active}`)?.scrollIntoView({ block: 'nearest' });
  }
  // Typing jumps to the next option starting with those letters, as native menus do.
  let typed = '';
  let typedAt = 0;
  function typeahead(key: string) {
    typed = (Date.now() - typedAt > 700 ? '' : typed) + key.toLocaleLowerCase(); typedAt = Date.now();
    const order = [...options.keys()].map(offset => (active + (typed.length === 1 ? 1 : 0) + offset) % options.length);
    const found = order.find(index => options[index].label.toLocaleLowerCase().startsWith(typed));
    if (found !== undefined) { active = found; document.getElementById(`${id}-${active}`)?.scrollIntoView({ block: 'nearest' }); }
  }
  function hide(refocus = true) { open = false; if (refocus) button.focus(); }
  // Back closes an open list, like Escape.
  $effect(() => { if (open) return claimBack({ close: () => hide(false), isOpen: () => open }); });
  function choose(index: number) { const option = options[index]; hide(); if (option && option.value !== value) onchange(option.value); }
  function keydown(event: KeyboardEvent) {
    const last = options.length - 1;
    const moves: Record<string, number> = { ArrowDown: active + 1, ArrowUp: active - 1, Home: 0, End: last };
    if (event.key in moves) { event.preventDefault(); active = Math.min(last, Math.max(0, moves[event.key])); document.getElementById(`${id}-${active}`)?.scrollIntoView({ block: 'nearest' }); }
    else if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); choose(active); }
    else if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); hide(); }
    else if (event.key === 'Tab') hide(false);
    else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) { event.preventDefault(); typeahead(event.key); }
  }
</script>

<div class="select" class:open>
  <button bind:this={button} type="button" class="select-trigger" aria-label={label} aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? id : undefined} {disabled}
    onclick={() => open ? hide() : void show()} onkeydown={(event) => { if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); void show(); } }}>
    {#if trigger}{@render trigger(current)}{:else}<span class="select-value">{current?.label ?? 'Choose…'}</span>{/if}
    <Icon name="chevron" size={14} />
  </button>
  {#if open}
    <div class="select-scrim" aria-hidden="true" onpointerdown={() => hide(false)}></div>
    <ul bind:this={list} {id} class="select-menu {placement}" out:leave role="listbox" tabindex="-1" aria-label={label} aria-activedescendant={`${id}-${active}`} onkeydown={keydown}>
      {#each options as option, index (option.value)}
        {#if option.group && (index === 0 || options[index - 1].group !== option.group)}<li role="presentation" class="select-group">{option.group}</li>{/if}
        <!-- svelte-ignore a11y_click_events_have_key_events (the listbox handles keys) -->
        <li id={`${id}-${index}`} role="option" aria-label={option.label} aria-selected={option.value === value} class:active={index === active} onpointermove={() => active = index} onclick={() => choose(index)}>
          <span class="option-text" style:font-family={option.family}><span>{option.label}</span>{#if option.hint}<small>{option.hint}</small>{/if}</span>
          {#if option.value === value}<Icon name="check" size={14} />{/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>
