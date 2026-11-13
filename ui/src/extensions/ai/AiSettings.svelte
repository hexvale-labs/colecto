<script lang="ts">
  import { onMount } from 'svelte';
  import { aiHost, aiService, type AiSettings, type Effort, type LabelRule, type SettingsView } from './api';
  import { setEnabled } from './state.svelte';
  import KeyField from './KeyField.svelte';
  import LabelEditor from './LabelEditor.svelte';
  let { account, accountName, extensionOn = true }: { account: string; accountName: string; extensionOn?: boolean } = $props();
  let view = $state<SettingsView>();
  let labels = $state<string[]>([]);
  let error = $state('');
  let editing = $state<string | null>(null);
  let tested = $state<Record<'text' | 'decision', { busy: boolean; note: string; ok: boolean }>>({ text: { busy: false, note: '', ok: false }, decision: { busy: false, note: '', ok: false } });
  let settings = $derived(view?.settings);
  let on = $derived(!!settings?.enabled);
  const efforts: { value: Effort | null; label: string }[] = [{ value: null, label: 'Default' }, { value: 'none', label: 'None' }, { value: 'low', label: 'Low' }, { value: 'medium', label: 'Medium' }, { value: 'high', label: 'High' }];
  const empty: LabelRule = { enabled: false, model: false, description: '', rules: [] };
  let rules = $derived(settings?.labels[account] ?? {});

  async function load() {
    try { view = await aiHost.settings(); setEnabled(view.settings.enabled); error = ''; }
    catch (e) { error = (e as Error).message; }
    try { labels = Object.keys(await aiService(account).labels()); } catch { labels = []; }
  }
  // Settings save as they change. A failed save reloads what the Host kept.
  async function save(change: (draft: AiSettings) => void) {
    if (!view) return;
    const next = $state.snapshot(view.settings) as AiSettings;
    change(next);
    view.settings = next;
    try { view = await aiHost.save(next); setEnabled(view.settings.enabled); error = ''; }
    catch (e) { error = (e as Error).message; void load(); }
  }
  function saveRule(name: string, rule: LabelRule) {
    return save(draft => { (draft.labels[account] ??= {})[name] = rule; });
  }
  async function test(kind: 'text' | 'decision') {
    tested[kind] = { busy: true, note: '', ok: false };
    try { await aiHost.test(kind); tested[kind] = { busy: false, note: 'Connected.', ok: true }; }
    catch (e) { tested[kind] = { busy: false, note: (e as Error).message, ok: false }; }
  }
  function describe(rule: LabelRule | undefined) {
    if (!rule?.enabled) return 'Off';
    const parts = [];
    if (rule.rules.length) parts.push(`${rule.rules.length} ${rule.rules.length === 1 ? 'rule' : 'rules'}`);
    if (rule.model) parts.push('model on');
    return parts.join(' · ') || 'Nothing set up yet';
  }
  const percent = (value: number) => `${Math.round(value * 100)}%`;
  onMount(() => { void load(); });
</script>

