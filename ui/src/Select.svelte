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
  // The side and height actually used: flips to the roomier side and never grows past the
  // nearest clipping ancestor. If neither side fits, the menu is clipped rather than scrolling
  // that ancestor, which would shift the whole app.
  let side = $state<'below' | 'above'>('below');
  let room = $state<number | undefined>();
  const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  // Closing is quicker than opening and shrinks back toward the trigger.
  function leave(node: HTMLElement) {
    const from = side === 'above' ? 4 : -4;
    return { duration: reduced ? 0 : 120, easing: (t: number) => t * (2 - t), css: (t: number) => `opacity:${t};transform:scale(${0.97 + 0.03 * t}) translateY(${(1 - t) * from}px)` };
  }
  async function show() {
    if (disabled || !options.length) return;
    onopen?.();
    active = Math.max(0, options.findIndex(option => option.value === value));
    place();
    open = true; await tick(); list?.focus({ preventScroll: true });
    reveal();
  }
  function place() {
    const trigger = button.getBoundingClientRect();
    let top = 0, bottom = window.innerHeight;
    for (let node = button.parentElement; node; node = node.parentElement) {
      if (getComputedStyle(node).overflowY === 'visible') continue;
      const rect = node.getBoundingClientRect();
      top = Math.max(top, rect.top); bottom = Math.min(bottom, rect.bottom);
    }
    const below = bottom - trigger.bottom - 12, above = trigger.top - top - 12;
    const wanted = Math.min(320, window.innerHeight * 0.6, options.length * 40 + 12);
    const fits = (space: number) => space >= wanted;
    side = placement === 'above' ? (fits(above) || above >= below ? 'above' : 'below') : (fits(below) || below >= above ? 'below' : 'above');
    room = Math.max(120, side === 'above' ? above : below);
  }
  // Scrolls only the list itself; scrollIntoView would also scroll clipped ancestors.
  function reveal() {
    const item = document.getElementById(`${id}-${active}`);
    if (!list || !item) return;
    if (item.offsetTop < list.scrollTop) list.scrollTop = item.offsetTop;
    else if (item.offsetTop + item.offsetHeight > list.scrollTop + list.clientHeight) list.scrollTop = item.offsetTop + item.offsetHeight - list.clientHeight;
  }
  // Typing jumps to the next option starting with those letters, as native menus do.
  let typed = '';
  let typedAt = 0;
  function typeahead(key: string) {
    typed = (Date.now() - typedAt > 700 ? '' : typed) + key.toLocaleLowerCase(); typedAt = Date.now();
    const order = [...options.keys()].map(offset => (active + (typed.length === 1 ? 1 : 0) + offset) % options.length);
    const found = order.find(index => options[index].label.toLocaleLowerCase().startsWith(typed));
    if (found !== undefined) { active = found; reveal(); }
  }
  function hide(refocus = true) { open = false; if (refocus) button.focus(); }
  // Back closes an open list, like Escape.
  $effect(() => { if (open) return claimBack({ close: () => hide(false), isOpen: () => open }); });
  function choose(index: number) { const option = options[index]; hide(); if (option && option.value !== value) onchange(option.value); }
  function keydown(event: KeyboardEvent) {
    const last = options.length - 1;
    const moves: Record<string, number> = { ArrowDown: active + 1, ArrowUp: active - 1, Home: 0, End: last };
    if (event.key in moves) { event.preventDefault(); active = Math.min(last, Math.max(0, moves[event.key])); reveal(); }
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
    <ul bind:this={list} {id} class="select-menu {side}" style:--room={room === undefined ? undefined : `${room}px`} out:leave role="listbox" tabindex="-1" aria-label={label} aria-activedescendant={`${id}-${active}`} onkeydown={keydown}>
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
