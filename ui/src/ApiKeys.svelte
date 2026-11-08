<script lang="ts">
  import { onMount } from 'svelte';
  import { hostService, type AccountInfo, type ApiKey, type Capability } from './lib/api';
  let { accounts }: { accounts: AccountInfo[] } = $props();
  let keys = $state<ApiKey[]>([]);
  let name = $state('');
  let selectedAccounts = $state<string[]>([]);
  let capabilities = $state<Capability[]>([]);
  let expiry = $state('');
  let secret = $state('');
  let error = $state('');
  let status = $state('');
  let busy = $state(false);
  const operations: Capability[] = ['account.read', 'messages.list', 'messages.read', 'messages.label', 'drafts.list', 'drafts.read', 'drafts.write', 'messages.send'];
  async function reload() { keys = await hostService.apiKeys(); }
  onMount(() => { void reload().catch(e => error = e.message); });
  async function create(event: SubmitEvent) {
    event.preventDefault(); busy = true; error = ''; status = ''; secret = '';
    try {
      const result = await hostService.mintApiKey({ name, accounts: selectedAccounts, capabilities, ...(expiry ? { expires_at: Math.floor(new Date(expiry).getTime() / 1000) } : {}) });
      secret = result.secret; name = ''; selectedAccounts = []; capabilities = []; expiry = '';
      await reload();
    } catch (e) { error = (e as Error).message; } finally { busy = false; }
  }
  async function revoke(id: string) {
    busy = true; error = ''; status = '';
    try { await hostService.revokeApiKey(id); secret = ''; await reload(); status = 'API key revoked.'; }
    catch (e) { error = (e as Error).message; } finally { busy = false; }
  }
  async function copy() {
    try { await navigator.clipboard.writeText(secret); status = 'API key copied.'; }
    catch { error = 'Copy is unavailable. Select and copy the key below.'; }
  }
  const date = (value: number | null | undefined) => value ? new Date(value * 1000).toLocaleString() : 'Never';
</script>

<section class="settings-card" aria-labelledby="api-keys-title">
  <h2 id="api-keys-title">API keys</h2>
  <p>Give automation access to selected accounts and operations. Keys cannot manage settings. messages.send requests human approval; it never sends directly.</p>
  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  <p role="status">{status}</p>
  {#if secret}<section class="secret" aria-label="New API key"><h3>Copy your key now</h3><p>This secret is shown only here, until you leave or dismiss it.</p><textarea aria-label="API key secret" readonly value={secret} spellcheck="false"></textarea><button class="secondary" onclick={copy}>Copy key</button> <button class="text-button" onclick={() => secret = ''}>Dismiss secret</button></section>{/if}
  <form onsubmit={create}>
    <label class="field">Key name<input bind:value={name} maxlength="80" required placeholder="Desktop assistant" autocomplete="off" /></label>
    <fieldset disabled={busy}><legend>Accounts</legend>{#each accounts as account}<label class="choice"><input type="checkbox" bind:group={selectedAccounts} value={account.id} />{account.settings?.name || account.settings?.address || 'Demo account'}</label>{/each}</fieldset>
    <fieldset disabled={busy}><legend>Operations</legend>{#each operations as operation}<label class="choice"><input type="checkbox" bind:group={capabilities} value={operation} />{operation}</label>{/each}</fieldset>
    <label class="field">Expires at (optional)<input type="datetime-local" bind:value={expiry} /></label>
    <button class="primary" disabled={busy || !name.trim() || !selectedAccounts.length || !capabilities.length}>{busy ? 'Saving…' : 'Generate API key'}</button>
  </form>
  <h3>Issued keys</h3>
  {#if !keys.length}<p>No API keys yet.</p>{/if}
  {#each keys as key (key.id)}<article class="issued-key"><h4>{key.name}</h4><p>{key.accounts.map(id => accounts.find(a => a.id === id)?.settings?.name || accounts.find(a => a.id === id)?.settings?.address || id).join(', ')}</p><p>{key.capabilities.join(', ')}</p><p>Created {date(key.created_at)} · Last used {date(key.last_used_at)}<br />Expires {key.expires_at ? date(key.expires_at) : 'Never'}</p>{#if key.revoked_at}<p>Revoked {date(key.revoked_at)}</p>{:else}<button class="secondary" disabled={busy} onclick={() => revoke(key.id)} aria-label={`Revoke ${key.name}`}>Revoke key</button>{/if}</article>{/each}
</section>

<style>
  fieldset { margin: 1rem 0; border: 1px solid var(--border); border-radius: 8px; padding: 0.75rem; }
  .choice { display: flex; align-items: center; gap: 0.6rem; min-height: 44px; overflow-wrap: anywhere; }
  .choice input { width: 18px; height: 18px; flex-shrink: 0; }
  .issued-key { padding: 1rem 0; border-top: 1px solid var(--border); overflow-wrap: anywhere; }
  h4 { margin: 0; }
  .secret { margin: 1rem 0; padding: 1rem; border: 1px solid var(--border); border-radius: 8px; }
  textarea { width: 100%; min-height: 100px; resize: vertical; word-break: break-all; font-size: 16px; }
</style>
