// Ctrl + / Ctrl - / Ctrl 0 zoom the whole interface in the desktop app (#119). A browser
// already zooms with these keys and remembers the level per site, so there the keys are left
// to it. On phones and tablets the system's own text size and pinch apply instead.
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { nativeMobile } from './native';

/** Zoom levels, as browsers step through them. */
export const ZOOM_STEPS = [0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];
const KEY = 'colecto.zoom';

/** The next level from `level` for a key, or `null` when the key is not a zoom shortcut. */
export function nextZoom(level: number, key: string): number | null {
  const index = ZOOM_STEPS.indexOf(level);
  const at = index < 0 ? ZOOM_STEPS.indexOf(1) : index;
  if (key === '0') return 1;
  // `=` is the unshifted `+` on many layouts; `_` the shifted `-`.
  if (key === '+' || key === '=') return ZOOM_STEPS[Math.min(at + 1, ZOOM_STEPS.length - 1)];
  if (key === '-' || key === '_') return ZOOM_STEPS[Math.max(at - 1, 0)];
  return null;
}

export function installZoom() {
  if (typeof window !== 'object' || nativeMobile || !(isTauri() || '__TAURI_INTERNALS__' in window)) return;
  let level = 1;
  try { const saved = Number(localStorage.getItem(KEY)); if (ZOOM_STEPS.includes(saved)) level = saved; } catch { /* Stays at 100%. */ }
  const apply = () => void getCurrentWebview().setZoom(level).catch(() => {});
  if (level !== 1) apply();
  window.addEventListener('keydown', event => {
    if (!(event.ctrlKey || event.metaKey) || event.altKey || event.isComposing) return;
    // Numpad keys report their own codes; map them to the same shortcuts.
    const key = event.code === 'NumpadAdd' ? '+' : event.code === 'NumpadSubtract' ? '-' : event.code === 'Numpad0' ? '0' : event.key;
    const next = nextZoom(level, key);
    if (next === null) return;
    event.preventDefault();
    if (next === level) return;
    level = next; apply();
    try { localStorage.setItem(KEY, String(level)); } catch { /* Not remembered this time. */ }
  }, { capture: true });
}
