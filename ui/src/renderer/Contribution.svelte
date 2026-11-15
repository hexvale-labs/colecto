<script lang="ts">
  import { setContext } from 'svelte';
  import type { Node as Tree } from '../runtime/protocol';
  import type { ExtensionRuntime } from '../runtime/runtime.svelte';
  import Node from './Node.svelte';
  import type { SurfaceContext } from './Surface.svelte';
  let { runtime, slot, tree }: { runtime: ExtensionRuntime; slot: string; tree: Tree } = $props();
  setContext<SurfaceContext>('surface', {
    get prefix() { return `x-${runtime.id}-slot`; },
    get owner() { return runtime.id; },
    event: (node, kind, value) => runtime.event(`slot:${slot}`, node, kind, value),
    files: (node, files, buffers) => runtime.files(`slot:${slot}`, node, files, buffers),
    may: capability => runtime.capabilities.includes(capability),
  });
</script>

<!-- `display: contents` keeps the slot owner's layout; the title names who drew it. -->
<div class="x-contribution" title={`From ${runtime.name}`} data-extension={runtime.id}><Node node={tree} /></div>

<style>.x-contribution { display: contents; }</style>
