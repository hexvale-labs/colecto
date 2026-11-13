<script lang="ts" module>
  /** Lets Settings show or hide an extension's pane in the dockable workspace. */
  export interface Workspace { shown: (pane: string) => boolean; toggle: (pane: string, on: boolean) => void }
  const bundledPanes: Record<string, string> = { inbox: 'mailbox', reader: 'reader', accounts: 'accounts' };
</script>
<script lang="ts">
  import { onMount } from 'svelte';
  import MailShell from './extensions/mail-shell/MailShell.svelte';
  import OverlayLayer from './OverlayLayer.svelte';
  import { capabilityLabel } from './lib/capabilities';
  import FilePick from './FilePick.svelte';
  import InstallWarning from './InstallWarning.svelte';
  import Onboarding, { needsOnboarding } from './Onboarding.svelte';
  import mark from './assets/logo-square.svg';
  import { extensionHost, hostService, setAccessToken } from './lib/api';
  import type { AccountInfo, Capability, ExtensionState } from './lib/api';
  let authRequired = $state(false);
  let token = $state('');
  function connectDialog(node: HTMLDialogElement) { node.showModal(); return { destroy() { node.close(); } }; }
  async function connect(event: SubmitEvent) { event.preventDefault(); busy = true; error = ''; setAccessToken(token); try { await hostService.accounts(); authRequired = false; token = ''; await Promise.all([reload(), checkOnboarding()]); } catch (e) { error = (e as Error).message; } finally { busy = false; } }
  let packageFile = $state<FileList | undefined>();
  let warning = $state<File | null>(null);
  let installNotice = $state('');
  let extensions = $state<ExtensionState[]>([]);
  let accounts = $state<AccountInfo[]>([]);
  let account = $state('demo');
  let managing = $state(false);
  let onboarding = $state(false);
  async function checkOnboarding() {
    // Both requests start together; once onboarding is done, the shell need not wait for accounts.
    const list = hostService.accounts();
    list.catch(() => {});
    try {
      const preferences = await hostService.preferences();
      onboarding = !preferences.onboarded && needsOnboarding(await list, preferences);
    } catch { onboarding = false; }
  }
  let loading = $state(true);
  let busy = $state(false);
  let error = $state('');
  async function reload() {
    error = '';
    try {
      accounts = await hostService.accounts();
      if (!accounts.some(a => a.id === account)) account = accounts[0]?.id ?? 'demo';
      extensions = await extensionHost.list(account);
    } catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }
  async function update(extension: ExtensionState, enabled: boolean) {
    busy = true; error = '';
    try { await extensionHost.update(account, extension.manifest.id, enabled); await reload(); if (extension.manifest.id === 'mail-shell' && !enabled) managing = true; }
    catch (e) { const message = (e as Error).message; await reload(); error ||= message; }
    finally { busy = false; }
  }
  let requested = $state<Capability[]>([]);
  let installError = $state('');
  let dropping = $state(false);
  /** Why a file cannot be an extension package, or an empty string. */
  const unusable = (file: File) => !file.name.toLowerCase().endsWith('.colectoext') ? `${file.name} is not an extension package. Choose a .colectoext file.`
    : file.size > 16 * 1024 * 1024 ? 'Extension packages must be at most 16 MiB.' : '';
  async function review(event?: SubmitEvent) {
    event?.preventDefault();
    const file = packageFile?.[0];
    if (!file) return;
    installNotice = '';
    installError = unusable(file);
    if (installError) return;
    try { requested = (await extensionHost.review(new Uint8Array(await file.arrayBuffer()))).capabilities; warning = file; } catch (e) { installError = (e as Error).message; }
  }
  // A package dropped anywhere on the Extensions page goes straight to the same review as Install.
  function dropped(files: FileList) {
    dropping = false;
    if (busy || loading) return;
    installNotice = '';
    installError = files.length > 1 ? 'Drop one extension package at a time.' : unusable(files[0]);
    if (installError) return;
    packageFile = files;
    void review();
  }
  const carriesFiles = (event: DragEvent) => !!event.dataTransfer?.types.includes('Files');
  async function install(file: File) {
    warning = null;
    busy = true; error = ''; installNotice = '';
    try {
      await extensionHost.install(new Uint8Array(await file.arrayBuffer()));
      installNotice = 'Installed and enabled with all requested capabilities.';
      await reload();
    } catch (e) { const message = (e as Error).message; await reload(); error ||= message; }
    finally { busy = false; }
  }
  async function remove(extension: ExtensionState) {
    if (!window.confirm(`Remove ${extension.manifest.name}? Saved mail will be kept.`)) return;
    busy = true; error = '';
    try { await extensionHost.remove(extension.manifest.id); await reload(); }
    catch (e) { const message = (e as Error).message; await reload(); error ||= message; }
    finally { busy = false; }
  }
  function manage() {
    managing = true; void reload();
  }
  onMount(() => {
    const requireToken = () => authRequired = true; window.addEventListener('colecto-auth-required', requireToken);
    void Promise.all([extensionHost.list('demo').then(value => extensions = value).catch(e => error = (e as Error).message), checkOnboarding()])
      .finally(() => loading = false);
    return () => window.removeEventListener('colecto-auth-required', requireToken);
  });
</script>

