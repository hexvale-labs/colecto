<script lang="ts">
  import { initial } from '../lib/initial';
  import type { Labels } from '../lib/api';
  import { onMount } from 'svelte';
  import type { Draft, Message, MessageContent, ReaderService, AttachmentInfo } from '../lib/api';
  import { replyDraft, type ReplyMode } from '../lib/reply';
  let { id, service, accountLabel, self = '', folder = 'Inbox', labels, labeling, labelError = '', onlabel, onback, onreply }: { labelError?: string; labels: Labels; labeling: boolean; onlabel: (name: string, remove: boolean) => void; id: string; folder?: string; accountLabel: string; self?: string; service: ReaderService; onback: () => void; onreply?: (draft: Draft) => void } = $props();
  let message = $state<Message | null>(null);
  let content = $state<MessageContent | null>(null);
  let contentError = $state('');
  let error = $state('');
  let loading = $state(true);
  let showHtml = $state(true);
  let remoteImages = $state(false);
  let downloading = $state<number | null>(null);
  // The frame has no scripts, forms, or same-origin access; this policy also blocks remote content until asked.
  let frameDoc = $derived(content?.html ? `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:${remoteImages ? ' https:' : ''}"><base target="_blank"><style>html{color-scheme:light}body{margin:20px;font:15px/1.6 system-ui,sans-serif;color:#1d1d1f;background:#fff;overflow-wrap:anywhere}img{max-width:100%;height:auto}</style>${content.html}` : '');
  async function load() {
    loading = true; error = '';
    try { message = await service.read(id); }
    catch (e) { error = (e as Error).message; }
    finally { loading = false; }
    if (message) {
      try { content = await service.content(id); contentError = ''; }
      catch (e) { contentError = (e as Error).message; }
    }
  }
  function reply(mode: ReplyMode) { if (message) onreply?.(replyDraft(mode, message, content, self)); }
  async function save(file: AttachmentInfo) {
    downloading = file.index; contentError = '';
    try {
      const url = URL.createObjectURL(await service.attachment(id, file.index));
      const link = Object.assign(document.createElement('a'), { href: url, download: file.name });
      link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (e) { contentError = (e as Error).message; }
    finally { downloading = null; }
  }
  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : bytes < 1_048_576 ? `${Math.round(bytes / 1024)} KB` : `${(bytes / 1_048_576).toFixed(1)} MB`;
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
      {#if onreply}<div class="reply-actions" role="group" aria-label="Respond"><button class="secondary" onclick={() => reply('reply')}>Reply</button><button class="secondary" onclick={() => reply('all')}>Reply all</button><button class="secondary" onclick={() => reply('forward')}>Forward</button></div>{/if}
      {#if contentError}<p class="content-note" role="status">Showing saved text. {contentError}</p>{/if}
      {#if content?.attachments.length}<ul class="attachments" aria-label="Attachments">{#each content.attachments as file (file.index)}<li><button class="attachment" disabled={downloading !== null} onclick={() => save(file)} aria-label={`Download ${file.name}`}><span class="attachment-name">{file.name}</span><small>{downloading === file.index ? 'Downloading…' : size(file.size)}</small></button></li>{/each}</ul>{/if}
      {#if content?.html}<div class="body-controls"><button class="text-button" aria-pressed={!showHtml} onclick={() => showHtml = !showHtml}>{showHtml ? 'Show plain text' : 'Show formatted'}</button>{#if showHtml && !remoteImages}<button class="text-button" onclick={() => remoteImages = true}>Load remote images</button>{/if}</div>{/if}
      {#if content?.html && showHtml}<iframe class="html-body" title="Message content" sandbox="allow-popups allow-popups-to-escape-sandbox" referrerpolicy="no-referrer" srcdoc={frameDoc}></iframe>
      {:else}<div class="letter-body">{content?.text || message.body}</div>{/if}
      <div class="letter-end" aria-hidden="true">• • •</div>
    </article>
  {/if}
</section>
