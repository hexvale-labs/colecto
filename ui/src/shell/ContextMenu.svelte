<script lang="ts">
  // A row's context menu, drawn by the shell from the items its extension sent (API 2 `menu`).
  // It opens at the pointer, or under the row from the keyboard, and stays inside the window.
  import { tick } from 'svelte';
  import Icon from '../Icon.svelte';
  import type { ExtensionRuntime } from '../runtime/runtime.svelte';
  let { runtime }: { runtime: ExtensionRuntime } = $props();
  let menu = $state<HTMLElement>();
  let size = $state({ width: 0, height: 0 });
  const MARGIN = 8;
  const left = $derived(Math.max(MARGIN, Math.min(runtime.menu!.x, window.innerWidth - size.width - MARGIN)));
  const top = $derived(Math.max(MARGIN, runtime.menu!.y + size.height + MARGIN > window.innerHeight ? runtime.menu!.y - size.height : runtime.menu!.y));
  const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const items = () => [...(menu?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? [])];
  let opener: Element | null = null;
  $effect(() => {
    opener = document.activeElement;
    void tick().then(() => items()[0]?.focus());
    return () => { if (opener instanceof HTMLElement && opener.isConnected) opener.focus(); };
  });
  function keydown(event: KeyboardEvent) {
    const buttons = items();
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const last = buttons.length - 1;
    const moves: Record<string, number> = { ArrowDown: at >= last ? 0 : at + 1, ArrowUp: at <= 0 ? last : at - 1, Home: 0, End: last };
    if (event.key in moves) { event.preventDefault(); buttons[moves[event.key]]?.focus(); }
    else if (event.key === 'Escape' || event.key === 'Tab') { event.preventDefault(); event.stopPropagation(); runtime.menu = null; }
  }
</script>

<svelte:window onresize={() => runtime.menu = null} onblur={() => runtime.menu = null} />
<div class="select-scrim" aria-hidden="true" onpointerdown={() => runtime.menu = null} oncontextmenu={(event) => { event.preventDefault(); runtime.menu = null; }}></div>
<div bind:this={menu} bind:clientWidth={size.width} bind:clientHeight={size.height} class="select-menu context-menu" class:still={reduced} role="menu" tabindex="-1" aria-label="Message actions" style:left={`${left}px`} style:top={`${top}px`} onkeydown={keydown}>
  {#each runtime.menu!.items ?? [] as item (item.id)}
    {#if item.separator}<div class="context-separator" role="separator"></div>{/if}
    <button role={item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'} aria-checked={item.checked} class:destructive={item.danger} disabled={item.disabled} onclick={() => runtime.chooseMenu(item.id)}>
      <span class="context-icon" aria-hidden="true">{#if item.checked}<Icon name="check" size={14} />{:else if item.icon}<Icon name={item.icon} size={14} />{/if}</span>{item.label}
    </button>
  {/each}
</div>

<style>
  .context-menu { position: fixed; z-index: var(--layer-menus); min-width: 200px; max-width: min(320px, calc(100vw - 16px)); max-height: calc(100vh - 16px); padding: 4px; transform-origin: top left; }
  .context-menu.still { animation: none; }
  .context-menu button { display: flex; align-items: center; gap: 8px; width: 100%; padding: 7px 10px; border-radius: 8px; font-size: calc(13.5px * var(--text-scale)); text-align: left; transition: background-color 90ms ease; }
  .context-menu button:focus-visible { background: var(--raise); outline: none; }
  @media (hover: hover) and (pointer: fine) { .context-menu button:hover:not(:disabled) { background: var(--raise); } }
  .context-menu button:disabled { opacity: 0.45; }
  .context-menu button.destructive { color: var(--danger); }
  .context-icon { display: inline-grid; place-items: center; width: 16px; flex-shrink: 0; color: var(--ink-3); }
  .destructive .context-icon { color: inherit; }
  .context-separator { height: 1px; margin: 4px 6px; background: var(--line-soft); }
</style>
