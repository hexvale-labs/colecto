// Touch and pen input for `<input type="range" class="slider">`. The Android app's WebView never
// moved a styled range under a finger, and WebKit only drags from the thumb, so the slider
// follows the pointer itself. Mouse input keeps the native behaviour. `touch-action: pan-y`
// (style.css) leaves vertical swipes to scrolling: a swipe that turns into a scroll cancels
// the pointer, so it never changes the value.

/** Svelte action: set the range from touch drags and taps, firing `input` and `change`. */
export function touchSlider(input: HTMLInputElement) {
  let pointer: number | null = null;
  let start = '';
  let dragging = false;
  let origin = 0;
  function set(x: number) {
    const box = input.getBoundingClientRect();
    // The thumb's centre travels between half a thumb from each end, as in the engines.
    const thumb = parseFloat(getComputedStyle(input).getPropertyValue('--thumb')) || 16;
    const min = Number(input.min || 0), max = Number(input.max || 100);
    const ratio = Math.min(1, Math.max(0, (x - box.left - thumb / 2) / Math.max(1, box.width - thumb)));
    const before = input.value;
    input.value = String(min + ratio * (max - min)); // The input rounds to its step.
    if (input.value !== before) input.dispatchEvent(new Event('input', { bubbles: true }));
  }
  function down(event: PointerEvent) {
    if (event.pointerType === 'mouse' || input.disabled || pointer !== null) return;
    pointer = event.pointerId; start = input.value; dragging = false; origin = event.clientX;
    input.setPointerCapture?.(pointer);
  }
  function move(event: PointerEvent) {
    if (event.pointerId !== pointer) return;
    // A few pixels of slop, so a resting finger does not nudge the value.
    if (!dragging && Math.abs(event.clientX - origin) < 4) return;
    dragging = true;
    set(event.clientX);
  }
  function up(event: PointerEvent) {
    if (event.pointerId !== pointer) return;
    pointer = null;
    set(event.clientX); // A tap jumps to where it landed.
    if (input.value !== start) input.dispatchEvent(new Event('change', { bubbles: true }));
  }
  function cancel(event: PointerEvent) {
    if (event.pointerId !== pointer) return;
    pointer = null;
    if (input.value !== start) input.dispatchEvent(new Event('change', { bubbles: true }));
  }
  input.addEventListener('pointerdown', down);
  input.addEventListener('pointermove', move);
  input.addEventListener('pointerup', up);
  input.addEventListener('pointercancel', cancel);
  return {
    destroy() {
      input.removeEventListener('pointerdown', down);
      input.removeEventListener('pointermove', move);
      input.removeEventListener('pointerup', up);
      input.removeEventListener('pointercancel', cancel);
    },
  };
}
