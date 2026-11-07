<script lang="ts">
  import { initial } from '../lib/initial';
  import type { Labels } from '../lib/api';
  import { onMount } from 'svelte';
  import type { Message, ReaderService } from '../lib/api';
  let { id, service, accountLabel, folder = 'Inbox', labels, labeling, labelError = '', onlabel, onback }: { labelError?: string; labels: Labels; labeling: boolean; onlabel: (name: string, remove: boolean) => void; id: string; folder?: string; accountLabel: string; service: ReaderService; onback: () => void } = $props();
  let message = $state<Message | null>(null);
  let error = $state('');
  let loading = $state(true);
  async function load() {
    loading = true; error = '';
    try { message = await service.read(id); }
    catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }
  onMount(() => { void load(); });
</script>
<section class="detail" aria-label="Message reader" aria-busy={loading}>
  <header class="detail-toolbar"><button class="text-button back" onclick={onback}>← Back to list</button><span class="toolbar-label">MESSAGE</span><span class="pill">{accountLabel}</span></header>
  {#if loading}<div class="detail-state">Opening message…</div>
  {:else if error}<div class="detail-state"><p role="alert">{error}</p><button class="secondary" onclick={load}>Try again</button></div>
  {:else if message}
    <article class="letter">
      <div class="eyebrow">{message.date} · {folder.toUpperCase()}</div>
      <h2>{message.subject || '(No subject)'}</h2>
      <div class="sender-row"><span class="avatar" aria-hidden="true">{initial(message.sender || message.address)}</span><div><strong>{message.sender || message.address || 'Unknown sender'}</strong><span class="address">{message.address}</span></div><span class="to-me">to you</span></div>
      {#if labelError}<p class="error-box" role="alert">{labelError}</p>{/if}
      {#if Object.keys(labels).length}<fieldset class="message-labels" disabled={labeling}><legend>Labels</legend>{#each Object.keys(labels) as name}<label><input type="checkbox" aria-label={`Label ${name}`} checked={labels[name].includes(id)} onchange={(event) => onlabel(name, !event.currentTarget.checked)} />{name}</label>{/each}</fieldset>{/if}
      <div class="letter-body">{message.body}</div>
      <div class="letter-end" aria-hidden="true">• • •</div>
    </article>
  {/if}
</section>
