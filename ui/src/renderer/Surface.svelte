<script lang="ts" module>
  export interface SurfaceContext { prefix: string; event: (node: string, kind: string, value?: unknown) => void }
</script>
<script lang="ts">
  // One place an extension draws, inside chrome the shell owns (docs/extensions/ui-model.md,
  // "Surfaces"). It shows the extension's state while it starts or after it stops.
  import { setContext } from 'svelte';
  import type { ExtensionRuntime } from '../runtime/runtime.svelte';
  import Node from './Node.svelte';
  let { runtime, surface }: { runtime: ExtensionRuntime; surface: string } = $props();
  // Node ids are the extension's own; prefixing keeps them apart from the app's and each other's.
  setContext<SurfaceContext>('surface', {
    get prefix() { return `x-${runtime.id}-${surface}`; },
    event: (node, kind, value) => runtime.event(surface, node, kind, value),
  });
  const tree = $derived(runtime.trees[surface]);
</script>

<div class="extension-surface" data-extension={runtime.id}>
  {#if runtime.status === 'stopped'}
    <div class="list-state error"><p role="alert">{runtime.reason || `${runtime.name} is not running.`}</p><button class="secondary" onclick={() => runtime.restart()}>Restart</button></div>
  {:else if tree}
    <Node node={tree} />
  {:else}
    <div class="list-state" role="status">Starting {runtime.name}…</div>
  {/if}
</div>

<style>
  .extension-surface { display: grid; align-content: start; gap: 12px; min-width: 0; min-height: 0; }
</style>
