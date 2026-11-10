<script lang="ts" module>
  export interface Option<V> { value: V; label: string; hint?: string }
</script>
<script lang="ts" generics="T extends string">
  import { tick, type Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  let { label, value, options, onchange, disabled = false, placement = 'below', trigger }: {
    label: string; value: T; options: Option<T>[]; onchange: (value: T) => void;
    disabled?: boolean; placement?: 'below' | 'above'; trigger?: Snippet<[Option<T> | undefined]>;
  } = $props();
  const id = `listbox-${Math.random().toString(36).slice(2)}`;
  let open = $state(false);
  let active = $state(0);
  let button: HTMLButtonElement;
  let list = $state<HTMLElement>();
  let current = $derived(options.find(option => option.value === value));
  async function show() {
    if (disabled || !options.length) return;
    active = Math.max(0, options.findIndex(option => option.value === value));
    open = true; await tick(); list?.focus();
  }
  function hide(refocus = true) { open = false; if (refocus) button.focus(); }
  function choose(index: number) { const option = options[index]; hide(); if (option && option.value !== value) onchange(option.value); }
  function keydown(event: KeyboardEvent) {
    const last = options.length - 1;
    const moves: Record<string, number> = { ArrowDown: active + 1, ArrowUp: active - 1, Home: 0, End: last };
    if (event.key in moves) { event.preventDefault(); active = Math.min(last, Math.max(0, moves[event.key])); document.getElementById(`${id}-${active}`)?.scrollIntoView({ block: 'nearest' }); }
    else if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); choose(active); }
    else if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); hide(); }
    else if (event.key === 'Tab') hide(false);
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
    <ul bind:this={list} {id} class="select-menu {placement}" role="listbox" tabindex="-1" aria-label={label} aria-activedescendant={`${id}-${active}`} onkeydown={keydown}>
      {#each options as option, index (option.value)}
        <!-- svelte-ignore a11y_click_events_have_key_events (the listbox handles keys) -->
        <li id={`${id}-${index}`} role="option" aria-label={option.label} aria-selected={option.value === value} class:active={index === active} onpointermove={() => active = index} onclick={() => choose(index)}>
          <span class="option-text"><span>{option.label}</span>{#if option.hint}<small>{option.hint}</small>{/if}</span>
          {#if option.value === value}<Icon name="check" size={14} />{/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>
