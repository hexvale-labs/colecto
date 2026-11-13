// The app's back stack. Every place you can go back from (Settings, a message on a phone, a
// dialog, an open listbox) claims an entry while it is open. Each entry is also a browser
// history entry, so the system back button, the Android back gesture, the browser's back and
// the in-app back arrows all close the newest thing first, and only leave the app from the
// inbox. See docs/DESIGN.md, "Back and gestures".
import { platform } from './native';

export interface BackEntry {
  /** Close what this entry opened. The owner may refuse (unsaved changes); see `isOpen`. */
  close: () => void;
  /** Checked just after `close`: still open means the owner kept it, and the entry stays. */
  isOpen: () => boolean;
  /** The screen that leaves on back. Gestures move it, and `push` slides it in and out. */
  surface?: () => HTMLElement | null | undefined;
  /** What the surface covers, shown underneath while it moves. */
  under?: () => HTMLElement | null | undefined;
  /** Slide the surface in when claimed and out when closed by back (full-screen pages). */
  push?: boolean;
}

const stack: BackEntry[] = [];
let depth = 0; // History entries above the base that belong to the stack.
let traversing = false; // Our own history.go() is on its way.
let scheduled = false;

const levelOf = (state: unknown) => typeof (state as { colectoBack?: unknown } | null)?.colectoBack === 'number' ? (state as { colectoBack: number }).colectoBack : 0;
const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

// History changes are batched to a microtask so closing one thing and opening another in the
// same update moves history once, and never pushes while a traversal is still in flight.
function schedule() { if (!scheduled) { scheduled = true; queueMicrotask(sync); } }
function sync() {
  scheduled = false;
  if (traversing) return;
  const want = stack.length;
  if (want > depth) {
    for (let level = depth + 1; level <= want; level++) history.pushState({ colectoBack: level }, '');
    depth = want;
  } else if (want < depth) {
    traversing = true;
    const delta = want - depth;
    depth = want;
    history.go(delta);
    // A traversal that never reports back (rare WebView quirks) must not block the stack.
    setTimeout(() => { if (traversing) { traversing = false; schedule(); } }, 400);
  }
}

function onPopState(event: PopStateEvent) {
  const level = levelOf(event.state);
  if (traversing && level === depth) { traversing = false; schedule(); return; }
  traversing = false;
  if (level < depth) {
    const count = depth - level;
    depth = level;
    for (let i = 0; i < count; i++) { const entry = stack.pop(); if (entry) leave(entry, i === 0); }
  } else if (level > depth) {
    // Forward into an entry nothing owns any more (or one left from before a reload).
    depth = level;
  }
  schedule();
}

/** Claim back while something is open. Returns the release to call when it closes by itself. */
export function claimBack(entry: BackEntry): () => void {
  stack.push(entry);
  schedule();
  if (entry.push) requestAnimationFrame(() => enter(entry));
  return () => {
    const index = stack.indexOf(entry);
    if (index < 0) return;
    stack.splice(index, 1);
    schedule();
  };
}

/** For in-app back arrows: go back through history when the stack has something to close. */
export function goBack() {
  if (!stack.length) return false;
  history.back();
  return true;
}

// --- Motion -----------------------------------------------------------------------------------
// iOS pages slide fully from the right with the drawer curve; elsewhere the Material shared-axis
// step: a short slide with a fade. Only transform and opacity move.
const EASE_DRAWER = 'cubic-bezier(0.32, 0.72, 0, 1)';
const EASE_OUT = 'cubic-bezier(0.23, 1, 0.32, 1)';
const offset = (el: HTMLElement) => platform === 'ios' ? el.clientWidth : Math.min(96, el.clientWidth * 0.3);

function peek(under: HTMLElement | null | undefined, on: boolean) {
  if (!under) return;
  if (on) under.dataset.peek = ''; else delete under.dataset.peek;
}

