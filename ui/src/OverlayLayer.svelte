<script lang="ts">
  import { overlayExtensions } from './extensions/registry';
  import GuestOverlay from './GuestOverlay.svelte';
  import type { ExtensionState } from './lib/api';
  let { extensions, account }: { extensions: ExtensionState[]; account: string } = $props();
</script>
{#each extensions.filter(e => e.enabled && e.manifest.capabilities.includes('window.overlay')) as extension (extension.manifest.id)}
  <div class="extension-overlay" style:z-index={extension.manifest.z_index}>
    {#if extension.bundled}
      {@const Component = overlayExtensions[extension.manifest.id as keyof typeof overlayExtensions]}
      {#if Component}<Component />{/if}
    {:else}<GuestOverlay id={extension.manifest.id} {account} />{/if}
  </div>
{/each}
<style>.extension-overlay { position: fixed; inset: 0; pointer-events: none; }</style>
