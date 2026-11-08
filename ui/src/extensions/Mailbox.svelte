<script lang="ts">
  import type { MailItem } from '../lib/api';
  let { items, fetching = false, fetchedAt = null, searching = false, selected, folder, loading, error, disabled, onopen, onretry }: {
    searching?: boolean; items: MailItem[]; selected: string | null; folder: 'Inbox' | 'Drafts' | 'Sent';
    fetching?: boolean; fetchedAt?: number | null; loading: boolean; error: string; disabled: boolean;
    onopen: (id: string) => void; onretry: () => void;
  } = $props();
</script>

<section class="mailbox" aria-label={`${folder} messages`} aria-busy={loading}>
  <header class="mailbox-header">
    <div class="eyebrow">MAILBOX</div>
    <div class="title-row"><h1>{folder}</h1><span class="count">{loading ? '…' : items.length}</span></div>
    <p>{folder === 'Drafts' ? 'Saved locally in this account.' : folder === 'Sent' ? 'Messages in your provider’s Sent mailbox.' : 'Latest messages in this account.'}</p>
  </header>
  <div class="message-list">
    {#if (fetching || error) && items.length > 0}<p class="cache-status">Showing cached mail; it may be out of date.</p>{/if}
    {#if error}<div class="list-state"><p role="alert">{error}</p><button class="secondary" onclick={onretry}>Try again</button></div>{/if}
    {#if fetching}<p class="cache-status" role="status">Fetching latest messages…</p>{/if}
    {#if loading && items.length === 0}
      <div class="list-state">Loading your {folder.toLowerCase()}…</div>

    {:else if items.length === 0 && !error && !fetching}
      <div class="list-state"><span class="empty-mark" aria-hidden="true">✎</span><h2>{searching ? 'No matching messages' : folder === 'Drafts' ? 'No drafts yet' : 'All quiet here'}</h2><p>{searching ? 'Try another search or clear the label filter.' : folder === 'Drafts' ? 'Start a note with Compose. Save it when you’re ready.' : 'There are no messages in this folder.'}</p></div>
    {:else}
      {#each items as item (item.id)}
        <button class="mail-item" class:selected={selected === item.id} aria-pressed={selected === item.id} {disabled} onclick={() => onopen(item.id)}>
          <span class="item-top"><strong>{item.name || 'Unknown sender'}</strong><span class="item-date">{item.date}</span></span>
          <span class="item-subject">{item.subject || '(No subject)'}</span>
          <span class="item-preview">{item.preview || 'No preview'}</span>
        </button>
      {/each}
    {/if}
  </div>
  <footer class="list-footer" class:cache-footer={folder !== 'Drafts'}>{folder === 'Drafts' ? 'Drafts are saved by your mail service.' : `${fetchedAt ? `Cached · last fetched ${new Date(fetchedAt * 1000).toLocaleString()}.` : 'Cached mail may be out of date.'} ${fetching ? 'Updating…' : ''}`}{#if folder !== 'Drafts'}<button class="text-button" disabled={fetching} onclick={onretry}>Refresh</button>{/if}</footer>
</section>

<style>
  .cache-status { padding: 8px 20px; margin: 0; color: var(--muted); font-size: 12px; }
  .cache-footer { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .cache-footer button { flex-shrink: 0; min-height: 44px; }
</style>
