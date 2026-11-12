export const bundledFonts = [
  { value: 'mona', label: 'Mona Sans', family: 'Mona Sans Variable' },
  { value: 'inter', label: 'Inter', family: 'Inter Variable' },
  { value: 'geist', label: 'Geist', family: 'Geist Variable' },
  { value: 'plex', label: 'IBM Plex Sans', family: 'IBM Plex Sans' },
  { value: 'atkinson', label: 'Atkinson Hyperlegible Next', family: 'Atkinson Hyperlegible Next Variable' },
];
const loaders: Record<string, () => Promise<unknown>> = {
  inter: () => import('@fontsource-variable/inter'),
  geist: () => import('@fontsource-variable/geist'),
  plex: () => import('@fontsource/ibm-plex-sans/400.css').then(() => import('@fontsource/ibm-plex-sans/600.css')),
  atkinson: () => import('@fontsource-variable/atkinson-hyperlegible-next'),
};
export async function previewBundledFonts() { await Promise.all(Object.values(loaders).map(load => load())); }
let generation = 0;
export async function applyTypography(font = 'mona', size = 14) {
  const request = ++generation;
  if (loaders[font]) await loaders[font]();
  if (request !== generation) return;
  const family = bundledFonts.find(item => item.value === font)?.family ?? font;
  document.documentElement.style.setProperty('--font', `${JSON.stringify(family)}, system-ui, sans-serif`);
  document.documentElement.style.fontSize = `${size}px`;
}
export function installedFonts() {
  const candidates = ['Arial', 'Arial Narrow', 'Verdana', 'Tahoma', 'Trebuchet MS', 'Georgia', 'Segoe UI', 'Segoe UI Variable', 'Calibri', 'Candara', 'Corbel', 'Helvetica', 'Helvetica Neue', 'SF Pro', 'SF Pro Text', 'Lucida Grande', 'Cantarell', 'Noto Sans', 'Ubuntu', 'DejaVu Sans', 'Liberation Sans', 'Droid Sans', 'Roboto', 'Open Sans', 'Lato'];
  const context = document.createElement('canvas').getContext('2d');
  if (!context) return [];
  return candidates.filter(family => ['monospace', 'serif'].some(fallback => {
    context.font = `72px ${fallback}`;
    const width = context.measureText('mmmmmmmmmmlliWW012345').width;
    context.font = `72px "${family}", ${fallback}`;
    return context.measureText('mmmmmmmmmmlliWW012345').width !== width;
  }));
}
