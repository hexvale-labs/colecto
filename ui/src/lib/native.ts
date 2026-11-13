// The bridge to the mobile app's native plugin (crates/colecto-native). In a browser or on
// desktop every call here does nothing.
import { invoke, isTauri } from '@tauri-apps/api/core';

const agent = typeof navigator === 'object' ? navigator.userAgent : '';
/** Which platform's conventions to follow for motion and gestures; not used to guess input. */
export const platform: 'android' | 'ios' | 'other' = /Android/.test(agent) ? 'android'
  : /iPhone|iPad|iPod/.test(agent) || (/Macintosh/.test(agent) && navigator.maxTouchPoints > 1) ? 'ios' : 'other';
/** Running inside the Android or iOS app (not a mobile browser). */
export const nativeMobile = platform !== 'other' && typeof window === 'object' && (isTauri() || '__TAURI_INTERNALS__' in window);

const pressable = 'button, a[href], summary, label, [role="button"], [role="tab"], [role="option"], [role="menuitem"], input[type="checkbox"], input[type="radio"]';
const toggles = 'input[type="checkbox"], input[type="radio"], label:has(input[type="checkbox"], input[type="radio"]), [aria-pressed], [role="option"], [role="tab"]';

/** Press feedback for every control: the system click on Android, a selection tick for toggles. */
export function installPressFeedback() {
  if (!nativeMobile) return;
  let last = 0;
  document.addEventListener('click', event => {
    if (!event.isTrusted) return;
    const target = (event.target as Element | null)?.closest?.(pressable);
    if (!target || target.matches(':disabled, [aria-disabled="true"]')) return;
    // A tap on a label also clicks its input; one tap, one sound.
    if (event.timeStamp - last < 60) return;
    last = event.timeStamp;
    void invoke('plugin:colecto-native|press', { kind: target.matches(toggles) ? 'toggle' : 'press' }).catch(() => {});
  }, { capture: true, passive: true });
}
