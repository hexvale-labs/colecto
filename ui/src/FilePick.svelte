<script lang="ts">
  import Icon from './Icon.svelte';
  let { id, label, hint, accept, files = $bindable() }: { id: string; label: string; hint: string; accept: string; files?: FileList } = $props();
  let over = $state(false);
  let input: HTMLInputElement;
  let file = $derived(files?.[0]);
  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
  function drop(event: DragEvent) {
    event.preventDefault(); over = false;
    if (event.dataTransfer?.files.length) { input.files = event.dataTransfer.files; files = input.files; }
  }
</script>

<!-- The native input stays in place, labelled, for keyboard, screen readers and drag and drop. -->
<label class="file-pick" class:over class:chosen={!!file} for={id} ondragover={(event) => { event.preventDefault(); over = true; }} ondragleave={() => over = false} ondrop={drop}>
  <input bind:this={input} {id} type="file" {accept} required bind:files aria-label={label} />
  <span class="file-icon"><Icon name={file ? 'check' : 'upload'} size={18} /></span>
  <span class="file-text">
    <span class="file-label">{label}</span>
    <small>{file ? `${file.name} · ${size(file.size)}` : hint}</small>
  </span>
  <span class="file-action">{file ? 'Replace' : 'Browse'}</span>
</label>
