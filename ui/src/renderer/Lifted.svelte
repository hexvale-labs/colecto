<script lang="ts">
  // On narrow windows, a pane's bottom navigation (the mailbox's folders) sits in the app frame
  // rather than inside its pane, and stays put while panes switch.
  import { setContext } from 'svelte';
  import type { Node as Tree } from '../runtime/protocol';
  import { runtimes } from '../runtime/runtimes.svelte';
  import Node from './Node.svelte';
  import type { SurfaceContext } from './Surface.svelte';
  const find = (node: Tree | null | undefined): Tree | null => {
    if (!node) return null;
    if (node.type === 'nav' && node.placement === 'bottom') return node;
    for (const child of node.children ?? []) { const hit = find(child); if (hit) return hit; }
    return null;
  };
  const lifted = $derived(runtimes.list.map(runtime => ({ runtime, node: find(runtime.trees.pane) })).find(entry => entry.node));
  setContext<SurfaceContext>('surface', {
    get prefix() { return `x-${lifted?.runtime.id}-pane`; },
    get owner() { return lifted?.runtime.id ?? ''; },
    event: (node, kind, value) => lifted?.runtime.event('pane', node, kind, value),
    files: () => {},
    may: capability => !!lifted?.runtime.capabilities.includes(capability),
    lifted: true,
  });
</script>

{#if lifted?.node}<Node node={lifted.node} />{/if}
