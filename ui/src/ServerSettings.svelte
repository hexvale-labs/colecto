<script lang="ts">
  import Icon from './Icon.svelte';
  import { onMount } from 'svelte';
  import { setAccessToken } from './lib/api';
  type Service = { installed: boolean; enabled: boolean; active: boolean; supported: boolean; socket?: string; token_file?: string };
  let services = $state<Record<string, Service>>({});
  let upstream = $state<{address: string; token_file: string} | null>(null);
  let address = $state('');
  let error = $state('');
  let busy = $state(false);
  let reachable = $state(false);
  let local = $state(false);
  async function call(path: string, method = 'GET', body?: unknown) {
    const response = await fetch(`/api/v1${path}`, {method, credentials: 'omit', headers: {'Authorization': `Bearer ${sessionStorage.getItem('colecto.token') ?? ''}`, 'Content-Type': 'application/json', 'x-colecto-extension': 'host'}, body: body === undefined ? undefined : JSON.stringify(body)});
    const data = await response.json();
    if (!response.ok) throw new Error(data.error ?? 'Service unavailable');
    return data;
  }
  async function load() {
    try { upstream = await call('/upstream'); local = true; address = upstream?.address ?? ''; } catch { /* Remote services have no local controls. */ }
    try { services = await call('/services'); } catch { services = {}; }
    try { await call('/accounts'); reachable = true; } catch { reachable = false; }
  }
  onMount(load);
  async function switchTo(value: typeof upstream) {
    await call('/upstream', 'PUT', value);
    window.location.reload();
  }
  async function act(name: string, action: string, enabled = false) {
    busy = true; error = '';
    try {
      await call(`/services/${name}`, 'POST', {action, enabled, autostart: enabled});
      await load();
      const daemon = services.daemon;
      if (name === 'daemon' && action === 'install' && daemon?.active && daemon.socket && daemon.token_file) {
        // Type=simple can report running before the daemon creates its private token.
        for (let attempt = 0; ; attempt++) {
          try { await switchTo({address: `unix:${daemon.socket}`, token_file: daemon.token_file}); break; }
          catch (e) { if (attempt === 19) throw e; await new Promise(resolve => setTimeout(resolve, 250)); }
        }
      }
    } catch (e) { error = (e as Error).message; }
    finally { busy = false; }
  }
  async function open(event: SubmitEvent) {
    event.preventDefault(); error = '';
    try {
      if (address.startsWith('unix:')) {
        const token = services.daemon?.token_file ?? upstream?.token_file;
        if (!token) throw new Error('The local service must provide its owner token file.');
        await switchTo({address, token_file: token}); return;
      }
      const url = new URL(address);
      if (url.protocol !== 'https:' && !(url.protocol === 'http:' && ['localhost','127.0.0.1','[::1]'].includes(url.hostname))) throw new Error('Use HTTPS, loopback HTTP, or unix:/absolute/socket/path.');
      if (url.username || url.password || url.search || url.hash) throw new Error('Do not include credentials or an access token.');
      window.location.assign(url.origin);
    } catch(e) { error = (e as Error).message; }
  }
</script>
<section class="settings-section connection-settings">
  <header class="section-head"><h2>Connection</h2><p>Accounts and saved drafts belong to the connected service.</p></header>
  <div class="connection-summary"><span class:reachable class="connection-dot"></span><div><strong>Connected to</strong><p class="selectable">{upstream ? `Background service · ${upstream.address}` : local ? "This app’s built-in service" : window.location.origin}</p></div></div>
  {#if Object.values(services).some(s => s.supported)}
    <div class="settings-card"><h3>Background service</h3>
      {#each ['daemon', 'notifications'] as name}
        {@const service = services[name]}
        {#if service?.supported}
          <div class="connection-service"><div><strong>{name === 'daemon' ? 'Mail service' : 'Desktop notifications'}</strong><span class="connection-pill" class:reachable={service.active}>{!service.installed ? 'Not installed' : service.active ? 'Running' : 'Stopped'}</span></div>
            <div class="connection-actions">
              {#if !service.installed}<button disabled={busy} onclick={() => act(name, 'install')}>Install</button>
              {:else}
                <label class="connection-switch"><input type="checkbox" role="switch" checked={service.enabled} disabled={busy} onchange={(event) => act(name, 'autostart', event.currentTarget.checked)} />Start on login</label>
                <button class="icon-button" disabled={busy} aria-label={service.active ? `Stop ${name}` : `Start ${name}`} title={service.active ? 'Stop' : 'Start'} onclick={() => act(name, service.active ? 'stop' : 'start')}><Icon name={service.active ? 'stop' : 'play'} /></button>
                <button class="text-button" disabled={busy} onclick={() => act(name, 'uninstall')}>Uninstall</button>
              {/if}
            </div>
          </div>
        {/if}
      {/each}
      <p>Accounts on the background service are separate from this app’s own until added there.</p>
    </div>
  {/if}
  <div class="settings-card"><h3>Use another server</h3><p>Connect through a local socket, or open another server’s address.</p>
    <form class="inline-form" onsubmit={open}><label class="field">Server address<input type="text" bind:value={address} placeholder="https://mail.example.com or unix:/path" /></label><button type="submit">Open server</button></form>
    {#if upstream}<button class="text-button" onclick={() => switchTo(null).catch(e => error = e.message)}>Use this app’s built-in service</button>{/if}
    <button class="text-button" onclick={() => { setAccessToken(''); window.location.reload(); }}>Forget this session’s access token</button>
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>