<section class="settings-section ai-settings" aria-labelledby="ai-title">
  <header class="section-head"><h2 id="ai-title">AI</h2>
    <p>Optional help from AI services you choose. While this is off, Colecto never contacts them. When it is on, summaries and replies send the message you are reading, and automatic labels send newly arrived mail, to the service you set below.</p></header>
  {#if !extensionOn}<p class="content-note">The AI extension is turned off in Settings → Extensions, so the reader shows no AI actions.</p>{/if}
  {#if error}<p class="error-box" role="alert">{error}</p>{/if}
  {#if settings && view}
    <div class="settings-group">
      <div class="setting setting-row">
        <div class="setting-text"><h3>Use AI features</h3><p>Turns on summaries, drafted replies and automatic labels. Off by default.</p></div>
        <label class="switch"><input type="checkbox" checked={on} onchange={(event) => { const value = event.currentTarget.checked; void save(draft => { draft.enabled = value; }); }} /><span class="sr-only">Use AI features</span></label>
      </div>
    </div>
    <fieldset class="ai-options" class:inactive={!on} disabled={!on}>
      <legend class="sr-only">AI services</legend>
      <div class="settings-group">
        <h3 class="group-label">Text model</h3>
        <p>Writes summaries and replies. Works with any service that offers an OpenAI-compatible chat completions API, including ones on your own computer.</p>
        <div class="form-grid">
          <label class="field wide-field">Base URL<input type="url" placeholder="https://api.openai.com/v1" value={settings.text.base_url} onchange={(event) => { const value = event.currentTarget.value.trim(); void save(draft => { draft.text.base_url = value; }); }} /></label>
          <label class="field">Model<input placeholder="Model name" value={settings.text.model} autocomplete="off" spellcheck="false" onchange={(event) => { const value = event.currentTarget.value.trim(); void save(draft => { draft.text.model = value; }); }} /></label>
          <label class="field">Max tokens<input type="number" min="16" max="32000" step="1" value={settings.text.max_tokens} onchange={(event) => { const value = Math.round(Number(event.currentTarget.value)); void save(draft => { draft.text.max_tokens = value; }); }} /></label>
        </div>
        <KeyField label="API key" saved={view.keys.text} onsave={async (key) => { view = await aiHost.key('text', key); }} />
        <fieldset class="segmented-field effort">
          <legend>Thinking effort</legend>
          <div class="segmented">{#each efforts as option (option.label)}<label><input type="radio" name="ai-effort" checked={settings.text.effort === option.value} onchange={() => void save(draft => { draft.text.effort = option.value; })} />{option.label}</label>{/each}</div>
          <p class="hint">Only sent when not Default. Not every model supports it.</p>
        </fieldset>
        <div class="button-row"><button type="button" class="secondary" disabled={tested.text.busy} onclick={() => void test('text')}>{tested.text.busy ? 'Testing…' : 'Test connection'}</button>{#if tested.text.note}<span class={tested.text.ok ? 'test-ok' : 'test-failed'} role="status">{tested.text.note}</span>{/if}</div>
      </div>
      <div class="settings-group">
        <h3 class="group-label">Decision model</h3>
        <p>Answers yes-or-no questions for automatic labels, such as “does this belong under Receipts?”. Uses the OpenRouter Decisions API.</p>
        <div class="form-grid">
          <label class="field">Base URL<input type="url" value={settings.decision.base_url} onchange={(event) => { const value = event.currentTarget.value.trim(); void save(draft => { draft.decision.base_url = value; }); }} /></label>
          <label class="field">Model<input value={settings.decision.model} autocomplete="off" spellcheck="false" onchange={(event) => { const value = event.currentTarget.value.trim(); void save(draft => { draft.decision.model = value; }); }} /></label>
        </div>
        <KeyField label="API key" saved={view.keys.decision} onsave={async (key) => { view = await aiHost.key('decision', key); }} />
        <div class="field confidence">
          <span class="range-head"><label for="ai-threshold">Confidence needed</label><output for="ai-threshold">{percent(settings.decision.threshold)}</output></span>
          <input id="ai-threshold" class="slider" type="range" min="0.5" max="0.99" step="0.01" value={settings.decision.threshold} style:--fill={`${((settings.decision.threshold - 0.5) / 0.49) * 100}%`} aria-valuetext={percent(settings.decision.threshold)}
            oninput={(event) => { if (view) view.settings.decision.threshold = Number(event.currentTarget.value); }}
            onchange={(event) => { const value = Number(event.currentTarget.value); void save(draft => { draft.decision.threshold = value; }); }} />
          <span class="range-ends" aria-hidden="true"><span>More labels</span><span>Fewer, surer labels</span></span>
        </div>
        <div class="button-row"><button type="button" class="secondary" disabled={tested.decision.busy} onclick={() => void test('decision')}>{tested.decision.busy ? 'Testing…' : 'Test connection'}</button>{#if tested.decision.note}<span class={tested.decision.ok ? 'test-ok' : 'test-failed'} role="status">{tested.decision.note}</span>{/if}</div>
      </div>
      <div class="settings-group">
        <h3 class="group-label">Automatic labels</h3>
        <div class="setting setting-row">
          <div class="setting-text"><h3>Label new mail automatically</h3><p>Sorts newly arrived mail into your labels. Rules match text and need no AI service; a label can also ask the decision model. A message is only checked once, so a label you remove stays removed.</p></div>
          <label class="switch"><input type="checkbox" checked={settings.autolabel} onchange={(event) => { const value = event.currentTarget.checked; void save(draft => { draft.autolabel = value; }); }} /><span class="sr-only">Label new mail automatically</span></label>
        </div>
        <div class="label-list" class:inactive={!settings.autolabel}>
          <h4>Labels in {accountName}</h4>
          {#if !labels.length}<p class="empty-note">No labels yet. Create one from a message or with “Create a local label” in the command palette.</p>{/if}
          <ul>
            {#each labels as name (name)}
              {@const rule = rules[name]}
              <li class="label-row">
                <button type="button" class="label-open" disabled={!settings.autolabel} onclick={() => editing = name}><span class="label-name">{name}</span><small>{describe(rule)}</small></button>
                <label class="switch"><input type="checkbox" checked={!!rule?.enabled} disabled={!settings.autolabel} onchange={(event) => { const value = event.currentTarget.checked; void saveRule(name, { ...(rule ?? empty), enabled: value }); }} /><span class="sr-only">Label {name} automatically</span></label>
              </li>
            {/each}
          </ul>
        </div>
      </div>
    </fieldset>
  {:else if !error}<p class="content-note" role="status">Loading…</p>{/if}
</section>
{#if editing !== null}
  {@const name = editing}
  <LabelEditor {name} {account} rule={rules[name] ?? { ...empty, enabled: true }} modelReady={!!settings?.decision.model} onclose={() => editing = null} onsave={async (rule) => { await saveRule(name, rule); editing = null; }} />
{/if}

<style>
  .ai-options { min-width: 0; margin: 40px 0 0; padding: 0; border: 0; transition: opacity 160ms ease; }
  .inactive { opacity: 0.55; }
  .ai-options.inactive .inactive { opacity: 1; }
  .effort .segmented { flex-wrap: wrap; }
  .effort .hint { margin-top: 6px; color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); }
  .confidence { max-width: 440px; }
  /* A range input, not a text box: drop the field's input chrome. */
  .confidence input.slider { height: 20px; padding: 0; border: 0; background: transparent; }
  .range-head { display: flex; justify-content: space-between; }
  .range-head output { color: var(--ink); font-variant-numeric: tabular-nums; }
  .range-ends { display: flex; justify-content: space-between; color: var(--ink-3); font-size: calc(12px * var(--text-scale)); font-weight: 450; }
  .test-ok { color: var(--ink-2); font-size: calc(13px * var(--text-scale)); }
  .test-failed { color: var(--danger); font-size: calc(13px * var(--text-scale)); user-select: text; }
  .label-list { transition: opacity 160ms ease; }
  .label-list h4 { margin-bottom: 4px; color: var(--ink-2); font-size: calc(12.5px * var(--text-scale)); font-weight: 560; }
  .label-row { display: flex; align-items: center; gap: 12px; border-bottom: 1px solid var(--line-soft); }
  .label-row:last-child { border-bottom: 0; }
  .label-open { display: grid; flex: 1; min-width: 0; padding: 10px 8px; margin-left: -8px; border-radius: 8px; text-align: left; transition: background-color 120ms ease; }
  .label-open:hover:not(:disabled) { background: var(--raise); }
  .label-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink); font-size: calc(14px * var(--text-scale)); font-weight: 560; }
  .label-open small { color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); }
  @media (prefers-reduced-motion: reduce) { .ai-options, .label-list { transition: none; } }
</style>
