<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Icon from './Icon.svelte';
  import { hostService, setAccessToken } from './lib/api';
  import type { BackgroundService, Upstream, UpstreamInput } from './lib/api';
  type Name = 'daemon';
  let services = $state<Partial<Record<Name, BackgroundService>>>({});
  // While this app uses a remote server, the background service only relays its notifications.
  const about = $derived(services.daemon?.relay
    ? { title: 'Notifications', text: 'Shows desktop notifications for new mail from your server while Colecto is closed. Your server keeps syncing the mail.', icon: 'bell' as const }
    : { title: 'Mail service', text: 'Keeps syncing mail and shows desktop notifications for new mail while Colecto is closed. The app connects to it through a private local socket.', icon: 'server' as const });
  let upstream = $state<Upstream | null>(null);
  // Only this app's own service answers /upstream; a remote daemon refuses it.
  let local = $state(false);
  let loaded = $state(false);
  let busy = $state<Name | 'switch' | null>(null);
  let autostart = $state(true);
  let address = $state('');
  let token = $state('');
  // The desktop app reaches a remote daemon through its own Host, which keeps mail ready locally.
  const remote = $derived(local && /^https?:/i.test(address.trim()));
  const viaSocket = $derived(!!upstream?.address.startsWith('unix:'));
  let addressField = $state<HTMLInputElement>();
  let error = $state('');
  let notice = $state('');
  const supported = $derived(Object.values(services).some(service => service?.supported));
  const socketAddress = $derived(services.daemon?.socket ? `unix:${services.daemon.socket}` : '');

  async function load() {
    try { upstream = await hostService.upstream(); local = true; } catch { local = false; }
    if (local) { try { services = await hostService.services(); } catch { services = {}; } }
    loaded = true;
  }
  onMount(() => void load());

  function status(service: BackgroundService) {
    if (!service.installed) return { text: 'Not installed', tone: 'off' };
    return service.active ? { text: 'Running', tone: 'on' } : { text: 'Stopped', tone: 'off' };
  }
  async function act(name: Name, input: Parameters<typeof hostService.serviceAction>[1], done = '') {
    busy = name; error = ''; notice = '';
    try {
      await hostService.serviceAction(name, input);
      await load();
      notice = done;
      // A fresh mail service is ready to use: offer its socket instead of switching on the spot,
      // because its accounts are separate from this app's.
      if (input.action === 'install' && !services.daemon?.relay && socketAddress) {
        address = socketAddress; await tick(); addressField?.focus();
        notice = 'Mail service installed. Its address is filled in below; open it when you are ready.';
      }
    } catch (e) { error = (e as Error).message; }
    finally { busy = null; }
  }
  async function useUpstream(value: UpstreamInput | null) {
    busy = 'switch'; error = '';
    try { await hostService.setUpstream(value); window.location.reload(); }
    catch (e) { error = (e as Error).message; busy = null; }
  }
  function open(event: SubmitEvent) {
    event.preventDefault(); error = '';
    const value = address.trim();
    if (value.startsWith('unix:')) {
      const token = services.daemon?.token_file ?? upstream?.token_file;
      if (!local) { error = 'Only the app on this computer can connect through a socket.'; return; }
      if (!token) { error = 'Install the mail service first; Colecto needs its access token.'; return; }
      void useUpstream({ address: value, token_file: token }); return;
    }
    let url: URL;
    try { url = new URL(value); } catch { error = 'Enter an https:// address, or unix: followed by a socket path.'; return; }
    const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname);
    if (url.protocol !== 'https:' && !(url.protocol === 'http:' && loopback)) { error = 'Use HTTPS. Plain HTTP works only on this computer (localhost).'; return; }
    if (url.username || url.password || url.search || url.hash) { error = 'Leave credentials and tokens out of the address.'; return; }
    if (!local) { window.location.assign(url.origin); return; }
    // Re-saving the server already in use keeps its stored token.
    const secret = token.trim();
    if (!secret && upstream?.address !== url.origin) { error = 'Enter the server’s access token.'; return; }
    void useUpstream(secret ? { address: url.origin, token: secret } : { address: url.origin });
  }
</script>

