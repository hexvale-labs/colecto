<script lang="ts" module>
  export interface SurfaceContext {
    prefix: string;
    /** The extension that owns this surface; slots in it are named after it. */
    owner: string;
    event: (node: string, kind: string, value?: unknown) => void;
    /** Files picked in a `file_picker`, sent with their bytes as transferred buffers. */
    files: (node: string, files: { name: string; type: string; size: number; failed?: boolean }[], buffers: ArrayBuffer[]) => void;
    may: (capability: string) => boolean;
    /** Narrow layout: a `nav` with `placement: "bottom"` is drawn by the shell instead. */
    lifted?: boolean;
  }
</script>
<script lang="ts">
  // One place an extension draws, inside chrome the shell owns (docs/extensions/ui-model.md,
  // "Surfaces"). It shows the extension's state while it starts or after it stops.
  import { setContext } from 'svelte';
  import type { ExtensionRuntime } from '../runtime/runtime.svelte';
  import Node from './Node.svelte';
  let { runtime, surface, bare = false }: { runtime: ExtensionRuntime; surface: string; bare?: boolean } = $props();
  // Node ids are the extension's own; prefixing keeps them apart from the app's and each other's.
  setContext<SurfaceContext>('surface', {
    get prefix() { return `x-${runtime.id}-${surface}`; },
    get owner() { return runtime.id; },
    event: (node, kind, value) => runtime.event(surface, node, kind, value),
    files: (node, files, buffers) => runtime.files(surface, node, files, buffers),
    may: capability => runtime.capabilities.includes(capability),
  });
  const tree = $derived(runtime.trees[surface]);
</script>

{#if bare}
  <!-- Panes that fill their dock (the mailbox, the reader) draw edge to edge. -->
  {#if runtime.status === 'stopped'}<div class="list-state error"><p role="alert">{runtime.reason || `${runtime.name} is not running.`}</p><button class="secondary" onclick={() => runtime.restart()}>Restart</button></div>
  {:else if tree}<Node node={tree} />{/if}
{:else}
<div class="extension-surface" data-extension={runtime.id}>
  {#if runtime.status === 'stopped'}
    <div class="list-state error"><p role="alert">{runtime.reason || `${runtime.name} is not running.`}</p><button class="secondary" onclick={() => runtime.restart()}>Restart</button></div>
  {:else if tree}
    <Node node={tree} />
  {:else}
    <div class="list-state" role="status">Starting {runtime.name}…</div>
  {/if}
</div>
{/if}

<style>
  .extension-surface { display: grid; align-content: start; gap: 12px; min-width: 0; min-height: 0; }
</style>
