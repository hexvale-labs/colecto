<script lang="ts">
  import { tick } from 'svelte';
  import type { ReaderSlot } from '../../lib/reader-slots';
  import ReplyDialog from './ReplyDialog.svelte';
  import { ai, loadAi, summarize } from './state.svelte';
  let { reader, account }: { reader: ReaderSlot; account: string } = $props();
  void loadAi();
  const id = `ai-menu-${Math.random().toString(36).slice(2)}`;
  let open = $state(false);
  let replying = $state(false);
  let button = $state<HTMLButtonElement>();
  let menu = $state<HTMLElement>();
  const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  // Leaves quicker than it arrives, back toward the button, like the other menus.
  function leave(_node: HTMLElement) { return { duration: reduced ? 0 : 120, css: (t: number) => `opacity:${t};transform:scale(${0.97 + 0.03 * t}) translateY(${(1 - t) * -4}px)` }; }
  async function show(focus: 'first' | 'last' = 'first') {
    open = true; await tick();
    const buttons = menu?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]');
    buttons?.[focus === 'first' ? 0 : buttons.length - 1]?.focus();
  }
  function hide(refocus = true) { open = false; if (refocus) button?.focus(); }
  function run(item: 'summarize' | 'reply') {
    hide(item !== 'reply');
    if (item === 'summarize') void summarize(account, reader.id);
    else replying = true;
  }
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

{#if ai.enabled}
  <div class="ai-actions">
    <button bind:this={button} class="tool ai-tool" aria-haspopup="menu" aria-expanded={open} aria-controls={open ? id : undefined}
      onclick={() => open ? hide() : void show()}
      onkeydown={(event) => { if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); void show(event.key === 'ArrowDown' ? 'first' : 'last'); } }}>AI</button>
    {#if open}
      <div class="select-scrim" aria-hidden="true" onpointerdown={() => hide(false)}></div>
      <div bind:this={menu} {id} class="select-menu below ai-menu" role="menu" tabindex="-1" aria-label="AI" out:leave onkeydown={keydown}>
        <button role="menuitem" onclick={() => run('summarize')}>Summarize</button>
        {#if reader.reply}<button role="menuitem" onclick={() => run('reply')}>Reply</button>{/if}
      </div>
    {/if}
  </div>
  {#if reader.reply}<ReplyDialog bind:open={replying} {reader} {account} onclose={() => button?.focus()} />{/if}
{/if}

<style>
  .ai-actions { position: relative; display: flex; }
  /* Narrow readers turn tools into icons; this one has no icon, so its word stays. */
  .ai-tool { width: auto !important; padding: 0 11px !important; }
  .ai-menu { left: auto; right: 0; min-width: 160px; transform-origin: top right; }
  .ai-menu button { display: flex; align-items: center; width: 100%; padding: 7px 10px; border-radius: 8px; font-size: calc(13.5px * var(--text-scale)); text-align: left; transition: background-color 90ms ease, transform 120ms var(--ease-out); }
  .ai-menu button:focus-visible { background: var(--raise); outline: none; }
  @media (hover: hover) and (pointer: fine) { .ai-menu button:hover { background: var(--raise); } }
  .ai-menu button:active { transform: scale(0.985); }
</style>
