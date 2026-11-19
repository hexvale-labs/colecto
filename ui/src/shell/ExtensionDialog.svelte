<script lang="ts">
  // An extension's dialog: a native modal sheet whose header names the extension, so it can
  // never pass as Colecto's own. Escape always closes it.
  import { setContext } from 'svelte';
  import Icon from '../Icon.svelte';
  import Node from '../renderer/Node.svelte';
  import type { SurfaceContext } from '../renderer/Surface.svelte';
  import type { ExtensionRuntime } from '../runtime/runtime.svelte';
  let { runtime }: { runtime: ExtensionRuntime } = $props();
  setContext<SurfaceContext>('surface', {
    get prefix() { return `x-${runtime.id}-dialog`; },
    get owner() { return runtime.id; },
    event: (node, kind, value) => runtime.event('dialog', node, kind, value),
    files: (node, files, buffers) => runtime.files('dialog', node, files, buffers),
    may: capability => runtime.capabilities.includes(capability),
    menu: (node, key, x, y) => runtime.openMenu('dialog', node, key, x, y),
  });
  function open(node: HTMLDialogElement) { node.showModal(); return { destroy() { node.close(); } }; }
  function close() { runtime.dialog = null; runtime.event('dialog', '', 'close'); }
</script>

{#if runtime.dialog}
  <!-- Named by the extension's own label when it gives one; the header still names the extension. -->
  <dialog class="sheet x-dialog" aria-label={typeof runtime.dialog.label === 'string' ? runtime.dialog.label : `${runtime.name} dialog`} use:open oncancel={(event) => { event.preventDefault(); close(); }}>
    <header class="sheet-head x-dialog-head">
      <span class="x-dialog-source"><Icon name="extension" size={14} />{runtime.name}</span>
      <button type="button" class="icon-button" aria-label="Close" onclick={close}><Icon name="close" size={16} /></button>
    </header>
    <div class="sheet-body"><Node node={runtime.dialog} /></div>
  </dialog>
{/if}

<style>
  .x-dialog-head { align-items: center; padding-bottom: 0; }
  .x-dialog-source { display: inline-flex; align-items: center; gap: 6px; color: var(--ink-3); font-size: calc(12.5px * var(--text-scale)); font-weight: 560; }
  /* An extension's dialog is usually one form, so the body's gap would not reach its fields
     and the controls sat flush against each other (#86). A heading keeps its lead-in close. */
  .sheet-body > :global(form) { display: grid; gap: 16px; }
  .sheet-body > :global(form > :is(h2, h3) + .x-text) { margin-top: -10px; }
</style>