<section class="settings-section" aria-labelledby="connection-title">
  <header class="section-head"><h2 id="connection-title">Connection</h2><p>Accounts and drafts belong to the mail service you are connected to.</p></header>
  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  {#if notice}<p class="settings-notice" role="status">{notice}</p>{/if}
  <div class="settings-stack">
    <section class="settings-card" aria-labelledby="current-title">
      <h2 id="current-title" class="sr-only">Current service</h2>
      <div class="service-row">
        <span class="service-icon current" aria-hidden="true"><Icon name={upstream ? (viaSocket ? 'server' : 'globe') : local ? 'monitor' : 'globe'} size={18} /></span>
        <div class="service-text">
          <strong>{upstream ? (viaSocket ? 'Background mail service' : 'Remote server') : local ? 'This app' : 'Remote server'}</strong>
          <span class="service-meta"><span class="status-dot on" aria-hidden="true"></span>Connected<span class="sep" aria-hidden="true">·</span><span class="selectable service-detail">{upstream ? upstream.address : window.location.origin}</span></span>
        </div>
        {#if upstream}<button class="secondary" disabled={busy !== null} onclick={() => useUpstream(null)}>Use this app instead</button>{/if}
      </div>
    </section>

    {#if loaded && local && supported}
      <section class="settings-card" aria-labelledby="background-title">
        <h2 id="background-title">Background service</h2>
        <p>Run Colecto as a systemd user service, whichever way you installed it.</p>
        {#if services.daemon?.supported}
          {@const service = services.daemon}
          {@const state = status(service)}
          <div class="service-list service-row">
            <span class="service-icon" aria-hidden="true"><Icon name={about.icon} size={18} /></span>
            <div class="service-text">
              <span class="service-title"><strong>{about.title}</strong><span class="service-pill {state.tone}">{state.text}</span>{#if service.installed && service.version}<span class="service-version">v{service.version}</span>{/if}</span>
              <span class="service-about">{about.text}</span>
            </div>
            <div class="service-controls">
              <label class="switch" title="Start when you log in"><span>Start on login</span><input type="checkbox" role="switch" aria-label={`Start ${about.title} on login`} checked={service.installed ? service.enabled : autostart} disabled={busy !== null} onchange={(event) => { const on = event.currentTarget.checked; if (service.installed) void act('daemon', { action: 'autostart', enabled: on }); else autostart = on; }} /></label>
              {#if !service.installed}
                <button class="primary" disabled={busy !== null} onclick={() => act('daemon', { action: 'install', autostart }, `${about.title} installed.`)}>{busy === 'daemon' ? 'Installing…' : 'Install'}</button>
              {:else}
                {#if service.update_to}
                  {@const target = service.update_to}
                  <button class="secondary" disabled={busy !== null} aria-label={`Update ${about.title} to version ${target}`} title={`Rewrite the service for Colecto ${target} and this connection${service.active ? ', then restart it' : ''}`} onclick={() => act('daemon', { action: 'update' }, `${about.title} updated to version ${target}${service.active ? ' and restarted' : ''}.`)}>{busy === 'daemon' ? 'Updating…' : 'Update'}</button>
                {/if}
                <button class="icon-button" disabled={busy !== null} aria-label={`${service.active ? 'Stop' : 'Start'} ${about.title}`} title={service.active ? 'Stop' : 'Start'} onclick={() => act('daemon', { action: service.active ? 'stop' : 'start' })}><Icon name={service.active ? 'stop' : 'play'} size={16} /></button>
                <button class="icon-button remove" disabled={busy !== null} aria-label={`Uninstall ${about.title}`} title="Uninstall" onclick={() => { if (window.confirm(`Uninstall ${about.title}? It stops now and no longer starts on login. Its saved mail is kept.`)) void act('daemon', { action: 'uninstall' }, `${about.title} uninstalled.`); }}><Icon name="trash" size={16} /></button>
              {/if}
            </div>
          </div>
        {/if}
        {#if services.daemon?.installed && services.daemon.active && !upstream && socketAddress && address !== socketAddress}
          <p class="hint">The mail service keeps its own accounts. <button class="text-button inline" disabled={busy !== null} onclick={() => { address = socketAddress; void tick().then(() => addressField?.focus()); }}>Fill in its address</button></p>
        {/if}
      </section>
    {/if}

    <section class="settings-card" aria-labelledby="other-title">
      <h2 id="other-title">Use another server</h2>
      <p>{local ? 'Connect to a daemon by its HTTPS address and access token. Mail you open is kept on this computer too, so it stays quick on a slow connection. A local mail service is reached through its socket, as unix:/path/to/mail.sock.' : 'Open a daemon’s HTTPS address; your credentials stay on that server.'}</p>
      <form class="address-form" onsubmit={open}>
        <label class="field"><span class="sr-only">Server address</span><input bind:this={addressField} bind:value={address} required autocomplete="url" autocapitalize="none" spellcheck="false" placeholder={local ? 'https://mail.example.com or unix:/…' : 'https://mail.example.com'} /></label>
        {#if remote}<label class="field"><span class="sr-only">Access token</span><input type="password" bind:value={token} autocomplete="off" spellcheck="false" placeholder={upstream?.address === address.trim().replace(/\/$/, '') ? 'Access token (unchanged)' : 'Access token'} /></label>{/if}
        <button class="secondary" disabled={busy !== null}>{remote ? 'Connect' : 'Open'}</button>
      </form>
      <div class="button-row"><button type="button" class="text-button" onclick={() => { setAccessToken(''); window.location.reload(); }}>Forget this session’s access token</button></div>
    </section>
  </div>
</section>
