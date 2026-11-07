<script lang="ts">
  import type { MailItem } from '../lib/api';
  let { items, selected, folder, loading, error, disabled, onopen, onretry }: {
    items: MailItem[]; selected: string | null; folder: 'Inbox' | 'Drafts' | 'Sent';
    loading: boolean; error: string; disabled: boolean;
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
    {#if loading}
      <div class="list-state">Loading your {folder.toLowerCase()}…</div>
    {:else if error}
      <div class="list-state"><p role="alert">{error}</p><button class="secondary" onclick={onretry}>Try again</button></div>
    {:else if items.length === 0}
      <div class="list-state"><span class="empty-mark" aria-hidden="true">✎</span><h2>{folder === 'Drafts' ? 'No drafts yet' : 'All quiet here'}</h2><p>{folder === 'Drafts' ? 'Start a note with Compose. Save it when you’re ready.' : 'There are no messages in this folder.'}</p></div>
    {:else}
      {#each items as item (item.id)}
        <button class="mail-item" class:selected={selected === item.id} aria-pressed={selected === item.id} {disabled} onclick={() => onopen(item.id)}>
          <span class="item-top"><strong>{item.name}</strong><span class="item-date">{item.date}</span></span>
          <span class="item-subject">{item.subject || '(No subject)'}</span>
          <span class="item-preview">{item.preview || 'No preview'}</span>
        </button>
      {/each}
    {/if}
  </div>
  <footer class="list-footer">{folder === 'Drafts' ? 'Drafts are saved by your mail service.' : `Reopen ${folder} to refresh.`}</footer>
</section>
