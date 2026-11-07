<script lang="ts">
  import { tick } from 'svelte';
  export interface Command { id: string; name: string; run: () => void; disabled?: boolean }
  let { commands }: { commands: Command[] } = $props();
  let dialog: HTMLDialogElement;
  let input: HTMLInputElement;
  let query = $state('');
  let active = $state(0);
  let filtered = $derived(commands.filter(command => command.name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())));
  let previous: HTMLElement | null = null;
  export function open() {
    if (dialog.open) { dialog.close(); return; }
    previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    query = ''; active = 0; dialog.showModal(); input.focus();
  }
  function run(command?: Command) {
    if (!command || command.disabled) return;
    previous = null; dialog.close(); command.run();
  }
  async function keydown(event: KeyboardEvent) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      if (!filtered.length) return;
      active = (active + (event.key === 'ArrowDown' ? 1 : -1) + filtered.length) % filtered.length;
      await tick(); document.getElementById(`command-${active}`)?.scrollIntoView({ block: 'nearest' });
    } else if (event.key === 'Enter') { event.preventDefault(); run(filtered[active]); }
  }
</script>
<dialog bind:this={dialog} class="command-runner" aria-label="Commands" onclose={() => previous?.isConnected && previous.focus()}>
  <header><h2>Commands</h2><button class="text-button" onclick={() => dialog.close()}>Close</button></header>
  <input bind:this={input} aria-label="Search commands" role="combobox" aria-expanded="true" aria-controls="command-results" aria-activedescendant={filtered.length ? `command-${active}` : undefined} autocomplete="off" placeholder="What would you like to do?" bind:value={query} oninput={() => active = 0} onkeydown={keydown} />
  <div id="command-results" role="listbox" aria-label="Available commands">
    {#each filtered as command, index (command.id)}
      <button id={`command-${index}`} role="option" aria-selected={active === index} disabled={command.disabled} onclick={() => run(command)}>{command.name}</button>
    {:else}<p role="status">No matching commands</p>{/each}
  </div>
  <footer>↑ ↓ to navigate · Enter to run · Esc to close</footer>
</dialog>
