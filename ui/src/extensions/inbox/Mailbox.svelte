<script lang="ts">
  import type { AccountSync, MailItem } from '../../lib/api';
  import { when } from '../../lib/when';
  import { extensionBus } from '../../lib/bus';
  const bus = extensionBus('inbox');
  import Icon from '../../Icon.svelte';
  let { items, grouped = true, fetching = false, fetchedAt = null, sync = null, filtered = false, searching = false, selected, folder, loading, error, disabled, onopen, onretry }: {
    /** The account's sync mode and background progress, from the Host. */ sync?: AccountSync | null;
    /** A search or label narrows the list. */ filtered?: boolean;
    /** A search is on its way to the mail service. */ searching?: boolean; grouped?: boolean; items: MailItem[]; selected: string | null; folder: 'Inbox' | 'Drafts' | 'Sent';
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
  const every = (seconds: number) => seconds < 60 ? `${seconds} s` : seconds % 60 ? `${Math.round(seconds / 60)} min` : `${seconds / 60} min`;
  let mode = $derived(!sync ? '' : {
    local: 'Sample mail', jmap_push: 'JMAP push', imap_idle: 'IMAP IDLE', offline: 'Offline',
    polling: `${sync.protocol.toUpperCase()} · checks every ${every(sync.interval)}`,
  }[sync.mode]);
  let count = $derived(sync?.progress && sync.progress[1] > 0 ? ` ${Math.min(sync.progress[0], sync.progress[1])}/${sync.progress[1]}` : '');
  let busy = $derived(fetching || searching || sync?.activity === 'syncing');
  let status = $derived(
    searching ? { text: 'Searching…', detail: '' }
    : busy ? { text: `Syncing${count || '…'}`, detail: '' }
    : sync?.activity === 'prefetching' ? { text: `Downloading${count || '…'}`, detail: mode }
    : sync?.mode === 'offline' ? { text: fetched ? `Offline · updated ${fetched}` : 'Offline', detail: sync.error ?? '' }
    : { text: fetched ? `Updated ${fetched}` : 'Saved mail may be out of date.', detail: mode });
  // Messages that arrive with a refresh slide in, a few at a time; a first load or a folder
  // switch shows everything at once.
  let previous: { folder: string; ids: Set<string> } | null = null;
  let arrived = $state<Record<string, number>>({});
  $effect.pre(() => {
    const ids = items.map(item => item.id);
    const before = previous;
    previous = { folder, ids: new Set(ids) };
    const next: Record<string, number> = {};
    if (before && before.folder === folder && before.ids.size && !filtered) {
      let order = 0;
      for (const id of ids) if (!before.ids.has(id) && order < 8) next[id] = order++;
    }
    arrived = next;
  });
</script>

<section class="mailbox" aria-label={`${folder} messages`} aria-busy={loading}>
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
        <button class="mail-item" class:arrived={item.id in arrived} style:--arrival={arrived[item.id]} class:selected={selected === item.id} aria-pressed={selected === item.id} {disabled} onclick={() => { void bus.emit('v1/selected', {message_id:item.id}).catch(() => {}); onopen(item.id); }}>
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
    {:else}<span class="list-status" class:offline={!busy && sync?.mode === 'offline'} aria-live="polite" title={[status.text, status.detail].filter(Boolean).join(' · ')}><span class="status-text">{status.text}</span>{#if status.detail}<span class="status-detail">{status.detail}</span>{/if}</span><button class="icon-button refresh" class:spinning={busy} aria-label="Refresh" title="Refresh" disabled={fetching} onclick={onretry}><Icon name="refresh" size={15} /></button>{/if}
  </footer>
</section>
