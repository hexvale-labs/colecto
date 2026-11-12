<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Icon from './Icon.svelte';
  import { hostService, setAccessToken } from './lib/api';
  import type { BackgroundService, Upstream } from './lib/api';
  type Name = 'daemon' | 'notifications';
  const about: Record<Name, { title: string; text: string; icon: 'server' | 'bell' }> = {
    daemon: { title: 'Mail service', text: 'Keeps syncing mail when the app is closed. The app connects to it through a private local socket.', icon: 'server' },
    notifications: { title: 'Desktop notifications', text: 'Shows a desktop notification for new mail from the background service.', icon: 'bell' },
  };
  let services = $state<Partial<Record<Name, BackgroundService>>>({});
  let upstream = $state<Upstream | null>(null);
  // Only this app's own service answers /upstream; a remote daemon refuses it.
  let local = $state(false);
  let loaded = $state(false);
  let busy = $state<Name | 'switch' | null>(null);
  let autostart = $state<Record<Name, boolean>>({ daemon: true, notifications: true });
  let address = $state('');
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
      if (name === 'daemon' && input.action === 'install' && socketAddress) {
        address = socketAddress; await tick(); addressField?.focus();
        notice = 'Mail service installed. Its address is filled in below; open it when you are ready.';
      }
    } catch (e) { error = (e as Error).message; }
    finally { busy = null; }
  }
  async function useUpstream(value: Upstream | null) {
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
    window.location.assign(url.origin);
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
        <span class="service-icon current" aria-hidden="true"><Icon name={upstream ? 'server' : local ? 'monitor' : 'globe'} size={18} /></span>
        <div class="service-text">
          <strong>{upstream ? 'Background mail service' : local ? 'This app' : 'Remote server'}</strong>
          <span class="service-meta"><span class="status-dot on" aria-hidden="true"></span>Connected<span class="sep" aria-hidden="true">·</span><span class="selectable service-detail">{upstream ? upstream.address : window.location.origin}</span></span>
        </div>
        {#if upstream}<button class="secondary" disabled={busy !== null} onclick={() => useUpstream(null)}>Use this app instead</button>{/if}
      </div>
    </section>

    {#if loaded && local && supported}
      <section class="settings-card" aria-labelledby="background-title">
        <h2 id="background-title">Background services</h2>
        <p>Run Colecto as systemd user services, whichever way you installed it.</p>
        <ul class="service-list">
          {#each (['daemon', 'notifications'] as Name[]) as name (name)}
            {@const service = services[name]}
            {#if service?.supported}
              {@const state = status(service)}
              <li class="service-row">
                <span class="service-icon" aria-hidden="true"><Icon name={about[name].icon} size={18} /></span>
                <div class="service-text">
                  <span class="service-title"><strong>{about[name].title}</strong><span class="service-pill {state.tone}">{state.text}</span></span>
                  <span class="service-about">{about[name].text}</span>
                </div>
                <div class="service-controls">
                  <label class="switch" title="Start when you log in"><span>Start on login</span><input type="checkbox" role="switch" aria-label={`Start ${about[name].title} on login`} checked={service.installed ? service.enabled : autostart[name]} disabled={busy !== null} onchange={(event) => { const on = event.currentTarget.checked; if (service.installed) void act(name, { action: 'autostart', enabled: on }); else autostart[name] = on; }} /></label>
                  {#if !service.installed}
                    <button class="primary" disabled={busy !== null} onclick={() => act(name, { action: 'install', autostart: autostart[name] }, `${about[name].title} installed.`)}>{busy === name ? 'Installing…' : 'Install'}</button>
                  {:else}
                    <button class="icon-button" disabled={busy !== null} aria-label={`${service.active ? 'Stop' : 'Start'} ${about[name].title}`} title={service.active ? 'Stop' : 'Start'} onclick={() => act(name, { action: service.active ? 'stop' : 'start' })}><Icon name={service.active ? 'stop' : 'play'} size={16} /></button>
                    <button class="icon-button remove" disabled={busy !== null} aria-label={`Uninstall ${about[name].title}`} title="Uninstall" onclick={() => { if (window.confirm(`Uninstall ${about[name].title}? It stops now and no longer starts on login. Its saved mail is kept.`)) void act(name, { action: 'uninstall' }, `${about[name].title} uninstalled.`); }}><Icon name="trash" size={16} /></button>
                  {/if}
                </div>
              </li>
            {/if}
          {/each}
        </ul>
        {#if services.daemon?.installed && services.daemon.active && !upstream && socketAddress && address !== socketAddress}
          <p class="hint">The mail service keeps its own accounts. <button class="text-button inline" disabled={busy !== null} onclick={() => { address = socketAddress; void tick().then(() => addressField?.focus()); }}>Fill in its address</button></p>
        {/if}
      </section>
    {/if}

    <section class="settings-card" aria-labelledby="other-title">
      <h2 id="other-title">Use another server</h2>
      <p>Open a daemon’s HTTPS address; your credentials stay on that server. {local ? 'A local mail service is reached through its socket, as unix:/path/to/mail.sock.' : ''}</p>
      <form class="address-form" onsubmit={open}>
        <label class="field"><span class="sr-only">Server address</span><input bind:this={addressField} bind:value={address} required autocomplete="url" autocapitalize="none" spellcheck="false" placeholder={local ? 'https://mail.example.com or unix:/…' : 'https://mail.example.com'} /></label>
        <button class="secondary" disabled={busy !== null}>Open</button>
      </form>
      <div class="button-row"><button type="button" class="text-button" onclick={() => { setAccessToken(''); window.location.reload(); }}>Forget this session’s access token</button></div>
    </section>
  </div>
</section>
