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
  const operations: Capability[] = ['account.read', 'messages.list', 'messages.read', 'messages.label', 'drafts.list', 'drafts.read', 'drafts.write', 'messages.send', 'app.navigate'];
  let mcp = $state<boolean | null>(null);
  async function reload() { keys = await hostService.apiKeys(); }
  onMount(() => {
    void reload().catch(e => error = e.message);
    void hostService.preferences().then(p => mcp = p.mcp).catch(e => error = e.message);
  });
  async function toggleMcp(on: boolean) {
    busy = true; error = ''; status = '';
    try { mcp = (await hostService.savePreferences({ mcp: on })).mcp; status = on ? 'MCP server turned on.' : 'MCP server turned off.'; }
    catch (e) { error = (e as Error).message; } finally { busy = false; }
  }
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

<section class="settings-section" aria-labelledby="api-keys-title">
  <header class="section-head"><h2 id="api-keys-title">API keys</h2>
  <p>Give automation access to selected accounts and operations. Keys cannot manage settings, and <code>messages.send</code> only asks you for approval; it never sends directly.</p></header>
  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  <p role="status">{status}</p>
  <div class="settings-card setting-row">
    <div><h3>MCP server</h3><p>Lets MCP clients use a key below at <code>{window.location.origin}/api/v1/mcp</code>. Off until you turn it on; keys keep working over HTTP either way.</p></div>
    <label class="switch"><input type="checkbox" checked={!!mcp} disabled={busy || mcp === null} onchange={(event) => void toggleMcp(event.currentTarget.checked)} /><span class="sr-only">MCP server</span></label>
  </div>
  {#if secret}<section class="secret" aria-label="New API key"><h3>Copy your key now</h3><p>This secret is shown only here, until you leave or dismiss it.</p><textarea aria-label="API key secret" readonly value={secret} spellcheck="false"></textarea><div><button class="secondary" onclick={copy}>Copy key</button><button class="text-button" onclick={() => secret = ''}>Dismiss secret</button></div></section>{/if}
  <form class="settings-card" onsubmit={create}>
    <h3>New key</h3>
    <label class="field">Key name<input bind:value={name} maxlength="80" required placeholder="Desktop assistant" autocomplete="off" /></label>
    <fieldset class="check-group" disabled={busy}><legend>Accounts</legend>{#each accounts as account}<label class="choice"><input type="checkbox" bind:group={selectedAccounts} value={account.id} />{account.settings?.name || account.settings?.address || 'Demo account'}</label>{/each}</fieldset>
    <fieldset class="check-group" disabled={busy}><legend>Operations</legend>{#each operations as operation}<label class="choice"><input type="checkbox" bind:group={capabilities} value={operation} /><code>{operation}</code></label>{/each}</fieldset>
    <label class="field">Expires at (optional)<input type="datetime-local" bind:value={expiry} /></label>
    <button class="primary" disabled={busy || !name.trim() || !selectedAccounts.length || !capabilities.length}>{busy ? 'Saving…' : 'Generate API key'}</button>
  </form>
  <h3 class="list-title">Issued keys</h3>
  {#if !keys.length}<p class="empty-note">No API keys yet. Your mail is keeping to itself.</p>{/if}
  {#each keys as key (key.id)}<article class="settings-card issued-key"><h4>{key.name}</h4><p>{key.accounts.map(id => accounts.find(a => a.id === id)?.settings?.name || accounts.find(a => a.id === id)?.settings?.address || id).join(', ')}</p><p class="caps">{#each key.capabilities as capability}<code>{capability}</code>{/each}</p><p class="meta">Created {date(key.created_at)} · Last used {date(key.last_used_at)}<br />Expires {key.expires_at ? date(key.expires_at) : 'Never'}</p>{#if key.revoked_at}<p>Revoked {date(key.revoked_at)}</p>{:else}<button class="secondary" disabled={busy} onclick={() => revoke(key.id)} aria-label={`Revoke ${key.name}`}>Revoke key</button>{/if}</article>{/each}
</section>

<style>
  .check-group .choice { padding: .35rem .65rem; border: 1px solid var(--line); border-radius: 999px; font-size: .9rem; overflow-wrap: anywhere; max-width: 100%; }
  .check-group .choice:has(input:checked) { background: var(--select); border-color: var(--mint); }
  .check-group .choice:has(input:focus-visible) { outline: 2px solid var(--mint); outline-offset: 2px; }
  .check-group { display: flex; flex-wrap: wrap; gap: 2px 18px; margin-top: 18px; }
  .check-group legend { width: 100%; margin-bottom: 6px; padding: 0; color: var(--ink-2); font-size: 12.5px; font-weight: 560; }
  .settings-card > .primary { margin-top: 20px; }
  .list-title { margin: 28px 0 10px; font-size: 15px; }
  .issued-key { display: grid; gap: 6px; margin-bottom: 10px; overflow-wrap: anywhere; }
  .issued-key p { color: var(--ink-2); font-size: 13px; }
  .issued-key .meta { color: var(--ink-3); font-size: 12.5px; }
  .issued-key .secondary { justify-self: start; margin-top: 6px; }
  .caps { display: flex; flex-wrap: wrap; gap: 6px; }
  .caps code { padding: 2px 7px; border-radius: 6px; background: var(--sunk); }
  h4 { margin: 0; font-size: 14px; }
  .secret { display: grid; gap: 10px; margin: 16px 0; padding: 18px; border-radius: 12px; background: var(--select); }
  .secret h3 { margin: 0; color: var(--mint-hi); }
  .secret p { color: var(--ink-2); }
  .secret div { display: flex; gap: 12px; }
  textarea { width: 100%; min-height: 84px; padding: 10px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--sunk); resize: vertical; word-break: break-all; font: 13px/1.5 ui-monospace, monospace; }
</style>
