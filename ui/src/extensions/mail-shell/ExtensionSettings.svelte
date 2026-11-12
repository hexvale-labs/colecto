<script lang="ts">
  import { onMount } from 'svelte';
  import Select from '../../Select.svelte';
  import { extensionHost, subscribeEvents, type ExtensionSettings } from '../../lib/api';
  let { id }: { id: string } = $props();
  let data = $state<ExtensionSettings>();
  let status = $state('');
  let timer: ReturnType<typeof setTimeout>;
  let pending: ExtensionSettings['values'] = {};
  let saving = false;
  let disposed = false;
  async function flush() {
    if (saving || !Object.keys(pending).length) return;
    const values = pending; pending = {}; saving = true;
    try { await extensionHost.saveSettings(id, values); if (!disposed) status = 'Saved'; }
    catch (e) { if (!disposed) status = (e as Error).message; }
    finally { saving = false; if (Object.keys(pending).length) void flush(); }
  }
  function change(key: string, value: boolean | number | string, delay = 0) {
    if (!data) return;
    data.values[key] = value; pending[key] = value; status = '';
    clearTimeout(timer); timer = setTimeout(() => void flush(), delay);
  }
  onMount(() => {
    void extensionHost.settings(id).then(next => { if (!disposed) data = next; }).catch(e => status = e.message);
    const stop = subscribeEvents((name, event) => { if (name === 'extension.settings' && (event as {id?: string})?.id === id && !saving && !Object.keys(pending).length) void extensionHost.settings(id).then(next => { if (!disposed) data = next; }); });
    return () => { stop(); disposed = true; clearTimeout(timer); void flush(); };
  });
</script>
<h2>Extension settings</h2>
{#if data}
  {#each data.schema as setting}
    <section>
      {#if setting.type === "choice"}<span class="label">{setting.label}</span>{:else}<label for={`setting-${setting.key}`}>{setting.label}</label>{/if}
      {#if setting.description}<p>{setting.description}</p>{/if}
      {#if setting.type === 'toggle'}
        <input id={`setting-${setting.key}`} type="checkbox" role="switch" checked={Boolean(data.values[setting.key])} onchange={e => change(setting.key, e.currentTarget.checked)} />
      {:else if setting.type === 'range'}
        <input id={`setting-${setting.key}`} type="range" min={setting.min} max={setting.max} step={setting.step ?? 'any'} value={Number(data.values[setting.key])} aria-valuetext={`${data.values[setting.key]} ${setting.unit ?? ''}`} oninput={e => change(setting.key, Number(e.currentTarget.value), 300)} />
        <output for={`setting-${setting.key}`}>{data.values[setting.key]} {setting.unit ?? ''}</output>
      {:else if setting.type === 'choice'}
        {#if (setting.options?.length ?? 0) <= 4}
          <div role="group" aria-label={setting.label} class="segments">{#each setting.options ?? [] as option}<button class:active={data.values[setting.key] === option.value} aria-pressed={data.values[setting.key] === option.value} onclick={() => change(setting.key, option.value)}>{option.label}</button>{/each}</div>
        {:else}<Select label={setting.label} value={String(data.values[setting.key])} options={setting.options ?? []} onchange={value => change(setting.key, value)} />{/if}
      {:else}<input id={`setting-${setting.key}`} type="text" value={String(data.values[setting.key])} oninput={e => change(setting.key, e.currentTarget.value, 300)} />{/if}
    </section>
  {/each}
  <button class="reset" onclick={() => { for (const setting of data!.schema) change(setting.key, setting.default); }}>Reset to defaults</button>
{/if}
<p role="status">{status}</p>
<style>
  section { padding: 18px 0; border-bottom: 1px solid var(--line-soft); }
  label, .label { display: block; font-weight: 600; margin-bottom: 8px; }
  section { overflow-wrap: anywhere; min-width: 0; }
  input[type="text"] { max-width: 100%; }
  p { color: var(--ink-2); }
  input[type="range"] { width: min(300px, 80%); vertical-align: middle; }
  output { margin-left: 12px; font-variant-numeric: tabular-nums; }
  .segments { display: flex; flex-wrap: wrap; gap: 4px; }
  .active { background: var(--select); color: var(--mint); }
  .reset { margin-top: 20px; background: none; color: var(--ink-2); }
</style>
