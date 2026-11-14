<script lang="ts" module>
  export interface Discontinued { name: string; version: number; release: string }
</script>
<script lang="ts">
  import Icon from './Icon.svelte';
  import { breakingApiNotes } from './lib/api';
  let { items, installing = false, onclose }: { items: Discontinued[]; installing?: boolean; onclose: () => void } = $props();
  function show(node: HTMLDialogElement) { node.showModal(); return { destroy() { node.close(); } }; }
  const why = (release: string) => window.open(breakingApiNotes(release), '_blank', 'noopener');
</script>

<dialog class="install-warning" aria-labelledby="api-version-title" aria-describedby="api-version-text" use:show oncancel={(event) => { event.preventDefault(); onclose(); }}>
  <span class="warning-mark" aria-hidden="true"><Icon name="alert" size={22} /></span>
  <h2 id="api-version-title">{items.length === 1 ? `${items[0].name} can’t be loaded` : `${items.length} extensions can’t be loaded`}</h2>
  <div id="api-version-text" class="warning-text">
    <p>{items.length === 1 ? 'It uses' : 'They use'} an extension API version this version of Colecto no longer supports. {installing ? 'Nothing was installed.' : `${items.length === 1 ? 'It stays' : 'They stay'} installed but won’t run until ${items.length === 1 ? 'its author updates it' : 'their authors update them'}. You can remove ${items.length === 1 ? 'it' : 'them'} in Settings → Extensions.`}</p>
    <ul>{#each items as item}<li><strong>{item.name}</strong> uses API {item.version}, dropped in Colecto {item.release}. <button class="text-button" onclick={() => why(item.release)}>Why?</button></li>{/each}</ul>
  </div>
  <div class="warning-actions">
    <!-- svelte-ignore a11y_autofocus (closing is the only safe default) -->
    <button class="secondary" autofocus onclick={onclose}>Close</button>
    {#if items.length === 1}<button class="primary" onclick={() => why(items[0].release)}>Read why</button>{/if}
  </div>
</dialog>