{#snippet extensionSettings(workspace?: Workspace)}
  <!-- svelte-ignore a11y_no_static_element_interactions (dropping is a shortcut; the file picker is the accessible way in) -->
  <section class="extension-settings" class:dropping aria-labelledby="extensions-title"
    ondragover={(event) => { if (!carriesFiles(event)) return; event.preventDefault(); if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy'; dropping = true; }}
    ondragleave={(event) => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) dropping = false; }}
    ondrop={(event) => { if (!carriesFiles(event)) return; event.preventDefault(); if (event.dataTransfer) dropped(event.dataTransfer.files); }}>
    <header class="section-head"><h2 id="extensions-title">Extensions</h2><p>Enabled extensions have every capability they declare, on every account. Changes apply to new requests and are kept across restarts.</p></header>
    <form class="settings-group" onsubmit={review}>
      <h3 class="group-label">Install an extension</h3>
      <p>Pick or drop a <code>.colectoext</code> package. You review what it asks for before anything is installed. Author names are self-reported, not verified.</p>
      <fieldset class="install-row" disabled={busy || loading}>
        <legend class="sr-only">Install extension</legend>
        <FilePick id="extension-package" label="Extension package" hint=".colectoext, up to 16 MiB" accept=".colectoext" {dropping} ondropfiles={dropped} bind:files={packageFile} />
        <button class="secondary" type="submit" disabled={!packageFile?.length}>Install</button>
      </fieldset>
      {#if installError}<p class="error-box" role="alert">{installError}</p>{/if}
      {#if installNotice}<p class="settings-notice" role="status">{installNotice}</p>{/if}
    </form>
    <div class="settings-group">
      <h3 class="group-label">Installed</h3>
      <div class="settings-list">
        {#each extensions as extension (extension.manifest.id)}
          {@const pane = extension.bundled ? bundledPanes[extension.manifest.id] : extension.manifest.slot === 'panel' && !extension.manifest.capabilities.includes('window.overlay') ? `ext:${extension.manifest.id}` : undefined}
          <fieldset class="extension-card" class:off={!extension.enabled} disabled={busy || loading}>
            <legend class="sr-only">{extension.manifest.name}</legend>
            <div class="extension-top">
              <span class="extension-badge" aria-hidden="true">{extension.manifest.name.slice(0, 1)}</span>
              <div class="extension-title"><strong class="extension-name" aria-hidden="true">{extension.manifest.name}</strong><p class="meta"><span class="tag" class:trusted={extension.bundled}>{extension.bundled ? 'Bundled' : 'Third-party · Wasm'}</span>By {extension.manifest.author} · API {extension.manifest.api_version}</p></div>
              <label class="switch"><input type="checkbox" checked={extension.enabled} onchange={(event) => void update(extension, event.currentTarget.checked)} /><span class="sr-only">Enabled for all accounts</span></label>
            </div>
            <p class="extension-description">{extension.manifest.description}</p>
            {#if workspace && pane && extension.enabled}<label class="choice show-pane"><input type="checkbox" checked={workspace.shown(pane)} onchange={(event) => workspace.toggle(pane, event.currentTarget.checked)} />Show in the workspace</label>{/if}
            {#if extension.manifest.capabilities.length}<div class="grants"><span class="grants-title">Capabilities · all accounts</span><ul>{#each extension.manifest.capabilities as capability}<li>{capabilityLabel(capability)}</li>{/each}</ul></div>{/if}
            {#if !extension.bundled}<button class="text-button danger-text" onclick={() => void remove(extension)}>Remove extension</button>{/if}
          </fieldset>
        {/each}
      </div>
    </div>
  </section>
{/snippet}

{#if authRequired}
  <dialog class="connect-page" aria-label="Connect to mail service" use:connectDialog oncancel={(event) => event.preventDefault()}><form onsubmit={connect}><img src={mark} alt="Colecto" width="44" height="44" /><h1>Connect to your mail</h1><p>Enter this daemon’s owner access token. For a local service, it is in the owner-token file in your Colecto data directory. The token stays in this browser session only; the native app connects automatically.</p><label class="field">Daemon access token<input type="password" bind:value={token} autocomplete="off" required /></label>{#if error}<p role="alert" class="error-box">{error}</p>{/if}<button class="primary" disabled={busy}>Connect</button></form></dialog>
{/if}

{#if managing}
  <section class="settings-page standalone"><button class="secondary" onclick={() => managing = false}>Back to mail</button>{@render extensionSettings()}</section>
{:else if loading}
  <p class="boot" role="status">Loading extensions…</p>
{:else if onboarding}
  <Onboarding ondone={() => onboarding = false} />
{:else if extensions.find(e => e.manifest.id === 'mail-shell')?.enabled}
  <svelte:boundary onerror={(failure) => console.error('Mail workspace failed', failure)}>
    <MailShell extensionStates={extensions} {extensionSettings} onsettings={() => void reload()} />
    {#snippet failed(failure, reset)}<div class="error-box" role="alert">Mail workspace failed. Your saved mail remains in Core services.<button class="secondary" onclick={reset}>Retry workspace</button><button class="secondary" onclick={manage}>Extensions</button></div>{/snippet}
  </svelte:boundary>
{:else}
  <p class="error-box boot">Mail workspace disabled. Open Extensions to restore it.</p>
{/if}
{#if error}<p class="error-box" role="alert">{error}<button class="secondary" disabled={busy} onclick={() => void reload()}>Retry</button></p>{/if}
{#if !authRequired && !managing && !extensions.find(e => e.manifest.id === 'mail-shell')?.enabled}<button class="secondary" onclick={manage}>Extensions</button>{/if}
{#if warning}{@const file = warning}<InstallWarning capabilities={requested} name={file.name} oncancel={() => warning = null} onconfirm={() => void install(file)} />{/if}

<OverlayLayer {extensions} {account} />
