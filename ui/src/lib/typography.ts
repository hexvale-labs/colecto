/** Interface font and text size (Settings → Appearance). */
export interface BundledFont { id: string; name: string; family: string; load?: () => Promise<unknown> }

// Mona Sans ships with the main stylesheet; the others are fetched only when chosen or previewed.
export const bundledFonts: BundledFont[] = [
  { id: 'mona', name: 'Mona Sans', family: 'Mona Sans Variable' },
  { id: 'inter', name: 'Inter', family: 'Inter Variable', load: () => import('@fontsource-variable/inter') },
  { id: 'geist', name: 'Geist', family: 'Geist Variable', load: () => import('@fontsource-variable/geist') },
  { id: 'plex', name: 'IBM Plex Sans', family: 'IBM Plex Sans', load: () => Promise.all([import('@fontsource/ibm-plex-sans/400.css'), import('@fontsource/ibm-plex-sans/600.css')]) },
  { id: 'atkinson', name: 'Atkinson Hyperlegible Next', family: 'Atkinson Hyperlegible Next Variable', load: () => import('@fontsource-variable/atkinson-hyperlegible-next') },
];
export const DEFAULT_FONT = 'mona';

/** Text sizes offered in Settings. 14px is the size the interface is designed at. */
export const textSizes = [
  { size: 13, name: 'Small' }, { size: 14, name: 'Default' }, { size: 16, name: 'Large' }, { size: 18, name: 'Larger' },
] as const;
export const DEFAULT_TEXT_SIZE = 14;

/** CSS family for a stored font: a bundled id, or a system family name. */
export function familyOf(font: string) {
  return bundledFonts.find(item => item.id === font)?.family ?? font;
}
export function loadBundled(font?: string) {
  return Promise.all(bundledFonts.filter(item => item.load && (!font || item.id === font)).map(item => item.load!()));
}

let generation = 0;
/** Load the font if it is bundled, then switch the whole interface to it and to the text size. */
export async function applyTypography(font: string | null, size: number | null) {
  const request = ++generation;
  const id = font || DEFAULT_FONT;
  await loadBundled(id).catch(() => { /* Fall back to the next family in the stack. */ });
  if (request !== generation) return;
  const root = document.documentElement.style;
  if (id === DEFAULT_FONT) root.removeProperty('--font');
  else root.setProperty('--font', `${JSON.stringify(familyOf(id))}, ui-sans-serif, system-ui, sans-serif`);
  root.setProperty('--text-scale', String((size || DEFAULT_TEXT_SIZE) / DEFAULT_TEXT_SIZE));
}

// Common families on Linux, Windows and macOS. Browsers cannot list installed fonts without
// a permission prompt (and WebKit cannot at all), so these are probed by measuring text.
const candidates = [
  'Adwaita Sans', 'Cantarell', 'Noto Sans', 'Ubuntu', 'DejaVu Sans', 'Liberation Sans', 'Fira Sans', 'Source Sans 3', 'Open Sans', 'Roboto', 'Lato',
  'Segoe UI Variable', 'Segoe UI', 'Arial', 'Verdana', 'Tahoma', 'Trebuchet MS', 'Calibri', 'Aptos',
  'SF Pro', 'Helvetica Neue', 'Helvetica', 'Avenir Next', 'Georgia', 'Charter', 'Iowan Old Style',
];
/** Families from the list above that this device has installed. */
export function probeSystemFonts() {
  const context = document.createElement('canvas').getContext('2d');
  if (!context) return [];
  const sample = 'mmmmmmmmmmlliWW@#0123456789';
  return candidates.filter(family => ['monospace', 'serif'].some(fallback => {
    context.font = `72px ${fallback}`;
    const width = context.measureText(sample).width;
    context.font = `72px ${JSON.stringify(family)}, ${fallback}`;
    return context.measureText(sample).width !== width;
  }));
}
/** Every installed family, where the browser offers the Local Font Access API (asks first). */
export const canListAllFonts = () => typeof window !== 'undefined' && 'queryLocalFonts' in window;
export async function listAllFonts() {
  const fonts = await (window as unknown as { queryLocalFonts: () => Promise<{ family: string }[]> }).queryLocalFonts();
  return [...new Set(fonts.map(item => item.family))].filter(family => family.length <= 64).sort((a, b) => a.localeCompare(b));
}
