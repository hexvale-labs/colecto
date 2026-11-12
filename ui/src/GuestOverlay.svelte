<script lang="ts">
  import { onMount } from 'svelte';
  import { extensionHost, type OverlayOutput } from './lib/api';
  let { id, account }: { id: string; account: string } = $props();
  let output = $state<OverlayOutput>({ overlay: { items: [] }, state: '' });
  let images = $state<Record<string, string>>({});
  let error = $state('');
  onMount(() => {
    let stopped = false, busy = false;
    let pointer = { x: 0, y: 0, inside: false };
    const move = (e: PointerEvent) => pointer = { x: e.clientX, y: e.clientY, inside: true };
    const leave = () => pointer.inside = false;
    window.addEventListener('pointermove', move); document.addEventListener('pointerleave', leave);
    const tick = async () => {
      if (busy || stopped || document.hidden || error) return;
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
        if (!stopped) output = next;
      } catch (e) { if (!stopped) error = (e as Error).message; }
      finally { busy = false; }
    };
    const timer = setInterval(() => void tick(), 100); void tick();
    return () => { stopped = true; clearInterval(timer); window.removeEventListener('pointermove', move); document.removeEventListener('pointerleave', leave); Object.values(images).forEach(URL.revokeObjectURL); };
  });
</script>
{#if error}<p class="error-box" role="alert">{id}: {error}</p>{/if}
{#each output.overlay.items as item}
  <svg style:opacity={item.opacity ?? 1} aria-hidden="true" class="sprite" width={item.width} height={item.height} viewBox={`${item.frame.x} ${item.frame.y} ${item.frame.width} ${item.frame.height}`} style:transform={`translate(${item.x}px, ${item.y}px)`}>
    <image href={images[item.image]} />
  </svg>
{/each}
<style>.sprite { position: absolute; left: 0; top: 0; image-rendering: pixelated; overflow: hidden; }</style>
