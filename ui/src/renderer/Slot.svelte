<script lang="ts">
  // A place in one extension's surface where others with `ui.contribute` may add content
  // (docs/extensions/ui-model.md, "Contribution slots"). Each contribution draws in its own
  // surface, so its events go to the contributor and never to the slot's owner.
  import { getContext, untrack } from 'svelte';
  import { runtimes } from '../runtime/runtimes.svelte';
  import Contribution from './Contribution.svelte';
  import type { SurfaceContext } from './Surface.svelte';
  let { name, context }: { name: string; context: unknown } = $props();
  const surface = getContext<SurfaceContext>('surface');
  const full = $derived(`${surface.owner}/${name}`);
  // Close the slot only when its name changes or it unmounts, not when a parent replaces its
  // tree with the same context value.
  $effect(() => {
    const slot = full;
    return () => untrack(() => runtimes.openSlot(slot, null));
  });
  let sent = false;
  let lastSlot = '';
  let lastKey: string | undefined;
  $effect(() => {
    const slot = full;
    const value = $state.snapshot(context);
    const key = JSON.stringify(value);
    if (sent && slot === lastSlot && key === lastKey) return;
    sent = true;
    lastSlot = slot;
    lastKey = key;
    untrack(() => runtimes.openSlot(slot, value));
  });
  // Bundled contributors first, then the rest in install order.
  const contributors = $derived(runtimes.list.filter(runtime => runtime.status === 'running' && runtime.contributions[full]));
</script>

{#each contributors as runtime (runtime.id)}<Contribution {runtime} slot={full} tree={runtime.contributions[full]!} />{/each}
