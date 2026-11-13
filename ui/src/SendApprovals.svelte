<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { hostService, type PendingSend, type ApiKey } from './lib/api';
  import Icon from './Icon.svelte';
  let { requestId = null }: { requestId?: string | null } = $props();
  $effect(() => {
    const id = requestId;
    if (id && !busy) void tick().then(() => document.getElementById(`send-request-${id}`)?.focus());
  });
  let requests = $state<PendingSend[]>([]);
  let keys = $state<ApiKey[]>([]);
  let busy = $state(false);
  let error = $state('');
  let status = $state('');
  async function reload() {
    busy = true; error = '';
    try { [requests, keys] = await Promise.all([hostService.sendRequests(), hostService.apiKeys()]); }
    catch (e) { error = (e as Error).message; } finally { busy = false; }
  }
  onMount(() => { void reload(); });
  async function decide(request: PendingSend, decision: 'approve' | 'reject') {
    busy = true; error = ''; status = '';
    try {
      const result = await hostService.decideSend(request.id, decision);
      requests = requests.map(item => item.id === result.id ? result : item);
      status = result.status === 'uncertain' ? 'Delivery outcome is uncertain. Check Sent and the recipient before trying again.' : `Request ${result.status}.`;
    } catch (e) { error = `${(e as Error).message} Refresh the request status before taking further action.`; }
    finally { busy = false; }
  }
</script>
<section class="settings-section" aria-labelledby="approvals-title">
  <header class="section-head with-action"><div><h2 id="approvals-title">Send approvals</h2>
  <p>Automation cannot send until you approve the exact message below. Requests expire after 10 minutes, and any change needs a new request.</p></div>
  <button class="secondary" disabled={busy} onclick={reload}><Icon name="refresh" size={15} />Refresh</button></header>
  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  <p role="status">{status}</p>
  {#if !requests.length && !busy}<p class="empty-note">Nothing waiting for you. Automation is behaving.</p>{/if}
  <div class="settings-list">
  {#each requests.slice().reverse() as request (request.id)}
    <article id={`send-request-${request.id}`} tabindex="-1" aria-label={`Delivery request: ${request.snapshot?.subject || '(no subject)'}`}>
      <h3>{request.snapshot?.subject || '(no subject)'}</h3>
      <dl><dt>Requested by</dt><dd>{keys.find(key => key.id === request.requester)?.name || request.requester}</dd><dt>Account</dt><dd>{request.account}</dd><dt>From</dt><dd>{request.from}</dd><dt>Status</dt><dd>{request.status}</dd><dt>Expires</dt><dd>{new Date(request.expires_at * 1000).toLocaleString()}</dd></dl>
      {#if request.snapshot}<dl>{#each Object.entries(request.snapshot).filter(([key]) => key !== 'id') as [key, value]}<dt>{key === 'recipient' ? 'To' : key === 'subject' ? 'Subject' : key === 'body' ? 'Body' : key}</dt><dd class="snapshot">{typeof value === 'string' ? value : JSON.stringify(value, null, 2)}</dd>{/each}</dl>{/if}
      <p>Encryption: none. Attachments: none.</p>
      {#if request.status === 'pending' && request.snapshot}<div class="decisions"><button class="primary" disabled={busy} onclick={() => decide(request, 'approve')}>Approve and send</button><button class="secondary" disabled={busy} onclick={() => decide(request, 'reject')}>Reject</button></div>
      {:else if request.status === 'uncertain' || request.status === 'submitting'}<p>Delivery may have started. Do not submit a duplicate; check Sent or contact the recipient.</p>{/if}
    </article>
  {/each}
  </div>
</section>
<style>
  article { display: grid; gap: 4px; overflow-wrap: anywhere; outline: none; }
  h3 { margin: 0 0 8px; font-size: calc(15px * var(--text-scale)); font-weight: 600; }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 8px 18px; margin: 8px 0; font-size: calc(13.5px * var(--text-scale)); }
  dt { color: var(--ink-3); font-weight: 540; } dd { margin: 0; color: var(--ink); }
  dl + dl { padding-top: 12px; border-top: 1px solid var(--line-soft); }
  .snapshot { white-space: pre-wrap; } article > p { color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); }
  .decisions { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 10px; }
  @container settings (max-width: 480px) { dl { grid-template-columns: minmax(0, 1fr); gap: 2px; } dd { margin-bottom: 8px; } }
</style>
