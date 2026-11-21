<script lang="ts">
  // A message list: rows and day headings, as the mailbox always drew them. Below 200 rows every
  // row is in the DOM; above that only the rows near the viewport are, so thousands stay smooth.
  import type { Snippet } from 'svelte';
  import type { Row } from '../runtime/protocol';
  import Icon from '../Icon.svelte';
  let { rows, selected, arrived, more, onselect, onmenu, onend, empty }: { rows: Row[]; selected?: string; arrived: string[]; more?: 'available' | 'loading' | 'end' | 'error'; onselect: (key: string) => void; onmenu?: (key: string, x: number, y: number) => void; onend?: () => void; empty: Snippet } = $props();
  // Right-click or long-press, and the Menu key or Shift+F10 on a focused row, open its menu.
  function context(event: MouseEvent, key: string) {
    if (!onmenu) return;
    event.preventDefault();
    onmenu(key, event.clientX, event.clientY);
  }
  function menuKey(event: KeyboardEvent, key: string) {
    if (!onmenu || !(event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey))) return;
    event.preventDefault();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    onmenu(key, rect.left + 24, rect.bottom - 8);
  }
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
  let measuredRow = false;
  let measuredHeading = false;
  function measure(node: HTMLElement, kind: string) {
    if (!virtual || (kind === 'heading' ? measuredHeading : measuredRow)) return;
    const size = node.getBoundingClientRect().height;
    if (size > 0) {
      if (kind === 'heading') { headingHeight = size; measuredHeading = true; }
      else { rowHeight = size; measuredRow = true; }
    }
  }
  $effect(() => {
    if (!viewport) return;
    const observer = new ResizeObserver(() => { height = viewport!.clientHeight; measuredRow = false; measuredHeading = false; });
    observer.observe(viewport);
    return () => observer.disconnect();
  });
  // Older rows load when the end comes within a screen of view (#136): once for each length
  // of the list, so a page that adds nothing never asks again by itself.
  let askedAt = -1;
  function checkEnd() {
    if (more !== 'available' || !viewport || rows.length === askedAt) return;
    if (viewport.scrollTop + height < viewport.scrollHeight - height) return;
    askedAt = rows.length;
    onend?.();
  }
  $effect(() => {
    void more; void rows.length; void height;
    checkEnd();
  });
  let scrollFrame = 0;
  function onscroll() {
    if (scrollFrame) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = 0;
      if (!viewport) return;
      if (virtual) scrollTop = viewport.scrollTop;
      checkEnd();
    });
  }
  $effect(() => () => { if (scrollFrame) cancelAnimationFrame(scrollFrame); });
</script>

{#snippet row(item: Row)}
  {#if item.kind === 'heading'}
    <h3 class="day" use:measure={'heading'}>{(item.node as { text?: string }).text}</h3>
  {:else}
    {@const node = item.node as Record<string, string | boolean | undefined>}
    {@const order = arrived.indexOf(item.key)}
    <button class="mail-item" class:arrived={order >= 0} style:--arrival={order >= 0 ? order : undefined} class:selected={selected === item.key} class:unread={node.unread} aria-pressed={selected === item.key} disabled={!!node.disabled} use:measure={'row'} onclick={() => onselect(item.key)} oncontextmenu={(event) => context(event, item.key)} onkeydown={(event) => menuKey(event, item.key)}>
      <span class="item-sender">{#if node.starred}<span class="item-star" title="Starred"><Icon name="star" size={12} /><span class="sr-only">Starred, </span></span>{/if}{node.sender || 'Unknown sender'}</span>
      {#if node.date}<time class="item-date" title={node.date_title as string}>{node.date}</time>{/if}
      <span class="item-subject">{node.subject || '(No subject)'}</span>
      <span class="item-preview">{node.preview || 'No preview'}</span>
    </button>
  {/if}
{/snippet}

<div class="message-list" bind:this={viewport} {onscroll}>
  {#if !rows.length}
    {@render empty()}
  {:else if virtual}
    <div style:height={`${offsets[range[0]]}px`} aria-hidden="true"></div>
    {#each rows.slice(range[0], range[1]) as item (item.key)}{@render row(item)}{/each}
    <div style:height={`${offsets[rows.length] - offsets[range[1]]}px`} aria-hidden="true"></div>
  {:else}
    {#each rows as item (item.key)}{@render row(item)}{/each}
  {/if}
  {#if rows.length && more && more !== 'available'}
    <div class="list-more" class:error={more === 'error'} role="status">
      {#if more === 'loading'}<span class="list-more-spinner" aria-hidden="true"></span>Loading older messages…
      {:else if more === 'end'}No older messages
      {:else}Older messages could not be loaded.<button class="text-button" onclick={() => { askedAt = -1; onend?.(); }}>Try again</button>{/if}
    </div>
  {/if}
</div>
