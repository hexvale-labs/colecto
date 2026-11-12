<script lang="ts">
  import { onMount } from 'svelte';
  import { hostService } from './lib/api';
  import { applyTypography, previewBundledFonts } from './lib/typography';
  import Select from './Select.svelte';
  import { bundledFonts, installedFonts } from './lib/typography';
  import Icon from './Icon.svelte';
  let { value, onchange, font = 'mona', fontSize = 14 }: { value: string | null; onchange: (accent: string) => void; font?: string; fontSize?: number } = $props();
  let typographyError = $state('');
  onMount(() => { void hostService.preferences().then(p => { font = p.font ?? 'mona'; fontSize = p.font_size ?? 14; }).catch(e => typographyError = e.message); });
  async function ontypography(change: { font?: string; font_size?: number }) {
    try { const p = await hostService.savePreferences(change); font = p.font ?? 'mona'; fontSize = p.font_size ?? 14; await applyTypography(font, fontSize); typographyError = ''; }
    catch (e) { typographyError = (e as Error).message; }
  }
  let systemFonts = $state<string[]>(installedFonts());
  let fontNote = $state('');
  let options = $derived([
    ...bundledFonts.map(item => ({ ...item, group: 'BUNDLED' })),
    ...Array.from(new Set([...systemFonts, ...(!bundledFonts.some(item => item.value === font) ? [font] : [])])).map(family => ({ value: family, label: family, family, group: 'SYSTEM' })),
  ]);
  async function discover() {
    const local = window as unknown as { queryLocalFonts?: () => Promise<{ family: string }[]> };
    try {
      if (local.queryLocalFonts) systemFonts = [...new Set((await local.queryLocalFonts()).map(item => item.family))].filter(family => family.length <= 64).sort();
      else fontNote = 'Showing installed fonts detected on this device.';
    } catch { fontNote = 'Font access was declined. Showing detected system fonts.'; }
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
  <p>Colecto stays dark. Pick the accent used for selection, focus and primary buttons.</p></header>
  <div class="settings-card typography-settings">
    <h3>Typography</h3>{#if typographyError}<p role="alert">{typographyError}</p>{/if}
    <p>Bundled fonts work offline. System fonts must be installed on each device.</p>
    <Select onopen={() => { void previewBundledFonts().catch(() => {}); }} label="Interface font" value={font} {options} onchange={(font) => ontypography({ font })} />
    <button class="text-button" onclick={discover}>Find system fonts</button>
    {#if fontNote}<p role="status">{fontNote}</p>{/if}
    <fieldset class="segmented-field"><legend>Font size</legend><div class="segmented">
      {#each [{ size: 13, label: 'S' }, { size: 14, label: 'M' }, { size: 16, label: 'L' }, { size: 18, label: 'XL' }] as item}
        <label><input type="radio" name="font-size" checked={fontSize === item.size} onchange={() => ontypography({ font_size: item.size })} />{item.label}<small>{item.size}px</small></label>
      {/each}
    </div></fieldset>
    <p style:font-size={fontSize + 'px'}>The quick brown fox brings your next message.</p>
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
