<script lang="ts">
  import { initial, hue } from '../../lib/initial';
  import { when } from '../../lib/when';
  import Icon from '../../Icon.svelte';
  import type { Labels } from '../../lib/api';
  import { onMount } from 'svelte';
  import type { Draft, Message, MessageContent, ReaderService, AttachmentInfo } from '../../lib/api';
  import { replyDraft, type ReplyMode } from '../../lib/reply';
  import { copyText } from '../../lib/copy';
  let { id, service, self = '', folder = 'Inbox', labels, labeling, labelError = '', remoteImages: alwaysRemote = false, onlabel, onback, onreply }: { remoteImages?: boolean; labelError?: string; labels: Labels; labeling: boolean; onlabel: (name: string, remove: boolean) => void; id: string; folder?: string; self?: string; service: ReaderService; onback: () => void; onreply?: (draft: Draft) => void } = $props();
  let message = $state<Message | null>(null);
  let content = $state<MessageContent | null>(null);
  let contentError = $state('');
  let error = $state('');
  let loading = $state(true);
  let showHtml = $state(true);
  let allowRemote = $state(false);
  let remoteImages = $derived(alwaysRemote || allowRemote);
  let downloading = $state<number | null>(null);
  // The frame has no scripts, forms, or same-origin access; this policy also blocks remote content until asked.
  // Mail that paints its own backgrounds or text colours keeps its light sheet, so dark ink stays readable.
  // Plain HTML mail adopts the dark reading colours.
  let designed = $derived(!!content?.html && /bgcolor|background(-color)?\s*:|(^|[^-\w])color\s*[:=]/i.test(content.html));
  let frameStyle = $derived(designed
    ? 'html{color-scheme:light}body{margin:24px;font:15px/1.6 system-ui,sans-serif;color:#1d1d1f;background:#fff;overflow-wrap:anywhere}'
    : 'html{color-scheme:dark}body{margin:0;font:15.5px/1.65 system-ui,sans-serif;color:#d5ddd7;background:transparent;overflow-wrap:anywhere}a{color:#a2d6b1}blockquote{margin:0 0 0 2px;padding-left:14px;border-left:1px solid #3a463f;color:#9faba3}');
  // Designed mail is shown on its own light sheet, so its dark-mode rules would mix light backgrounds with light text.
  // Rename the media feature so those rules never match, whatever colour scheme the webview reports.
  let html = $derived(content?.html ? designed ? content.html.replace(/prefers-color-scheme\s*:\s*dark/gi, 'prefers-color-scheme: colecto-light') : content.html : '');
  let frameDoc = $derived(html ? `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:${remoteImages ? ' https:' : ''}"><base target="_blank"><style>${frameStyle}img{max-width:100%;height:auto}</style>${html}` : '');
  let contentLoading = $state(true);
  let date = $derived(message ? when(message.date) : null);
  async function load() {
    loading = true; contentLoading = true; error = '';
    // Fetch the body alongside the header instead of after it; opening HTML mail used to wait for both in turn.
    const body = service.content(id);
    body.catch(() => {});
    try { message = await service.read(id); }
    catch (e) { error = (e as Error).message; }
    finally { loading = false; }
    if (message) {
      try { content = await body; contentError = ''; }
      catch (e) { contentError = (e as Error).message; }
      finally { contentLoading = false; }
    }
  }
  let copied = $state<boolean | null>(null);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  async function copy() {
    if (!message) return;
    copied = await copyText(content?.text || message.body);
    clearTimeout(copiedTimer); copiedTimer = setTimeout(() => copied = null, 1600);
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

<section class="detail reader" aria-label="Message reader" aria-busy={loading}>
  <header class="detail-toolbar">
    <button class="icon-button back" aria-label="Back to list" onclick={onback}><Icon name="back" size={18} /></button>
    <span class="toolbar-crumb">{folder}</span>
    {#if onreply && message}<div class="toolbar-actions" role="group" aria-label="Respond">
      <button class="tool" aria-label="Reply" title="Reply" onclick={() => reply('reply')}><Icon name="reply" /><span>Reply</span></button>
      <button class="tool" aria-label="Reply all" title="Reply all" onclick={() => reply('all')}><Icon name="replyAll" /><span>Reply all</span></button>
      <button class="tool" aria-label="Forward" title="Forward" onclick={() => reply('forward')}><Icon name="forward" /><span>Forward</span></button>
    </div>{/if}
    {#if message}<button class="tool copy-tool" class:push={!onreply} aria-label="Copy message text" title="Copy message text" onclick={copy}><Icon name={copied ? 'check' : 'copy'} /><span aria-live="polite">{copied === null ? 'Copy' : copied ? 'Copied' : 'Copy failed'}</span></button>{/if}
  </header>
  {#if loading}<div class="detail-state" role="status">Opening message…</div>
  {:else if error}<div class="detail-state"><p role="alert">{error}</p><button class="secondary" onclick={load}>Try again</button></div>
  {:else if message}
    <article class="letter">
      <h2>{message.subject || '(No subject)'}</h2>
      <div class="sender-row">
        <span class="avatar" style:--hue={hue(message.address || message.sender)} aria-hidden="true">{initial(message.sender || message.address)}</span>
        <div class="sender-text"><strong>{message.sender || message.address || 'Unknown sender'}</strong>{#if message.address && message.address !== message.sender}<span class="address">{message.address}</span>{/if}</div>
        {#if date?.full}<time class="sent-at" title={message.date}>{date.full}</time>{/if}
      </div>
      {#if labelError}<p class="error-box" role="alert">{labelError}</p>{/if}
      {#if Object.keys(labels).length}<fieldset class="message-labels" disabled={labeling}><legend class="sr-only">Labels</legend>{#each Object.keys(labels) as name (name)}<label class="chip toggle"><input type="checkbox" aria-label={`Label ${name}`} checked={labels[name].includes(id)} onchange={(event) => onlabel(name, !event.currentTarget.checked)} /><Icon name="check" size={12} />{name}</label>{/each}</fieldset>{/if}
      {#if content?.attachments.length}<ul class="attachments" aria-label="Attachments">{#each content.attachments as file (file.index)}<li><button class="attachment" disabled={downloading !== null} onclick={() => save(file)} aria-label={`Download ${file.name}`}><Icon name="attach" /><span class="attachment-name">{file.name}</span><small>{downloading === file.index ? 'Downloading…' : size(file.size)}</small></button></li>{/each}</ul>{/if}
      {#if contentError}<p class="content-note" role="status">Showing saved text. {contentError}</p>{/if}
      {#if content?.html}<div class="body-controls">
        {#if showHtml && !remoteImages}<span class="body-note"><Icon name="image" />Remote images are blocked.</span><button class="text-button" onclick={() => allowRemote = true}>Load remote images</button>{/if}
        <button class="text-button push" aria-pressed={!showHtml} onclick={() => showHtml = !showHtml}><Icon name="text" />{showHtml ? 'Show plain text' : 'Show formatted'}</button>
      </div>{/if}
      {#if contentLoading}<div class="body-skeleton" aria-hidden="true"><span></span><span></span><span></span></div>
      {:else if content?.html && showHtml}<iframe class="html-body" class:designed title="Message content" sandbox="allow-popups allow-popups-to-escape-sandbox" referrerpolicy="no-referrer" srcdoc={frameDoc}></iframe>
      {:else}<div class="letter-body">{content?.text || message.body}</div>{/if}
    </article>
  {/if}
</section>
