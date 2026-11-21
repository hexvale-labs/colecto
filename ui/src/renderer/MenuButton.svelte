<script lang="ts">
  // A button that opens a short menu, keyboard operable like the app's other menus.
  import { tick } from 'svelte';
  import Icon, { type IconName } from '../Icon.svelte';
  import { prefersReducedMotion } from '../lib/ui';
  let { label, icon, items, disabled = false, onselect }: { label: string; icon?: IconName; items: { id: string; label: string; disabled?: boolean }[]; disabled?: boolean; onselect: (item: string) => void } = $props();
  const uid = $props.id();
  const id = `menu-${uid}`;
  let open = $state(false);
  // Fixed to the window, so a scrolling or clipped parent (a row of chips) cannot hide it.
  let place = $state({ left: 0, top: 0 });
  let positioned = $state(false);
  let button = $state<HTMLButtonElement>();
  let menu = $state<HTMLElement>();
  // Leaves quicker than it arrives, back toward the button.
  function leave(_node: HTMLElement) { return { duration: prefersReducedMotion() ? 0 : 120, css: (t: number) => `opacity:${t};transform:scale(${0.97 + 0.03 * t}) translateY(${(1 - t) * -4}px)` }; }
  async function show(focus: 'first' | 'last' = 'first') {
    positioned = false;
    open = true; await tick();
    const anchor = button!.getBoundingClientRect();
    const width = menu?.offsetWidth ?? 0;
    const height = menu?.offsetHeight ?? 0;
    // Under the button, lined up with its right edge when there is room on the left, else its left.
    const left = anchor.right - width >= 8 ? anchor.right - width : Math.min(anchor.left, window.innerWidth - width - 8);
    const below = anchor.bottom + 4 + height <= window.innerHeight - 8;
    place = { left: Math.max(8, left), top: below ? anchor.bottom + 4 : Math.max(8, anchor.top - 4 - height) };
    positioned = true;
    await tick();
    const buttons = menu?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]');
    buttons?.[focus === 'first' ? 0 : buttons.length - 1]?.focus();
  }
  function hide(refocus = true) { open = false; positioned = false; if (refocus) button?.focus(); }
  function run(item: string) { hide(false); onselect(item); }
  function keydown(event: KeyboardEvent) {
    const buttons = [...(menu?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const last = buttons.length - 1;
    const moves: Record<string, number> = { ArrowDown: at >= last ? 0 : at + 1, ArrowUp: at <= 0 ? last : at - 1, Home: 0, End: last };
    if (event.key in moves) { event.preventDefault(); buttons[moves[event.key]]?.focus(); }
    else if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); hide(); }
    else if (event.key === 'Tab') hide(false);
  }
</script>

<svelte:window onresize={() => { if (open) hide(false); }} />
<div class="x-menu">
  <button bind:this={button} class="tool x-menu-tool" aria-label={label} aria-haspopup="menu" aria-expanded={open} aria-controls={open ? id : undefined} {disabled}
    onclick={() => open ? hide() : void show()}
    onkeydown={(event) => { if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); void show(event.key === 'ArrowDown' ? 'first' : 'last'); } }}>{#if icon}<Icon name={icon} />{/if}<span>{label}</span></button>
  {#if open}
    <div class="select-scrim" aria-hidden="true" onpointerdown={() => hide(false)}></div>
    <div bind:this={menu} {id} class="select-menu below x-menu-list" role="menu" tabindex="-1" aria-label={label} style:left={`${place.left}px`} style:top={`${place.top}px`} style:visibility={positioned ? undefined : 'hidden'} out:leave onkeydown={keydown}>
      {#each items as item (item.id)}<button role="menuitem" disabled={item.disabled} onclick={() => run(item.id)}>{item.label}</button>{/each}
    </div>
  {/if}
</div>

<style>
  .x-menu { position: relative; display: flex; }
  /* Narrow readers turn tools into icons; without one, the word stays. */
  .x-menu-tool:not(:has(svg)) { width: auto !important; padding: 0 11px !important; }
  .x-menu-list { position: fixed; right: auto; min-width: 160px; transform-origin: top; }
  .x-menu-list button { display: flex; align-items: center; width: 100%; padding: 7px 10px; border-radius: 8px; font-size: calc(13.5px * var(--text-scale)); text-align: left; transition: background-color 90ms ease, transform 120ms var(--ease-out); }
  .x-menu-list button:focus-visible { background: var(--raise); outline: none; }
  @media (hover: hover) and (pointer: fine) { .x-menu-list button:hover { background: var(--raise); } }
  .x-menu-list button:active { transform: scale(0.985); }
</style>
