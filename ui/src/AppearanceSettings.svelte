<script lang="ts">
  import Icon from './Icon.svelte';
  let { value, onchange }: { value: string | null; onchange: (accent: string) => void } = $props();
  const DEFAULT_ACCENT = '#a2d6b1';
  // Light, low-chroma tones that stay readable as text and as a fill on the dark ground.
  const presets = [
    { name: 'Mint', color: DEFAULT_ACCENT }, { name: 'Sky', color: '#8ec5f0' }, { name: 'Periwinkle', color: '#a9b4f5' },
    { name: 'Lilac', color: '#c9a8f0' }, { name: 'Rose', color: '#f0a8c0' }, { name: 'Coral', color: '#f2b09a' },
    { name: 'Amber', color: '#ecc98a' }, { name: 'Lime', color: '#c4dd8f' },
  ];
  let current = $derived(value ?? DEFAULT_ACCENT);
  let custom = $derived(!presets.some(preset => preset.color === current));
  let picking = $state<string | null>(null);
  // The native picker reports every drag step; apply them live and save once it settles.
  let timer: ReturnType<typeof setTimeout> | undefined;
  function pick(color: string) {
    picking = color; document.documentElement.style.setProperty('--mint', color);
    clearTimeout(timer); timer = setTimeout(() => { onchange(color); picking = null; }, 250);
  }
</script>

<section class="settings-section" aria-labelledby="appearance-title">
  <header class="section-head"><h2 id="appearance-title">Appearance</h2>
  <p>Colecto stays dark. Pick the accent used for selection, focus and primary buttons.</p></header>
  <div class="settings-card">
    <h3 id="accent-title">Accent colour</h3>
    <div class="swatches" role="radiogroup" aria-labelledby="accent-title">
      {#each presets as preset (preset.color)}
        <label class="swatch" style:--swatch={preset.color} title={preset.name}>
          <input type="radio" name="accent" value={preset.color} checked={current === preset.color} onchange={() => onchange(preset.color === DEFAULT_ACCENT ? '' : preset.color)} />
          <span class="sr-only">{preset.name}</span><Icon name="check" size={14} />
        </label>
      {/each}
      <label class="swatch custom" class:chosen={custom} style:--swatch={custom ? picking ?? current : 'transparent'} title="Custom colour">
        <input type="color" value={picking ?? current} oninput={(event) => pick(event.currentTarget.value)} />
        <span class="sr-only">Custom colour</span><Icon name="plus" size={14} />
      </label>
    </div>
    {#if value}<div class="button-row"><button type="button" class="text-button" onclick={() => onchange('')}>Restore the default mint</button></div>{/if}
  </div>
</section>
