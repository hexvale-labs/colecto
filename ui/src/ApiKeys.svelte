<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  import { hostService, type AccountInfo, type ApiKey, type Capability } from './lib/api';
  import { capabilityLabel } from './lib/capabilities';
  let { accounts }: { accounts: AccountInfo[] } = $props();
  let keys = $state<ApiKey[]>([]);
  let name = $state('');
  let selectedAccounts = $state<string[]>([]);
  let capabilities = $state<Capability[]>([]);
  // Expiry in days from now; 0 never expires.
  let expiry = $state(0);
  const lifetimes = [{ days: 0, label: 'Never' }, { days: 1, label: '1 day' }, { days: 7, label: '1 week' }, { days: 30, label: '30 days' }, { days: 90, label: '90 days' }];
  const readOnly: Capability[] = ['account.read', 'messages.list', 'messages.read', 'drafts.list', 'drafts.read'];
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
      const result = await hostService.mintApiKey({ name, accounts: selectedAccounts, capabilities, ...(expiry ? { expires_at: Math.floor(Date.now() / 1000) + expiry * 86_400 } : {}) });
      secret = result.secret; name = ''; selectedAccounts = []; capabilities = []; expiry = 0;
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
  <div class="settings-group setting setting-row">
    <div class="setting-text"><h3>MCP server</h3><p>Lets MCP clients use a key below at <code>{window.location.origin}/api/v1/mcp</code>. Off until you turn it on; keys keep working over HTTP either way.</p></div>
    <label class="switch"><input type="checkbox" checked={!!mcp} disabled={busy || mcp === null} onchange={(event) => void toggleMcp(event.currentTarget.checked)} /><span class="sr-only">MCP server</span></label>
  </div>
  {#if secret}<section class="secret" aria-label="New API key"><h3>Copy your key now</h3><p>This secret is shown only here, until you leave or dismiss it.</p><textarea aria-label="API key secret" readonly value={secret} spellcheck="false"></textarea><div><button class="secondary" onclick={copy}>Copy key</button><button class="text-button" onclick={() => secret = ''}>Dismiss secret</button></div></section>{/if}
  <form class="settings-group" onsubmit={create}>
    <h3 class="group-label">New key</h3>
    <p>Choose what the key may reach. You see its secret once, right after it is made.</p>
    <label class="field">Key name<input bind:value={name} maxlength="80" required placeholder="Desktop assistant" autocomplete="off" /></label>
    <fieldset class="check-group" disabled={busy}><legend>Accounts</legend>
      <div class="chips">{#each accounts as account (account.id)}<label class="chip toggle"><input type="checkbox" bind:group={selectedAccounts} value={account.id} /><Icon name="check" size={12} />{account.settings?.name || account.settings?.address || 'Demo account'}</label>{/each}</div>
    </fieldset>
    <fieldset class="check-group" disabled={busy}>
      <legend>Operations</legend>
      <div class="quick"><button type="button" class="text-button" onclick={() => capabilities = [...readOnly]}>Read only</button><button type="button" class="text-button" onclick={() => capabilities = [...operations]}>Everything</button>{#if capabilities.length}<button type="button" class="text-button muted" onclick={() => capabilities = []}>Clear</button>{/if}</div>
      <div class="chips">{#each operations as operation (operation)}<label class="chip toggle" title={operation}><input type="checkbox" bind:group={capabilities} value={operation} /><Icon name="check" size={12} />{capabilityLabel(operation)}</label>{/each}</div>
      {#if capabilities.includes('messages.send')}<p class="hint">Sending always waits for your approval in Settings → Send approvals.</p>{/if}
    </fieldset>
    <fieldset class="segmented-field check-group" disabled={busy}>
      <legend>Expires</legend>
      <div class="segmented">{#each lifetimes as lifetime (lifetime.days)}<label><input type="radio" name="key-expiry" value={lifetime.days} bind:group={expiry} />{lifetime.label}</label>{/each}</div>
    </fieldset>
    <button class="primary" disabled={busy || !name.trim() || !selectedAccounts.length || !capabilities.length}>{busy ? 'Saving…' : 'Generate API key'}</button>
  </form>
  <div class="settings-group">
  <h3 class="group-label">Issued keys</h3>
  {#if !keys.length}<p class="empty-note">No API keys yet. Your mail is keeping to itself.</p>{/if}
  {#if keys.length}<div class="settings-list">
  {#each keys as key (key.id)}<article class="issued-key"><h4>{key.name}</h4><p>{key.accounts.map(id => accounts.find(a => a.id === id)?.settings?.name || accounts.find(a => a.id === id)?.settings?.address || id).join(', ')}</p><p class="caps">{#each key.capabilities as capability}<code title={capabilityLabel(capability)}>{capability}</code>{/each}</p><p class="meta">Created {date(key.created_at)} · Last used {date(key.last_used_at)}<br />Expires {key.expires_at ? date(key.expires_at) : 'Never'}</p>{#if key.revoked_at}<p>Revoked {date(key.revoked_at)}</p>{:else}<button class="secondary" disabled={busy} onclick={() => revoke(key.id)} aria-label={`Revoke ${key.name}`}>Revoke key</button>{/if}</article>{/each}
  </div>{/if}
  </div>
</section>

<style>
  .check-group legend + * { margin-top: 0; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip.toggle { max-width: 100%; overflow: hidden; text-overflow: ellipsis; }
  .quick { display: flex; gap: 14px; margin: -2px 0 8px; }
  .quick .text-button { min-height: 24px; font-size: calc(12.5px * var(--text-scale)); }
  .quick .muted { color: var(--ink-3); }
  .hint { margin-top: 8px; color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); }
  .check-group legend { width: 100%; margin-bottom: 8px; padding: 0; color: var(--ink-2); font-size: calc(12.5px * var(--text-scale)); font-weight: 540; }
  .settings-group > .primary { justify-self: start; }
  .issued-key { display: grid; gap: 6px; overflow-wrap: anywhere; }
  .issued-key p { color: var(--ink-2); font-size: calc(13px * var(--text-scale)); }
  .issued-key .meta { color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); }
  .issued-key .secondary { justify-self: start; margin-top: 6px; }
  .caps { display: flex; flex-wrap: wrap; gap: 6px; }
  .caps code { padding: 2px 7px; border-radius: 6px; background: var(--sunk); }
  h4 { margin: 0; font-size: calc(14px * var(--text-scale)); font-weight: 540; }
  .secret { display: grid; gap: 10px; margin: 16px 0; padding: 18px; border-radius: 12px; background: var(--select); }
  .secret h3 { margin: 0; color: var(--mint-hi); }
  .secret p { color: var(--ink-2); }
  .secret div { display: flex; gap: 12px; }
  textarea { width: 100%; min-height: 84px; padding: 10px 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--sunk); resize: vertical; word-break: break-all; font: calc(13px * var(--text-scale))/1.5 ui-monospace, monospace; }
</style>
