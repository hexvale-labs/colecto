<script lang="ts">
  // Another extension's app action, drawn where an extension asks for it (the mailbox header
  // shows Compose). While one is visible, the shell hides that action from the top bar.
  import { untrack } from 'svelte';
  import Icon, { type IconName } from '../Icon.svelte';
  import { runtimes } from '../runtime/runtimes.svelte';
  let { action, labelled = false }: { action: string; labelled?: boolean } = $props();
  const [owner, id] = $derived(action.split('/'));
  const runtime = $derived(runtimes.get(owner));
  const item = $derived(runtime?.appActions.find(entry => entry.id === id));
  $effect(() => { const key = action; untrack(() => runtimes.showRef(key, true)); return () => untrack(() => runtimes.showRef(key, false)); });
</script>

{#if runtime && item}
  <button class={labelled ? 'compose-button' : 'compose-icon'} aria-label={item.label} title={item.shortcut ? `${item.label}  ${item.shortcut.toUpperCase()}` : item.label} disabled={item.disabled || runtimes.busy || !runtimes.ready} onclick={() => runtime.send({ t: 'app_action', id: item.id })}><Icon name={(item.icon ?? 'compose') as IconName} />{#if labelled}<span>{item.label}</span>{/if}</button>
{/if}
