<script lang="ts">
  import Icon from './Icon.svelte';
  import Select from './Select.svelte';
  import { bundledFonts, canListAllFonts, familyOf, listAllFonts, loadBundled, probeSystemFonts, textSizes } from './lib/typography';
  let { value, font, textSize, onchange, ontypography }: {
    value: string | null; font: string; textSize: number;
    onchange: (accent: string) => void; ontypography: (change: { font?: string; font_size?: number }) => void;
  } = $props();
  let systemFonts = $state(probeSystemFonts());
  let fontNote = $state('');
  let fontOptions = $derived([
    ...bundledFonts.map(item => ({ value: item.id, label: item.name, family: item.family, group: 'Bundled' })),
    // A saved system font stays listed even on a device that cannot detect it.
    ...[...new Set([...systemFonts, ...(bundledFonts.some(item => item.id === font) ? [] : [font])])]
      .map(family => ({ value: family, label: family, family, group: 'System' })),
  ]);
  async function listAll() {
    fontNote = '';
    try { systemFonts = await listAllFonts(); fontNote = `${systemFonts.length} installed fonts listed.`; }
    catch { fontNote = 'Colecto was not allowed to list your fonts. Showing common ones it found.'; }
  }
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
  <p>Colecto stays dark. Choose how text looks and the accent used for selection, focus and primary buttons.</p></header>
  <div class="settings-card">
    <h3 id="font-title">Font</h3>
    <p>Bundled fonts work offline on every device. System fonts must be installed where you use Colecto.</p>
    <div class="font-picker">
      <Select label="Interface font" value={font} options={fontOptions} onopen={() => void loadBundled().catch(() => {})} onchange={(next) => ontypography({ font: next })}>
        {#snippet trigger(current)}<span class="font-current" style:font-family={familyOf(current?.value ?? font)}>{current?.label ?? font}</span>{/snippet}
      </Select>
      {#if canListAllFonts()}<button type="button" class="text-button" onclick={listAll}>List all installed fonts</button>{/if}
    </div>
    {#if fontNote}<p class="hint" role="status">{fontNote}</p>{/if}
  </div>
  <div class="settings-card">
    <fieldset class="segmented-field">
      <legend><h3>Text size</h3></legend>
      <p>Scales text across the app. Spacing stays compact.</p>
      <div class="segmented text-sizes">
        {#each textSizes as item (item.size)}
          <label title={`${item.name}, ${item.size}px`}><input type="radio" name="text-size" checked={textSize === item.size} onchange={() => ontypography({ font_size: item.size })} /><span class="sample" style:font-size={`${item.size + 1}px`} aria-hidden="true">Aa</span><span class="sr-only">{item.name}</span></label>
        {/each}
      </div>
    </fieldset>
  </div>
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
