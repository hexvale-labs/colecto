<script lang="ts">
  import { onMount } from 'svelte';
  import { hostService, type PendingSend, type ApiKey } from './lib/api';
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
<section class="settings-card" aria-labelledby="approvals-title">
  <h2 id="approvals-title">Send approvals</h2>
  <p>Automation cannot send until you approve the exact message below. Requests expire after 10 minutes. Changes require a new request.</p>
  <button class="secondary" disabled={busy} onclick={reload}>Refresh requests</button>
  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  <p role="status">{status}</p>
  {#if !requests.length && !busy}<p>No delivery requests.</p>{/if}
  {#each requests.slice().reverse() as request (request.id)}
    <article aria-label={`Delivery request: ${request.snapshot?.subject || '(no subject)'}`}>
      <h3>{request.snapshot?.subject || '(no subject)'}</h3>
      <dl><dt>Requested by</dt><dd>{keys.find(key => key.id === request.requester)?.name || request.requester}</dd><dt>Account</dt><dd>{request.account}</dd><dt>From</dt><dd>{request.from}</dd><dt>Status</dt><dd>{request.status}</dd><dt>Expires</dt><dd>{new Date(request.expires_at * 1000).toLocaleString()}</dd></dl>
      {#if request.snapshot}<dl>{#each Object.entries(request.snapshot).filter(([key]) => key !== 'id') as [key, value]}<dt>{key === 'recipient' ? 'To' : key === 'subject' ? 'Subject' : key === 'body' ? 'Body' : key}</dt><dd class="snapshot">{typeof value === 'string' ? value : JSON.stringify(value, null, 2)}</dd>{/each}</dl>{/if}
      <p>Encryption: none. Attachments: none.</p>
      {#if request.status === 'pending'}<div class="decisions"><button class="primary" disabled={busy} onclick={() => decide(request, 'approve')}>Approve and send</button><button class="secondary" disabled={busy} onclick={() => decide(request, 'reject')}>Reject</button></div>
      {:else if request.status === 'uncertain' || request.status === 'submitting'}<p>Delivery may have started. Do not submit a duplicate; check Sent or contact the recipient.</p>{/if}
    </article>
  {/each}
</section>
<style>
  article { padding: 1.25rem 0; border-top: 1px solid var(--border); margin-top: 1rem; overflow-wrap: anywhere; }
  h3 { margin: 0 0 1rem; }
  dl { margin: 1rem 0; } dt { font-weight: 600; margin-top: 0.75rem; } dd { margin: 0.25rem 0 0; }
  .snapshot { white-space: pre-wrap; } .decisions { display: flex; flex-wrap: wrap; gap: 0.75rem; } button { min-height: 44px; touch-action: manipulation; }
</style>
