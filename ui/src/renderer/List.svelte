<script lang="ts">
  // A message list: rows and day headings, as the mailbox always drew them. Below 200 rows every
  // row is in the DOM; above that only the rows near the viewport are, so thousands stay smooth.
  import type { Snippet } from 'svelte';
  import type { Row } from '../runtime/protocol';
  let { rows, selected, arrived, onselect, empty }: { rows: Row[]; selected?: string; arrived: string[]; onselect: (key: string) => void; empty: Snippet } = $props();
  const THRESHOLD = 200;
  const OVERSCAN = 12;
  let viewport = $state<HTMLElement>();
  let scrollTop = $state(0);
  let height = $state(800);
  // One measurement per kind of row; until measured, a reasonable guess.
  let rowHeight = $state(72);
  let headingHeight = $state(36);
  const virtual = $derived(rows.length > THRESHOLD);
  const offsets = $derived.by(() => {
    if (!virtual) return [];
    const list = new Array<number>(rows.length + 1);
    list[0] = 0;
    for (let index = 0; index < rows.length; index++) list[index + 1] = list[index] + (rows[index].kind === 'heading' ? headingHeight : rowHeight);
    return list;
  });
  const range = $derived.by(() => {
    if (!virtual) return [0, rows.length];
    let start = 0;
    while (start < rows.length && offsets[start + 1] < scrollTop) start++;
    let end = start;
    while (end < rows.length && offsets[end] < scrollTop + height) end++;
    return [Math.max(0, start - OVERSCAN), Math.min(rows.length, end + OVERSCAN)];
  });
  function measure(node: HTMLElement, kind: string) {
    const size = node.getBoundingClientRect().height;
    if (size > 0) { if (kind === 'heading') headingHeight = size; else rowHeight = size; }
  }
  $effect(() => {
    if (!viewport) return;
    const observer = new ResizeObserver(() => height = viewport!.clientHeight);
    observer.observe(viewport);
    return () => observer.disconnect();
  });
</script>

{#snippet row(item: Row)}
  {#if item.kind === 'heading'}
    <h3 class="day" use:measure={'heading'}>{(item.node as { text?: string }).text}</h3>
  {:else}
    {@const node = item.node as Record<string, string | boolean | undefined>}
    {@const order = arrived.indexOf(item.key)}
    <button class="mail-item" class:arrived={order >= 0} style:--arrival={order >= 0 ? order : undefined} class:selected={selected === item.key} class:unread={node.unread} aria-pressed={selected === item.key} disabled={!!node.disabled} use:measure={'row'} onclick={() => onselect(item.key)}>
      <span class="item-sender">{node.sender || 'Unknown sender'}</span>
      {#if node.date}<time class="item-date" title={node.date_title as string}>{node.date}</time>{/if}
      <span class="item-subject">{node.subject || '(No subject)'}</span>
      <span class="item-preview">{node.preview || 'No preview'}</span>
    </button>
  {/if}
{/snippet}

<div class="message-list" bind:this={viewport} onscroll={() => scrollTop = viewport!.scrollTop}>
  {#if !rows.length}
    {@render empty()}
  {:else if virtual}
    <div style:height={`${offsets[range[0]]}px`} aria-hidden="true"></div>
    {#each rows.slice(range[0], range[1]) as item (item.key)}{@render row(item)}{/each}
    <div style:height={`${offsets[rows.length] - offsets[range[1]]}px`} aria-hidden="true"></div>
  {:else}
    {#each rows as item (item.key)}{@render row(item)}{/each}
  {/if}
</div>
