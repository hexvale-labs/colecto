<script lang="ts">
  import type { ListDensity, MailItem } from '../../lib/api';
  import { when } from '../../lib/when';
  import { extensionBus } from '../../lib/bus';
  const bus = extensionBus('inbox');
  import Icon from '../../Icon.svelte';
  let { items, grouped = true, density = 'full', fetching = false, fetchedAt = null, filtered = false, searching = false, selected, folder, loading, error, disabled, onopen, onretry }: {
    /** A search or label narrows the list. */ filtered?: boolean;
    /** A search is on its way to the mail service. */ searching?: boolean;
    /** Group rows under date headings such as Today and November. */ grouped?: boolean;
    /** Full rows show a preview line, medium rows drop it, compact rows are one line. */ density?: ListDensity; items: MailItem[]; selected: string | null; folder: 'Inbox' | 'Drafts' | 'Sent';
    fetching?: boolean; fetchedAt?: number | null; loading: boolean; error: string; disabled: boolean;
    onopen: (id: string) => void; onretry: () => void;
  } = $props();
  let rows = $derived.by(() => {
    const now = new Date();
    return items.map((item, index) => {
      const date = folder === 'Drafts' ? { label: '', group: '', full: '' } : when(item.date, now);
      return { item, date, heading: grouped && date.group && date.group !== (index ? when(items[index - 1].date, now).group : '') ? date.group : '' };
    });
  });
  let fetched = $derived(fetchedAt ? new Date(fetchedAt * 1000).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }) : '');
</script>

<section class="mailbox" data-density={density} aria-label={`${folder} messages`} aria-busy={loading}>
  <h1 class="folder-title">{folder}</h1>
  {#if error}<div class="list-state error"><p role="alert">{error}</p>{#if items.length > 0}<p>Showing saved mail; it may be out of date.</p>{/if}<button class="secondary" onclick={onretry}>Try again</button></div>{/if}
  <div class="message-list">
    {#if loading && items.length === 0}
      <div class="list-state">Loading your {folder.toLowerCase()}…</div>
    {:else if items.length === 0 && searching}
      <div class="list-state">Searching…</div>
    {:else if items.length === 0 && !error && !fetching}
      <div class="list-state"><h2>{filtered ? 'No matching messages' : folder === 'Drafts' ? 'No drafts yet' : 'Nothing here yet'}</h2><p>{filtered ? 'Try another search or clear the label filter.' : folder === 'Drafts' ? 'Start one with Compose and save it when you’re ready.' : folder === 'Sent' ? 'Messages you send will appear here.' : 'New mail will appear here when it arrives.'}</p></div>
    {:else}
      {#each rows as { item, date, heading } (item.id)}
        {#if heading}<h3 class="day">{heading}</h3>{/if}
        <button class="mail-item" class:selected={selected === item.id} aria-pressed={selected === item.id} {disabled} onclick={() => { void bus.emit('v1/selected', {message_id:item.id}).catch(() => {}); onopen(item.id); }}>
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
    {:else}<span class="list-status" aria-live="polite">{#if fetching || searching}<span class="pulse" aria-hidden="true"></span>{/if}{fetching ? 'Fetching latest messages…' : searching ? 'Searching…' : fetched ? `Updated ${fetched}` : 'Saved mail may be out of date.'}</span><button class="icon-button" aria-label="Refresh" title="Refresh" disabled={fetching} onclick={onretry}><Icon name="refresh" size={15} /></button>{/if}
  </footer>
</section>
