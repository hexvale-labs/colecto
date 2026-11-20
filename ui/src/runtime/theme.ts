// Theme tokens for extensions (docs/extensions/ui-model.md, "Theme tokens"): the colours,
// font and density the renderer already uses, resolved to plain values so frame and canvas
// content can match. Settings → Appearance changes them; the shell then sends `theme`.
import { prefersReducedMotion } from '../lib/ui';

export interface Theme {
  scheme: 'dark';
  colors: Record<string, string>;
  font: { family: string; size: number; scale: number };
  density: 'touch' | 'pointer';
  reduced_motion: boolean;
}

const COLORS = ['ground', 'bg', 'sunk', 'raise', 'control', 'float', 'line', 'line-soft', 'ink', 'ink-2', 'ink-3',
  'mint', 'mint-hi', 'mint-ink', 'select', 'status-online', 'danger', 'danger-bg'] as const;
// Token names follow the design page; `mint` is the accent.
const NAMES: Record<string, string> = { mint: 'accent', 'mint-hi': 'accent_hi', 'mint-ink': 'accent_ink', 'status-online': 'online' };

/** Any CSS colour as `#rrggbb`, by letting the browser paint it. */
function hex(color: string, context: CanvasRenderingContext2D) {
  context.clearRect(0, 0, 1, 1);
  context.fillStyle = '#000';
  context.fillStyle = color;
  context.fillRect(0, 0, 1, 1);
  const [r, g, b] = context.getImageData(0, 0, 1, 1).data;
  return `#${[r, g, b].map(value => value.toString(16).padStart(2, '0')).join('')}`;
}

export function currentTheme(): Theme {
  const style = getComputedStyle(document.documentElement);
  const canvas = Object.assign(document.createElement('canvas'), { width: 1, height: 1 });
  const context = canvas.getContext('2d', { willReadFrequently: true });
  const colors: Record<string, string> = {};
  for (const name of COLORS) {
    // Custom properties stay unresolved (`color-mix(…)`); a probe element resolves them.
    const probe = document.createElement('span');
    probe.style.color = `var(--${name})`;
    document.body.append(probe);
    const resolved = getComputedStyle(probe).color;
    probe.remove();
    colors[NAMES[name] ?? name.replace(/-/g, '_')] = context ? hex(resolved, context) : resolved;
  }
  const scale = Number(style.getPropertyValue('--text-scale')) || 1;
  return {
    scheme: 'dark',
    colors,
    font: { family: style.getPropertyValue('--font').split(',')[0].trim().replace(/^['"]|['"]$/g, ''), size: Math.round(14 * scale), scale },
    density: matchMedia('(pointer: coarse)').matches ? 'touch' : 'pointer',
    reduced_motion: prefersReducedMotion(),
  };
}
