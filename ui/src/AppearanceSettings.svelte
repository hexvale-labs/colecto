<script lang="ts">
  import Icon from './Icon.svelte';
  import Select from './Select.svelte';
  import { bundledFonts, canListAllFonts, familyOf, listAllFonts, loadBundled, probeSystemFonts, textSizes } from './lib/typography';
  import type { TextSpacing } from './lib/text-spacing.svelte';
  let { value, font, textSize, spacing, onchange, ontypography, children }: {
    value: string | null; font: string; textSize: number; spacing: TextSpacing;
    onchange: (accent: string) => void; ontypography: (change: { font?: string; font_size?: number; text_spacing?: TextSpacing }) => void; children?: import('svelte').Snippet;
  } = $props();
  // More room around letters, words, lines and paragraphs helps many readers with dyslexia.
  const spacings: { key: keyof TextSpacing; name: string; text: string }[] = [
    { key: 'letter', name: 'Letter spacing', text: 'Space between characters.' },
    { key: 'word', name: 'Word spacing', text: 'Space between words.' },
    { key: 'line', name: 'Line height', text: 'Space between lines.' },
    { key: 'paragraph', name: 'Paragraph spacing', text: 'Space between paragraphs, in mail and descriptions.' },
  ];
  const steps = ['Normal', 'More', 'Most'];
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
  <p>Colecto stays dark. Choose how text looks, the accent used for selection, focus and primary buttons, and how the message list is laid out.</p></header>
  <div class="settings-group">
    <h3 class="group-label">Text</h3>
    <div class="setting">
      <div class="setting-text"><h4 id="font-title">Font</h4><p>Bundled fonts work offline on every device. System fonts must be installed where you use Colecto.</p></div>
      <div class="font-picker">
        <Select label="Interface font" value={font} options={fontOptions} onopen={() => void loadBundled().catch(() => {})} onchange={(next) => ontypography({ font: next })}>
          {#snippet trigger(current)}<span class="font-current" style:font-family={familyOf(current?.value ?? font)}>{current?.label ?? font}</span>{/snippet}
        </Select>
        {#if canListAllFonts()}<button type="button" class="text-button" onclick={listAll}>List all installed fonts</button>{/if}
      </div>
      {#if fontNote}<p class="hint" role="status">{fontNote}</p>{/if}
    </div>
    <fieldset class="setting setting-row">
      <legend class="sr-only">Text size</legend>
      <div class="setting-text"><h4>Text size</h4><p>Scales text across the app.</p></div>
      <div class="segmented text-sizes">
        {#each textSizes as item (item.size)}
          <label title={`${item.name}, ${item.size}px`}><input type="radio" name="text-size" checked={textSize === item.size} onchange={() => ontypography({ font_size: item.size })} /><span class="sample" style:font-size={`${item.size + 1}px`} aria-hidden="true">Aa</span><span class="sr-only">{item.name}</span></label>
        {/each}
      </div>
    </fieldset>
    {#each spacings as item (item.key)}
      <fieldset class="setting setting-row">
        <legend class="sr-only">{item.name}</legend>
        <div class="setting-text"><h4>{item.name}</h4><p>{item.text}</p></div>
        <div class="segmented">
          {#each steps as step, index (step)}<label><input type="radio" name={`spacing-${item.key}`} checked={spacing[item.key] === index} onchange={() => ontypography({ text_spacing: { ...spacing, [item.key]: index } })} />{step}</label>{/each}
        </div>
      </fieldset>
    {/each}
  </div>
  <div class="settings-group">
    <h3 class="group-label">Colour</h3>
    <div class="setting">
      <div class="setting-text"><h4 id="accent-title">Accent</h4><p>Used for selection, focus and primary buttons. Pick a preset or any colour.</p></div>
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
      {#if value}<button type="button" class="text-button" onclick={() => onchange('')}>Restore the default mint</button>{/if}
    </div>
  </div>
  {@render children?.()}
</section>
