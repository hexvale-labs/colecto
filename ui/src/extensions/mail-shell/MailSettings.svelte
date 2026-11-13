<script lang="ts">
  import type { PrefetchPreferences } from '../../lib/api';
  let { prefetch, onchange }: { prefetch: PrefetchPreferences; onchange: (next: PrefetchPreferences) => void } = $props();
  // A refresh lists the newest 100 messages, so prefetch can reach no further.
  const MAX = 100;
  let timer: ReturnType<typeof setTimeout> | undefined;
  // Dragging the slider updates the number at once and saves once it settles.
  let draft = $state<number | null>(null);
  let shown = $derived(draft ?? prefetch.count);
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
  <div class="settings-group">
    <h3 class="group-label">Download ahead</h3>
    <div class="setting setting-row">
      <div class="setting-text"><h3>Download recent mail ahead of time</h3><p>After each refresh, the newest messages are saved so they open instantly, even offline. This never marks them as read.</p></div>
      <label class="switch"><input type="checkbox" checked={prefetch.enabled} onchange={(event) => onchange({ ...prefetch, enabled: event.currentTarget.checked })} /><span class="sr-only">Download recent mail ahead of time</span></label>
    </div>
    <div class="setting" class:off={!prefetch.enabled}>
      <div class="setting-text range-head"><h3><label for="prefetch-count">Messages to keep ready</label></h3><output for="prefetch-count">{shown}</output></div>
      <p class="range-note">The newest messages in your Inbox. More uses more storage and bandwidth; messages over 10 MB still open on demand.</p>
      <input id="prefetch-count" class="slider" type="range" min="0" max={MAX} step="5" value={shown} disabled={!prefetch.enabled} style:--fill={`${(shown / MAX) * 100}%`} aria-valuetext={`${shown} messages`} oninput={(event) => slide(Number(event.currentTarget.value))} onchange={commit} />
    </div>
  </div>
</section>

<style>
  .range-head { display: flex; align-items: baseline; justify-content: space-between; gap: 16px; max-width: 480px; }
  .range-head output { min-width: 4ch; color: var(--ink-2); font-variant-numeric: tabular-nums; text-align: right; }
  .range-note { max-width: 58ch; color: var(--ink-2); font-size: calc(13.5px * var(--text-scale)); line-height: 1.55; }
  .slider { max-width: 480px; margin-top: 10px; }
  .off { opacity: 0.55; transition: opacity 160ms ease; }
</style>
