<script lang="ts">
  import { onMount } from 'svelte';
  import Mailbox from './extensions/Mailbox.svelte';
  import Reader from './extensions/Reader.svelte';
  import Composer from './extensions/Composer.svelte';
  import { inboxService, readerService, composerService } from './lib/api';
  import type { Draft, MailItem } from './lib/api';
  type View = { kind: 'message'; id: string } | { kind: 'draft'; draft: Draft | null } | null;
  let connected = $state(false);
  let folder = $state<'Inbox' | 'Drafts'>('Inbox');
  let items = $state<MailItem[]>([]);
  let view = $state<View>(null);
  let loading = $state(true);
  let error = $state('');
  let openError = $state('');
  let dirty = $state(false);
  let busy = $state(false);
  let opening = $state(false);
  let listGeneration = 0;
  let openGeneration = 0;
  let selected = $derived(view?.kind === 'message' ? view.id : view?.kind === 'draft' ? view.draft?.id ?? null : null);
  async function loadList() {
    const generation = ++listGeneration;
    const requestedFolder = folder;
    loading = true; error = ''; openError = '';
    try {
      const result: MailItem[] = requestedFolder === 'Inbox'
        ? (await inboxService.messages()).map(m => ({ id: m.id, name: m.sender, subject: m.subject, preview: m.body, date: m.date }))
        : (await inboxService.drafts()).slice().reverse().map(d => ({ id: d.id, name: d.recipient || 'No recipient', subject: d.subject, preview: d.body, date: 'Draft' }));
      if (generation === listGeneration) items = result;
    } catch (e) { if (generation === listGeneration) error = (e as Error).message; }
    finally { if (generation === listGeneration) loading = false; }
  }
  function canLeave() { return !busy && (!dirty || window.confirm('Discard unsaved changes? Your last saved draft will be kept.')); }
  function clearView() { ++openGeneration; view = null; dirty = false; openError = ''; opening = false; }
  function navigate(next: 'Inbox' | 'Drafts') {
    if (!canLeave()) return;
    clearView(); folder = next; void loadList();
  }
  function compose() { if (!canLeave()) return; clearView(); view = { kind: 'draft', draft: null }; }
  function back() { if (!canLeave()) return; clearView(); }
  async function open(id: string) {
    if (!canLeave()) return;
    clearView();
    if (folder === 'Inbox') { view = { kind: 'message', id }; return; }
    const generation = ++openGeneration;
    opening = true;
    try {
      const draft = await composerService.read(id);
      if (generation === openGeneration) view = { kind: 'draft', draft };
    } catch (e) { if (generation === openGeneration) openError = (e as Error).message; }
    finally { if (generation === openGeneration) opening = false; }
  }
  function saved(draft: Draft) {
    // Keep the mounted editor intact; refreshing the list must not reset its state.
    if (view?.kind === 'draft') view.draft = draft;
    if (folder === 'Drafts') void loadList();
  }
  function beforeUnload(event: BeforeUnloadEvent) { if (dirty || busy) { event.preventDefault(); event.returnValue = ''; } }
  onMount(() => { void loadList(); void inboxService.connected().then(value => connected = value).catch(() => {}); });
</script>

<svelte:window onbeforeunload={beforeUnload} />
<div class="app-shell" class:detail-open={view !== null}>
  <aside class="sidebar">
    <a class="brand" href="/" onclick={(event) => { event.preventDefault(); navigate('Inbox'); }} aria-label="Colecto home"><span class="brand-mark" aria-hidden="true">c</span>colecto<span class="brand-dot">.</span></a>
    <button class="compose-button" disabled={busy} onclick={compose}><span aria-hidden="true">＋</span> Compose</button>
    <nav aria-label="Mail folders">
      <button class:active={folder === 'Inbox'} aria-current={folder === 'Inbox' ? 'page' : undefined} disabled={busy} onclick={() => navigate('Inbox')}><span class="nav-icon" aria-hidden="true">▤</span>Inbox<span class="nav-dot" aria-hidden="true"></span></button>
      <button class:active={folder === 'Drafts'} aria-current={folder === 'Drafts' ? 'page' : undefined} disabled={busy} onclick={() => navigate('Drafts')}><span class="nav-icon" aria-hidden="true">✎</span>Drafts</button>
    </nav>
    <div class="sidebar-note"><span class="small-leaf" aria-hidden="true">✳</span><p>A quieter place<br />for your mail.</p></div>
    <div class="account"><span class="account-avatar" aria-hidden="true">D</span><div><strong>{connected ? 'Mail account' : 'Demo account'}</strong><span>{connected ? 'Connected mail' : 'Local to this computer'}</span></div><span class="online-dot" aria-label="Local demo"></span></div>
  </aside>
  <main class="workspace">
    <Mailbox {items} {selected} {folder} {loading} error={error || openError} disabled={busy || opening} onopen={open} onretry={loadList} />
    <div class="reading-pane">
      {#key view?.kind === 'message' ? `message:${view.id}` : view}
        {#if view?.kind === 'message'}<Reader id={view.id} service={readerService} onback={back} />
        {:else if view?.kind === 'draft'}<Composer {connected} draft={view.draft} service={composerService} onback={back} ondirty={(value) => dirty = value} onbusy={(value) => busy = value} onsaved={saved} />
        {:else}
          <section class="welcome"><div class="welcome-art" aria-hidden="true"><span class="envelope">⌑</span><span class="art-star">✳</span></div><div class="eyebrow">LESS NOISE. MORE ROOM.</div><h2>Mail, with room<br />to think.</h2><p>{opening ? 'Opening your draft…' : 'Pick a message. Settle into a conversation.\nEverything else can wait a moment.'}</p><span class="welcome-caption">COLECTO · LOCAL DEMO</span></section>
        {/if}
      {/key}
    </div>
  </main>
</div>
