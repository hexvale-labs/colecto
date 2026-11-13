<script lang="ts">
  import { onMount } from 'svelte';
  import Select from '../../Select.svelte';
  import { extensionHost, subscribeEvents, type ExtensionSetting, type ExtensionSettings, type ExtensionState } from '../../lib/api';
  import { touchSlider } from '../../lib/slider';
  let { manifest }: { manifest: ExtensionState['manifest'] } = $props();
  type Value = ExtensionSettings['values'][string];
  let data = $state<ExtensionSettings>();
  let error = $state('');
  let saved = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let savedTimer: ReturnType<typeof setTimeout> | undefined;
  let pending: Record<string, Value> = {};
  let saving = false;
  let disposed = false;
  const field = (key: string) => `ext-${manifest.id}-${key}`;
  async function load() {
    try { const next = await extensionHost.settings(manifest.id); if (!disposed) { data = next; error = ''; } }
    catch (e) { if (!disposed) error = (e as Error).message; }
  }
  // Writes are batched; a change made while a save is in flight goes out right after it.
  async function flush() {
    clearTimeout(timer); timer = undefined;
    if (saving || !Object.keys(pending).length) return;
    const values = pending; pending = {}; saving = true;
    try {
      await extensionHost.saveSettings(manifest.id, values);
      if (!disposed) { error = ''; saved = true; clearTimeout(savedTimer); savedTimer = setTimeout(() => saved = false, 1600); }
    } catch (e) { if (!disposed) { error = (e as Error).message; void load(); } }
    finally { saving = false; if (Object.keys(pending).length) void flush(); }
  }
  function change(key: string, value: Value, delay = 0) {
    if (!data) return;
    data.values[key] = value; pending[key] = value; saved = false;
    clearTimeout(timer); timer = setTimeout(() => void flush(), delay);
  }
  function reset() { if (data) for (const setting of data.schema) change(setting.key, setting.default); }
  const isDefault = $derived(!!data && data.schema.every(setting => data!.values[setting.key] === setting.default));
  const number = new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 });
  const shown = (setting: ExtensionSetting, value: Value) => `${number.format(Number(value))}${setting.unit ? (setting.unit === '%' ? '%' : ` ${setting.unit}`) : ''}`;
  // How far along its track a slider is, for the filled part of the track.
  const fill = (setting: ExtensionSetting, value: Value) => `${((Number(value) - (setting.min ?? 0)) / ((setting.max ?? 1) - (setting.min ?? 0) || 1)) * 100}%`;
  onMount(() => {
    void load();
    // Another window changed these settings; refresh unless this page has unsaved edits.
    const stop = subscribeEvents((name, event) => { if (name === 'extension.settings' && (event as { id?: string })?.id === manifest.id && !saving && !Object.keys(pending).length) void load(); });
    return () => { stop(); void flush(); disposed = true; clearTimeout(savedTimer); };
  });
</script>

<section class="settings-section" aria-labelledby="extension-settings-title">
  <header class="section-head">
    <h2 id="extension-settings-title">{manifest.name}</h2>
    <p>{manifest.description}</p>
  </header>
  {#if data}
    <div class="settings-group">
      {#each data.schema as setting (setting.key)}
        {@const value = data.values[setting.key]}
        {#if setting.type === 'toggle'}
          <div class="setting setting-row">
            <div class="setting-text"><h3 id={`${field(setting.key)}-label`}>{setting.label}</h3>{#if setting.description}<p>{setting.description}</p>{/if}</div>
            <label class="switch"><input type="checkbox" aria-labelledby={`${field(setting.key)}-label`} checked={value === true} onchange={(event) => change(setting.key, event.currentTarget.checked)} /></label>
          </div>
        {:else if setting.type === 'range'}
          <div class="setting range">
            <div class="setting-text"><div class="range-head"><label for={field(setting.key)}>{setting.label}</label><output for={field(setting.key)}>{shown(setting, value)}</output></div>{#if setting.description}<p>{setting.description}</p>{/if}</div>
            <div><input id={field(setting.key)} class="slider" type="range" use:touchSlider min={setting.min} max={setting.max} step={setting.step ?? 'any'} value={Number(value)} style:--fill={fill(setting, value)} aria-valuetext={shown(setting, value)} oninput={(event) => change(setting.key, Number(event.currentTarget.value), 300)} onchange={() => void flush()} />
            <div class="range-ends" aria-hidden="true"><span>{shown(setting, setting.min ?? 0)}</span><span>{shown(setting, setting.max ?? 0)}</span></div></div>
          </div>
        {:else if setting.type === 'choice' && (setting.options?.length ?? 0) <= 4}
          <fieldset class="setting setting-row">
            <legend class="sr-only">{setting.label}</legend>
            <div class="setting-text"><h3>{setting.label}</h3>{#if setting.description}<p>{setting.description}</p>{/if}</div>
            <div class="segmented">{#each setting.options ?? [] as option (option.value)}<label><input type="radio" name={field(setting.key)} value={option.value} checked={value === option.value} onchange={() => change(setting.key, option.value)} />{option.label}</label>{/each}</div>
          </fieldset>
        {:else if setting.type === 'choice'}
          <div class="setting">
            <div class="setting-text"><h3>{setting.label}</h3>{#if setting.description}<p>{setting.description}</p>{/if}</div>
            <div class="choice-select"><Select label={setting.label} value={String(value)} options={setting.options ?? []} onchange={(next) => change(setting.key, next)} /></div>
          </div>
        {:else}
          <div class="setting">
            <div class="setting-text"><h3><label for={field(setting.key)}>{setting.label}</label></h3>{#if setting.description}<p>{setting.description}</p>{/if}</div>
            <input id={field(setting.key)} class="text-setting" type="text" value={String(value)} oninput={(event) => change(setting.key, event.currentTarget.value, 300)} onblur={() => void flush()} />
          </div>
        {/if}
      {/each}
      <div class="footer">
        <button type="button" class="text-button" disabled={isDefault} onclick={reset}>Reset to defaults</button>
        <span class="save-state" class:visible={saved} role="status">{saved ? 'Saved' : ''}</span>
      </div>
    </div>
  {:else if !error}
    <p class="content-note">Loading settings…</p>
  {/if}
  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
</section>

<style>
  .range-head { display: flex; align-items: baseline; justify-content: space-between; gap: 16px; }
  .range-head label { color: var(--ink); font-size: calc(14px * var(--text-scale)); font-weight: 540; }
  .range > * { max-width: 480px; }
  .range-head output { min-width: 6ch; color: var(--ink-2); font-size: calc(13px * var(--text-scale)); font-variant-numeric: tabular-nums; text-align: right; }
  .range-head + p { margin-top: 3px; }
  .range-ends { display: flex; justify-content: space-between; margin-top: 6px; color: var(--ink-3); font-size: calc(11.5px * var(--text-scale)); }
  .choice-select { max-width: 280px; }
  .text-setting { width: 100%; max-width: 440px; height: 38px; padding: 0 12px; border: 1px solid var(--line); border-radius: 8px; background: var(--sunk); font-size: calc(14px * var(--text-scale)); outline: none; transition: border-color 140ms ease; }
  .text-setting:focus { border-color: var(--mint); }
  .setting-text label { font: inherit; }
  .footer { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .footer .text-button:disabled { color: var(--ink-3); }
  .save-state { color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); opacity: 0; transition: opacity 200ms var(--ease-out); }
  .save-state.visible { opacity: 1; }
</style>
