<script lang="ts">
  import type { PrefetchPreferences } from '../../lib/api';
  let { prefetch, onchange }: { prefetch: PrefetchPreferences; onchange: (next: PrefetchPreferences) => void } = $props();
  // A refresh lists the newest 100 messages, so prefetch can reach no further.
  const MAX = 100;
  let count = $derived(prefetch.count);
  let timer: ReturnType<typeof setTimeout> | undefined;
  // Dragging the slider updates the label at once and saves once it settles.
  let draft = $state<number | null>(null);
  let shown = $derived(draft ?? count);
  function slide(value: number) {
    draft = value; clearTimeout(timer);
    timer = setTimeout(commit, 300);
  }
  function commit() {
    clearTimeout(timer);
    if (draft !== null && draft !== prefetch.count) onchange({ ...prefetch, count: draft });
    draft = null;
  }
</script>

<section class="settings-section" aria-labelledby="mail-settings-title">
  <header class="section-head"><h2 id="mail-settings-title">Mail</h2>
  <p>How Colecto keeps your mail ready to read.</p></header>
  <div class="settings-stack">
    <div class="settings-card setting-row">
      <div><h3 id="prefetch-label">Download recent mail ahead of time</h3><p>After each refresh, Colecto saves the newest messages so they open instantly, even offline. Downloading them never marks them as read.</p></div>
      <label class="switch"><input type="checkbox" aria-labelledby="prefetch-label" checked={prefetch.enabled} onchange={(event) => onchange({ ...prefetch, enabled: event.currentTarget.checked })} /></label>
    </div>
    <div class="settings-card range" class:off={!prefetch.enabled}>
      <div class="range-head"><label for="prefetch-count">Messages to keep ready</label><output for="prefetch-count">{shown}</output></div>
      <p>The newest messages in your Inbox. More uses more storage and bandwidth; messages over 10 MB are left to open on demand.</p>
      <input id="prefetch-count" class="slider" type="range" min="0" max={MAX} step="5" value={shown} disabled={!prefetch.enabled} style:--fill={`${(shown / MAX) * 100}%`} aria-valuetext={`${shown} messages`} oninput={(event) => slide(Number(event.currentTarget.value))} onchange={commit} />
      <div class="range-ends" aria-hidden="true"><span>0</span><span>{MAX}</span></div>
    </div>
  </div>
</section>

<style>
  .range-head { display: flex; align-items: baseline; justify-content: space-between; gap: 16px; margin-bottom: 4px; }
  .range-head label { color: var(--ink); font-size: calc(15px * var(--text-scale)); font-weight: 640; letter-spacing: -0.01em; }
  .range > * { max-width: 480px; }
  .range-head output { min-width: 4ch; color: var(--ink-2); font-size: calc(13px * var(--text-scale)); font-weight: 560; font-variant-numeric: tabular-nums; text-align: right; }
  .range > p { margin: 0 0 4px; max-width: 58ch; color: var(--ink-2); font-size: calc(13.5px * var(--text-scale)); line-height: 1.55; }
  .slider { margin-top: 12px; }
  .range-ends { display: flex; justify-content: space-between; margin-top: 6px; color: var(--ink-3); font-size: calc(11.5px * var(--text-scale)); }
  .range.off { opacity: 0.55; transition: opacity 160ms ease; }
</style>
