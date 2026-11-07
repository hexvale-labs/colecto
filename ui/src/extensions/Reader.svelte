<script lang="ts">
  import { onMount } from 'svelte';
  import type { Message, ReaderService } from '../lib/api';
  let { id, service, accountLabel, folder = 'Inbox', onback }: { id: string; folder?: string; accountLabel: string; service: ReaderService; onback: () => void } = $props();
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
      <div class="sender-row"><span class="avatar" aria-hidden="true">{message.sender.charAt(0)}</span><div><strong>{message.sender}</strong><span class="address">{message.address}</span></div><span class="to-me">to you</span></div>
      <div class="letter-body">{message.body}</div>
      <div class="letter-end" aria-hidden="true">• • •</div>
    </article>
  {/if}
</section>
