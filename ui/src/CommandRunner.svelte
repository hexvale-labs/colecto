<script lang="ts" module>
  export interface Command { id: string; name: string; run: () => void; disabled?: boolean }
  export interface MessageHit { id: string; name: string; subject: string; preview: string }
</script>
<script lang="ts">
  import { tick } from 'svelte';
  import Icon from './Icon.svelte';
  let { commands, messages = [], scope = 'mail', onopen, onsearch }: {
    commands: Command[]; messages?: MessageHit[]; scope?: string;
    onopen?: (id: string) => void; onsearch?: (query: string) => void;
  } = $props();
  type Entry = Command & { group: 'Commands' | 'Messages'; hint?: string };
  let dialog: HTMLDialogElement;
  let input: HTMLInputElement;
  let query = $state('');
  let active = $state(0);
  // Every word must appear, in any order, so "go sent" finds "Go to Sent".
  const matches = (text: string, words: string[]) => words.every(word => text.toLocaleLowerCase().includes(word));
  let entries = $derived.by((): Entry[] => {
    const text = query.trim();
    const words = text.toLocaleLowerCase().split(/\s+/).filter(Boolean);
    const found: Entry[] = commands.filter(command => matches(command.name, words)).map(command => ({ ...command, group: 'Commands' }));
    if (!words.length) return found;
    const mail = messages.filter(message => matches(`${message.name} ${message.subject} ${message.preview}`, words)).slice(0, 6);
    found.push(...mail.map(message => ({ id: `message-${message.id}`, group: 'Messages' as const, name: message.subject || '(No subject)', hint: message.name, run: () => onopen?.(message.id) })));
    if (onsearch) found.push({ id: 'search-all', group: 'Messages', name: `Search ${scope} for “${text}”`, run: () => onsearch(text) });
    return found;
  });
  let previous: HTMLElement | null = null;
  export function open() {
    if (dialog.open) { dialog.close(); return; }
    previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    query = ''; active = 0; dialog.showModal(); input.focus();
  }
  function run(entry?: Entry) {
    if (!entry || entry.disabled) return;
    previous = null; dialog.close(); entry.run();
  }
  async function keydown(event: KeyboardEvent) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      if (!entries.length) return;
      active = (active + (event.key === 'ArrowDown' ? 1 : -1) + entries.length) % entries.length;
      await tick(); document.getElementById(`command-${active}`)?.scrollIntoView({ block: 'nearest' });
    } else if (event.key === 'Enter') { event.preventDefault(); run(entries[active]); }
  }
</script>
<dialog bind:this={dialog} class="command-runner" aria-label="Commands" onclose={() => previous?.isConnected && previous.focus()}>
  <h2 class="sr-only">Commands</h2>
  <div class="command-input"><Icon name="search" size={18} /><input bind:this={input} aria-label="Search commands and mail" role="combobox" aria-expanded="true" aria-controls="command-results" aria-activedescendant={entries.length ? `command-${active}` : undefined} autocomplete="off" spellcheck="false" placeholder="Search mail or type a command" bind:value={query} oninput={() => active = 0} onkeydown={keydown} /><kbd>Esc</kbd></div>
  <div id="command-results" role="listbox" aria-label="Results">
    {#each entries as entry, index (entry.id)}
      {#if index === 0 || entries[index - 1].group !== entry.group}<p class="command-group" role="presentation">{entry.group}</p>{/if}
      <button id={`command-${index}`} role="option" aria-selected={active === index} disabled={entry.disabled} onpointermove={() => active = index} onclick={() => run(entry)}>
        {#if entry.group === 'Messages'}<Icon name={entry.id === 'search-all' ? 'search' : 'mail'} size={15} />{/if}
        <span class="command-name">{entry.name}</span>{#if entry.hint}<small>{entry.hint}</small>{/if}
      </button>
    {:else}<p class="command-empty" role="status">Nothing matches. Try fewer words.</p>{/each}
  </div>
  <footer><span><kbd>↑</kbd><kbd>↓</kbd> Move</span><span><kbd>↵</kbd> Run</span><span><kbd>Esc</kbd> Close</span></footer>
</dialog>
