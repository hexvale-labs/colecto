<script lang="ts">
  // A button that opens a short menu, keyboard operable like the app's other menus.
  import { tick } from 'svelte';
  import Icon, { type IconName } from '../Icon.svelte';
  let { label, icon, items, disabled = false, onselect }: { label: string; icon?: IconName; items: { id: string; label: string; disabled?: boolean }[]; disabled?: boolean; onselect: (item: string) => void } = $props();
  const id = `menu-${Math.random().toString(36).slice(2)}`;
  let open = $state(false);
  let button = $state<HTMLButtonElement>();
  let menu = $state<HTMLElement>();
  const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  // Leaves quicker than it arrives, back toward the button.
  function leave(_node: HTMLElement) { return { duration: reduced ? 0 : 120, css: (t: number) => `opacity:${t};transform:scale(${0.97 + 0.03 * t}) translateY(${(1 - t) * -4}px)` }; }
  async function show(focus: 'first' | 'last' = 'first') {
    open = true; await tick();
    const buttons = menu?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]');
    buttons?.[focus === 'first' ? 0 : buttons.length - 1]?.focus();
  }
  function hide(refocus = true) { open = false; if (refocus) button?.focus(); }
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

<div class="x-menu">
  <button bind:this={button} class="tool x-menu-tool" aria-label={label} aria-haspopup="menu" aria-expanded={open} aria-controls={open ? id : undefined} {disabled}
    onclick={() => open ? hide() : void show()}
    onkeydown={(event) => { if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); void show(event.key === 'ArrowDown' ? 'first' : 'last'); } }}>{#if icon}<Icon name={icon} />{/if}<span>{label}</span></button>
  {#if open}
    <div class="select-scrim" aria-hidden="true" onpointerdown={() => hide(false)}></div>
    <div bind:this={menu} {id} class="select-menu below x-menu-list" role="menu" tabindex="-1" aria-label={label} out:leave onkeydown={keydown}>
      {#each items as item (item.id)}<button role="menuitem" disabled={item.disabled} onclick={() => run(item.id)}>{item.label}</button>{/each}
    </div>
  {/if}
</div>

<style>
  .x-menu { position: relative; display: flex; }
  /* Narrow readers turn tools into icons; without one, the word stays. */
  .x-menu-tool:not(:has(svg)) { width: auto !important; padding: 0 11px !important; }
  .x-menu-list { left: auto; right: 0; min-width: 160px; transform-origin: top right; }
  .x-menu-list button { display: flex; align-items: center; width: 100%; padding: 7px 10px; border-radius: 8px; font-size: calc(13.5px * var(--text-scale)); text-align: left; transition: background-color 90ms ease, transform 120ms var(--ease-out); }
  .x-menu-list button:focus-visible { background: var(--raise); outline: none; }
  @media (hover: hover) and (pointer: fine) { .x-menu-list button:hover { background: var(--raise); } }
  .x-menu-list button:active { transform: scale(0.985); }
</style>