function enter(entry: BackEntry) {
  const el = entry.surface?.();
  if (!el || reduced() || !stack.includes(entry)) return;
  const under = entry.under?.();
  peek(under, true);
  const ios = platform === 'ios';
  const run = el.animate(
    [{ transform: `translateX(${offset(el)}px)`, opacity: ios ? 1 : 0 }, { transform: 'none', opacity: 1 }],
    { duration: ios ? 300 : 240, easing: ios ? EASE_DRAWER : EASE_OUT },
  );
  run.finished.catch(() => {}).finally(() => peek(under, false));
}

/** Back closed `entry`: play its exit (from where a gesture left it), then close it. */
function leave(entry: BackEntry, animate: boolean) {
  const el = entry.surface?.();
  const active = gesture?.entry === entry ? gesture : null;
  gesture = null;
  if (!animate || !el || reduced() || (!entry.push && !active)) { finish(entry, el, active?.under); return; }
  const under = active?.under ?? entry.under?.();
  peek(under, true);
  const ios = platform === 'ios';
  const from = { transform: el.style.transform || 'none', opacity: el.style.opacity || '1' };
  const to = active && !ios
    ? { transform: `translateX(${active.edge === 'right' ? -48 : 48}px) scale(0.9)`, opacity: '0' }
    : { transform: `translateX(${ios ? el.clientWidth : offset(el)}px)`, opacity: ios ? '1' : '0' };
  const run = el.animate([from, to], { duration: ios ? 240 : 180, easing: ios ? EASE_DRAWER : EASE_OUT, fill: 'forwards' });
  if (under && ios) under.animate([{ transform: under.style.transform || 'translateX(-25%)' }, { transform: 'none' }], { duration: 240, easing: EASE_DRAWER });
  // The screen underneath is visible but not yet current: a tap there would be undone when
  // this one closes, so the app ignores input until it has.
  document.body.inert = true;
  run.finished.catch(() => {}).finally(() => { document.body.inert = false; finish(entry, el, under); run.cancel(); });
}

function finish(entry: BackEntry, el: HTMLElement | null | undefined, under: HTMLElement | null | undefined) {
  reset(el, under);
  entry.close();
  // The owner kept it open (unsaved changes): it claims its place again.
  setTimeout(() => { if (entry.isOpen() && !stack.includes(entry)) { stack.push(entry); schedule(); } });
}

function reset(el: HTMLElement | null | undefined, under: HTMLElement | null | undefined) {
  if (el) { el.style.transform = ''; el.style.opacity = ''; el.style.borderRadius = ''; el.style.boxShadow = ''; el.style.transition = ''; el.style.willChange = ''; }
  if (under) { under.style.transform = ''; under.style.transition = ''; }
  peek(under, false);
}

// --- Gestures ---------------------------------------------------------------------------------
// Android streams predictive back progress from NativePlugin.kt as `colecto:back` events; the
// gesture itself commits through history (the plugin calls WebView.goBack()). On iOS the page
// tracks a swipe from the left edge and calls history.back() when it is far or fast enough.
interface Gesture { entry: BackEntry; el: HTMLElement; under: HTMLElement | null | undefined; edge: 'left' | 'right' }
let gesture: Gesture | null = null;

function begin(edge: 'left' | 'right') {
  const entry = stack.at(-1);
  const el = entry?.surface?.();
  if (!entry || !el || reduced()) { gesture = null; return false; }
  gesture = { entry, el, under: entry.under?.(), edge };
  el.style.transition = 'none'; el.style.willChange = 'transform';
  peek(gesture.under, true);
  return true;
}

