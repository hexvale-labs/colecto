<script lang="ts">
  import { onMount } from 'svelte';
  import MailShell from './extensions/MailShell.svelte';
  import Select from './Select.svelte';
  import mark from './assets/logo-square.svg';
  import { extensionHost, hostService, setAccessToken } from './lib/api';
  import type { AccountInfo, Capability, ExtensionState } from './lib/api';
  let authRequired = $state(false);
  let token = $state('');
  function connectDialog(node: HTMLDialogElement) { node.showModal(); return { destroy() { node.close(); } }; }
  async function connect(event: SubmitEvent) { event.preventDefault(); busy = true; error = ''; setAccessToken(token); try { await hostService.accounts(); authRequired = false; token = ''; await reload(); } catch (e) { error = (e as Error).message; } finally { busy = false; } }
  let manifestFile = $state<FileList | undefined>();
  let moduleFile = $state<FileList | undefined>();
  let installNotice = $state('');
  let extensions = $state<ExtensionState[]>([]);
  let accounts = $state<AccountInfo[]>([]);
  let account = $state('demo');
  let managing = $state(false);
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
  async function update(extension: ExtensionState, enabled: boolean, grants: Capability[]) {
    busy = true; error = '';
    try { await extensionHost.update(account, extension.manifest.id, enabled, grants); await reload(); if (extension.manifest.id === 'mail-shell' && !enabled) managing = true; }
    catch (e) { const message = (e as Error).message; await reload(); error ||= message; }
    finally { busy = false; }
  }
  async function install(event: SubmitEvent) {
    event.preventDefault();
    const manifest = manifestFile?.[0]; const module = moduleFile?.[0];
    if (!manifest || !module) return;
    if (manifest.size > 8192 || module.size > 65536) { error = 'Manifest must be at most 8 KiB and Wasm at most 64 KiB'; return; }
    busy = true; error = ''; installNotice = '';
    try {
      await extensionHost.install(await manifest.text(), Array.from(new Uint8Array(await module.arrayBuffer())));
      installNotice = 'Installed disabled with no account access. Review the requested permissions before enabling.';
      await reload();
    } catch (e) { const message = (e as Error).message; await reload(); error ||= message; }
    finally { busy = false; }
  }
  async function remove(extension: ExtensionState) {
    if (!window.confirm(`Remove ${extension.manifest.name} and all its grants? Saved mail will be kept.`)) return;
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
    void extensionHost.list('demo').then(value => extensions = value)
      .catch(e => error = (e as Error).message).finally(() => loading = false);
    return () => window.removeEventListener('colecto-auth-required', requireToken);
  });
</script>

{#snippet extensionSettings()}
  <section class="extension-settings" aria-labelledby="extensions-title">
    <header class="section-head"><h2 id="extensions-title">Extensions</h2><p>Install isolated Wasm packages, turn extensions on or off, and decide which account data each one can use. Changes apply to new requests and are kept across restarts.</p></header>
    <form onsubmit={install}>
      <fieldset disabled={busy || loading}><legend>Install extension</legend>
        <p>Choose a TOML manifest and its Wasm binary. Author names are self-reported, not verified.</p>
        <label for="extension-manifest">TOML manifest</label><input id="extension-manifest" type="file" accept=".toml" required bind:files={manifestFile} />
        <label for="extension-module">Wasm module</label><input id="extension-module" type="file" accept=".wasm" required bind:files={moduleFile} />
        <button class="secondary" type="submit" disabled={!manifestFile?.length || !moduleFile?.length}>Install disabled</button>
      </fieldset>
    </form>
    {#if installNotice}<p role="status">{installNotice}</p>{/if}
    <div class="field-row"><span class="row-label" id="permission-account-label">Permissions for</span><Select label="Permissions for account" value={account} disabled={busy || loading} options={accounts.map(item => ({ value: item.id, label: item.settings?.name || item.settings?.address || 'Demo account' }))} onchange={(next) => { account = next; void reload(); }} /></div>
    {#each extensions as extension (extension.manifest.id)}
      <fieldset disabled={busy || loading}>
        <legend>{extension.manifest.name}</legend>
        <p>{extension.manifest.description}</p><p class="meta">{extension.bundled ? 'Bundled, trusted application code' : 'Third-party, isolated Wasm'}. By {extension.manifest.author} · API {extension.manifest.api_version}</p>
        <label class="switch"><input type="checkbox" checked={extension.enabled} onchange={(event) => void update(extension, event.currentTarget.checked, extension.grants)} /><span>Enabled for all accounts</span></label>
        {#if extension.manifest.capabilities.length}<div class="grants">{#each extension.manifest.capabilities as capability}
          <label class="choice"><input type="checkbox" checked={extension.grants.includes(capability)} onchange={(event) => void update(extension, extension.enabled, event.currentTarget.checked ? [...extension.grants, capability] : extension.grants.filter(c => c !== capability))} /> <code>{capability}</code></label>
        {/each}</div>{/if}
        {#if !extension.bundled}<button class="secondary" onclick={() => void remove(extension)}>Remove extension</button>{/if}
      </fieldset>
    {/each}
  </section>
{/snippet}

{#if authRequired}
  <dialog class="connect-page" aria-label="Connect to mail service" use:connectDialog oncancel={(event) => event.preventDefault()}><form onsubmit={connect}><img src={mark} alt="Colecto" width="44" height="44" /><h1>Connect to your mail</h1><p>Enter this daemon’s owner access token. For a local service, it is in the owner-token file in your Colecto data directory. The token stays in this browser session only; the native app connects automatically.</p><label class="field">Daemon access token<input type="password" bind:value={token} autocomplete="off" required /></label>{#if error}<p role="alert" class="error-box">{error}</p>{/if}<button class="primary" disabled={busy}>Connect</button></form></dialog>
{/if}

{#if managing}
  <section class="settings-page standalone"><button class="secondary" onclick={() => managing = false}>Back to mail</button>{@render extensionSettings()}</section>
{:else if loading}
  <p class="boot" role="status">Loading extensions…</p>
{:else if extensions.find(e => e.manifest.id === 'mail-shell')?.enabled}
  <svelte:boundary>
    <MailShell extensionStates={extensions} {extensionSettings} onsettings={() => void reload()} />
    {#snippet failed(failure, reset)}<div class="error-box" role="alert">Mail workspace failed. Your saved mail remains in Core services.<button class="secondary" onclick={reset}>Retry workspace</button><button class="secondary" onclick={manage}>Extensions</button></div>{/snippet}
  </svelte:boundary>
{:else}
  <p class="error-box boot">Mail workspace disabled. Open Extensions to restore it.</p>
{/if}
{#if error}<p class="error-box" role="alert">{error}<button class="secondary" disabled={busy} onclick={() => void reload()}>Retry</button></p>{/if}
{#if !authRequired && !managing && !extensions.find(e => e.manifest.id === 'mail-shell')?.enabled}<button class="secondary" onclick={manage}>Extensions</button>{/if}
