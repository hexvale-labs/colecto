<script lang="ts">
  import type { MailItem } from '../lib/api';
  import { when } from '../lib/when';
  import Icon from '../Icon.svelte';
  let { items, fetching = false, fetchedAt = null, searching = false, selected, folder, loading, error, disabled, onopen, onretry }: {
    searching?: boolean; items: MailItem[]; selected: string | null; folder: 'Inbox' | 'Drafts' | 'Sent';
    fetching?: boolean; fetchedAt?: number | null; loading: boolean; error: string; disabled: boolean;
    onopen: (id: string) => void; onretry: () => void;
  } = $props();
  let rows = $derived.by(() => {
    const now = new Date();
    return items.map((item, index) => {
      const date = folder === 'Drafts' ? { label: '', group: '', full: '' } : when(item.date, now);
      return { item, date, heading: date.group && date.group !== (index ? when(items[index - 1].date, now).group : '') ? date.group : '' };
    });
  });
  let fetched = $derived(fetchedAt ? new Date(fetchedAt * 1000).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }) : '');
</script>

<section class="mailbox" aria-label={`${folder} messages`} aria-busy={loading}>
  <h1 class="folder-title">{folder}</h1>
  {#if (fetching || error) && items.length > 0}<p class="list-note">Showing cached mail; it may be out of date.</p>{/if}
  {#if error}<div class="list-state error"><p role="alert">{error}</p><button class="secondary" onclick={onretry}>Try again</button></div>{/if}
  {#if fetching}<p class="list-note live" role="status"><span class="pulse" aria-hidden="true"></span>Fetching latest messages…</p>{/if}
  <div class="message-list">
    {#if loading && items.length === 0}
      <div class="list-state">Loading your {folder.toLowerCase()}…</div>
    {:else if items.length === 0 && !error && !fetching}
      <div class="list-state"><h2>{searching ? 'No matching messages' : folder === 'Drafts' ? 'No drafts yet' : 'Nothing here yet'}</h2><p>{searching ? 'Try another search or clear the label filter.' : folder === 'Drafts' ? 'Start one with Compose and save it when you’re ready.' : folder === 'Sent' ? 'Messages you send will appear here.' : 'New mail will appear here when it arrives.'}</p></div>
    {:else}
      {#each rows as { item, date, heading } (item.id)}
        {#if heading}<h3 class="day">{heading}</h3>{/if}
        <button class="mail-item" class:selected={selected === item.id} aria-pressed={selected === item.id} {disabled} onclick={() => onopen(item.id)}>
          <span class="item-sender">{item.name || 'Unknown sender'}</span>
          {#if date.label}<time class="item-date" title={date.full}>{date.label}</time>{/if}
          <span class="item-subject">{item.subject || '(No subject)'}</span>
          <span class="item-preview">{item.preview || 'No preview'}</span>
        </button>
      {/each}
    {/if}
  </div>
  <footer class="list-footer">
    {#if folder === 'Drafts'}<span>Drafts are saved by your mail service.</span>
    {:else}<span>{fetching ? 'Updating…' : fetched ? `Updated ${fetched}` : 'Cached mail may be out of date.'}</span><button class="icon-button" aria-label="Refresh" title="Refresh" disabled={fetching} onclick={onretry}><Icon name="refresh" size={15} /></button>{/if}
  </footer>
</section>
