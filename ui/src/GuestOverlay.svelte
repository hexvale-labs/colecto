<script lang="ts" module>
  /** Wait before the next frame after `failures` failed ones: quick at first, then at most 15 s. */
  export const retryDelay = (failures: number) => Math.min(15_000, 500 * 2 ** Math.max(0, failures - 1));
  /** A restarting daemon (for example after updating its systemd unit) fails a few frames; only a longer outage is shown. */
  export const SHOW_AFTER_FAILURES = 4;
</script>
<script lang="ts">
  import { onMount } from 'svelte';
  import { extensionHost, type OverlayOutput } from './lib/api';
  let { id, account }: { id: string; account: string } = $props();
  let output = $state<OverlayOutput>({ overlay: { items: [] }, state: '' });
  let images = $state<Record<string, string>>({});
  let error = $state('');
  onMount(() => {
    let stopped = false, busy = false, failures = 0, resumeAt = 0;
    let pointer = { x: 0, y: 0, inside: false };
    const move = (e: PointerEvent) => pointer = { x: e.clientX, y: e.clientY, inside: true };
    const leave = () => pointer.inside = false;
    window.addEventListener('pointermove', move); document.addEventListener('pointerleave', leave);
    const tick = async () => {
      if (busy || stopped || document.hidden || Date.now() < resumeAt) return;
      busy = true;
      try {
        const next = await extensionHost.frame(account, id, JSON.stringify({ event: 'frame', time_ms: Date.now(), pointer, viewport: { width: innerWidth, height: innerHeight }, state: output.state }));
        for (const item of next.overlay.items) {
          if (!images[item.image]) {
            const url = URL.createObjectURL(await extensionHost.asset(id, item.image));
            if (stopped) { URL.revokeObjectURL(url); return; }
            images[item.image] = url;
          }
        }
        if (!stopped) { output = next; failures = 0; error = ''; }
      } catch (e) {
        // Keep trying: the service may be restarting. Earlier versions stopped at the first error for good.
        failures += 1; resumeAt = Date.now() + retryDelay(failures);
        if (!stopped && failures >= SHOW_AFTER_FAILURES) error = (e as Error).message;
      }
      finally { busy = false; }
    };
    const timer = setInterval(() => void tick(), 100); void tick();
    return () => { stopped = true; clearInterval(timer); window.removeEventListener('pointermove', move); document.removeEventListener('pointerleave', leave); Object.values(images).forEach(URL.revokeObjectURL); };
  });
</script>
{#if error}<p class="error-box overlay-error" role="alert">{id}: {error} Retrying…</p>{/if}
{#each output.overlay.items as item}
  <svg style:opacity={item.opacity ?? 1} aria-hidden="true" class="sprite" width={item.width} height={item.height} viewBox={`${item.frame.x} ${item.frame.y} ${item.frame.width} ${item.frame.height}`} style:transform={`translate(${item.x}px, ${item.y}px)`}>
    <image href={images[item.image]} />
  </svg>
{/each}
<style>.sprite { position: absolute; left: 0; top: 0; image-rendering: pixelated; overflow: hidden; } .overlay-error { position: absolute; left: 16px; bottom: 16px; max-width: min(420px, calc(100vw - 32px)); }</style>