function move(progress: number, dx = 0) {
  if (!gesture) return;
  const { el, under, edge } = gesture;
  const p = Math.max(0, Math.min(1, progress));
  if (platform === 'ios') {
    el.style.transform = `translateX(${dx}px)`;
    if (under) { under.style.transition = 'none'; under.style.transform = `translateX(${-25 * (1 - p)}%)`; }
  } else {
    // Material predictive back: the screen shrinks to 90 % and drifts away from the edge.
    el.style.transform = `translateX(${(edge === 'right' ? -1 : 1) * p * 24}px) scale(${1 - 0.1 * p})`;
    el.style.borderRadius = `${Math.round(p * 28)}px`;
    // An edge so the shrinking screen reads as a card over what it reveals.
    el.style.boxShadow = `0 0 0 1px var(--line), 0 24px 60px -16px oklch(0 0 0 / ${(0.7 * p).toFixed(2)})`;
  }
}

function cancel() {
  if (!gesture) return;
  const { el, under } = gesture;
  gesture = null;
  const run = el.animate([{ transform: el.style.transform || 'none', borderRadius: el.style.borderRadius || '0px', boxShadow: el.style.boxShadow || 'none' }, { transform: 'none', borderRadius: '0px', boxShadow: 'none' }], { duration: 200, easing: EASE_OUT });
  if (under && platform === 'ios') under.animate([{ transform: under.style.transform || 'none' }, { transform: 'translateX(-25%)' }], { duration: 200, easing: EASE_OUT });
  reset(el, null);
  run.finished.catch(() => {}).finally(() => reset(null, under));
}

function onNativeBack(event: Event) {
  const { phase, progress, edge } = (event as CustomEvent<{ phase: string; progress: number; edge: 'left' | 'right' }>).detail ?? {};
  if (phase === 'start') { if (begin(edge)) move(progress); }
  else if (phase === 'progress') move(progress);
  else if (phase === 'cancel') cancel();
}

/** iOS has no back button: a swipe from the left edge goes back, as in native apps. */
function edgeSwipe() {
  let start: { x: number; y: number; t: number } | null = null;
  let tracking = false;
  document.addEventListener('touchstart', event => {
    const touch = event.touches[0];
    start = event.touches.length === 1 && touch.clientX < 24 && stack.at(-1)?.surface ? { x: touch.clientX, y: touch.clientY, t: event.timeStamp } : null;
    tracking = false;
  }, { passive: true });
  document.addEventListener('touchmove', event => {
    if (!start) return;
    const touch = event.touches[0];
    const dx = touch.clientX - start.x, dy = touch.clientY - start.y;
    if (!tracking) {
      if (Math.abs(dy) > 10 && Math.abs(dy) > dx) { start = null; return; }
      if (dx < 10 || !begin('left')) return;
      tracking = true;
    }
    event.preventDefault();
    move(dx / (gesture?.el.clientWidth || innerWidth), Math.max(0, dx));
  }, { passive: false });
  const end = (event: TouchEvent) => {
    if (!start || !tracking || !gesture) { start = null; return; }
    const dx = (event.changedTouches[0]?.clientX ?? start.x) - start.x;
    const speed = dx / Math.max(1, event.timeStamp - start.t);
    start = null; tracking = false;
    if (event.type === 'touchend' && (dx > gesture.el.clientWidth * 0.35 || speed > 0.5)) history.back();
    else cancel();
  };
  document.addEventListener('touchend', end);
  document.addEventListener('touchcancel', end);
}

// Every modal dialog claims back while it is open, so back closes the top one, the same way
// Escape does (its `cancel` handler decides, and may keep it open).
function dialogs() {
  const showModal = HTMLDialogElement.prototype.showModal;
  HTMLDialogElement.prototype.showModal = function (this: HTMLDialogElement) {
    showModal.call(this);
    const release = claimBack({
      close: () => { if (this.open && this.dispatchEvent(new Event('cancel', { cancelable: true }))) this.close(); },
      isOpen: () => this.open,
    });
    this.addEventListener('close', release, { once: true });
  };
}

export function installBack() {
  // A reload starts a fresh stack; history entries from before it only lead back to the base.
  if (levelOf(history.state)) history.replaceState(null, '');
  addEventListener('popstate', onPopState);
  addEventListener('colecto:back', onNativeBack);
  if (platform === 'ios') edgeSwipe();
  dialogs();
}
