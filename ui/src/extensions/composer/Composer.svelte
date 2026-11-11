<script lang="ts">
  import { untrack } from 'svelte';
  import type { Draft, ComposerService, OutgoingAttachment } from '../../lib/api';
  import Icon from '../../Icon.svelte';
  import { copyText } from '../../lib/copy';
  let { connected, sender, draft, service, onback, ondirty, onbusy, onsaved }: {
    connected: boolean; sender: string; draft: Draft | null; service: ComposerService; onback: () => void;
    ondirty: (dirty: boolean) => void; onbusy: (busy: boolean) => void; onsaved: (draft: Draft) => void;
  } = $props();
  // The shell keys each editor instance; take a snapshot only when it opens.
  const initial = untrack(() => draft);
  let id = $state(initial?.id || undefined);
  let recipient = $state(initial?.recipient ?? '');
  let cc = $state(initial?.cc ?? '');
  let subject = $state(initial?.subject ?? '');
  let body = $state(initial?.body ?? '');
  // Threading and forward references come from the original message and are not edited.
  const thread = { ...(initial?.in_reply_to ? { in_reply_to: initial.in_reply_to, references: initial.references ?? '' } : {}), ...(initial?.forward_of ? { forward_of: initial.forward_of } : {}) };
  const heading = initial?.in_reply_to ? 'REPLY' : initial?.forward_of || initial?.subject?.startsWith('Fwd:') ? 'FORWARD' : 'NEW MESSAGE';
  let files = $state<(OutgoingAttachment & { size: number })[]>([]);
  const snapshot = () => JSON.stringify([recipient, cc, subject, body]);
  let baseline = $state(JSON.stringify([initial?.recipient ?? '', initial?.cc ?? '', initial?.subject ?? '', initial?.body ?? '']));
  let dirty = $derived(snapshot() !== baseline || files.length > 0);
  const MAX_FILES = 25 * 1024 * 1024;
  function attach(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const chosen = Array.from(input.files ?? []);
    input.value = ''; error = '';
    if (files.reduce((total, file) => total + file.size, 0) + chosen.reduce((total, file) => total + file.size, 0) > MAX_FILES) { error = 'Attachments must total at most 25 MB.'; return; }
    for (const file of chosen) {
      const reader = new FileReader();
      reader.onload = () => { files = [...files, { name: file.name, content_type: file.type || 'application/octet-stream', data: String(reader.result).split(',')[1] ?? '', size: file.size }]; };
      reader.onerror = () => { error = `Could not read ${file.name}.`; };
      reader.readAsDataURL(file);
    }
  }
  let copied = $state<boolean | null>(null);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  async function copy() {
    copied = await copyText(body);
    clearTimeout(copiedTimer); copiedTimer = setTimeout(() => copied = null, 1600);
  }
  let saving = $state(false);
  let saved = $state(false);
  let sending = $state(false);
  let sent = $state(false);
  let error = $state('');
  $effect(() => { ondirty(dirty); });
  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    saving = true; onbusy(true); error = ''; saved = false;
    try {
      const result = await service.save({ id, recipient, ...(cc ? { cc } : {}), subject, body, ...thread });
      id = result.id;
      baseline = snapshot();
      saved = true; ondirty(false); onsaved(result);
    } catch (e) { error = (e as Error).message; }
    finally { saving = false; onbusy(false); }
  }
  async function send() {
    if (saving || sent) return;
    saving = true; sending = true; onbusy(true); error = '';
    try {
      await service.send({ recipient, ...(cc ? { cc } : {}), subject, body, ...thread, ...(files.length ? { attachments: files.map(({ name, content_type, data }) => ({ name, content_type, data })) } : {}) });
      sent = true; baseline = snapshot(); files = []; ondirty(false);
    } catch (e) { error = (e as Error).message; }
    finally { saving = false; sending = false; onbusy(false); }
  }

</script>

<section class="detail composer" aria-label="Draft editor">
  <form class="compose-form" onsubmit={save}>
    <header class="detail-toolbar">
      <button class="icon-button back" type="button" aria-label="Back to list" disabled={saving} onclick={onback}><Icon name="back" size={18} /></button>
      <h2>{id ? 'Your draft' : heading === 'REPLY' ? 'Reply' : heading === 'FORWARD' ? 'Forward' : 'New message'}</h2>
    </header>
    <fieldset class="compose-fields" disabled={saving || sent}>
      <div class="input-row"><span class="row-label">From</span><strong class="sender">{sender}</strong></div>
      <div class="input-row"><label for="recipient">To</label><input id="recipient" autocomplete="email" autocapitalize="none" spellcheck={false} enterkeyhint="next" type="text" inputmode="email" placeholder="name@example.com" maxlength="4000" bind:value={recipient} /></div>
      <div class="input-row"><label for="cc">Cc</label><input id="cc" autocomplete="email" autocapitalize="none" spellcheck={false} type="text" inputmode="email" maxlength="4000" bind:value={cc} /></div>
      <div class="input-row"><label for="subject">Subject</label><input id="subject" maxlength="998" bind:value={subject} /></div>
      <label class="sr-only" for="body">Message</label><textarea id="body" placeholder="Write your message" maxlength="100000" bind:value={body}></textarea>
      {#if connected && (files.length || thread.forward_of)}<div class="compose-attachments">
        {#if thread.forward_of}<p class="content-note">The original message’s attachments are included when you send.</p>{/if}
        {#if files.length}<ul class="attachments" aria-label="Files to send">{#each files as file, index (index)}<li><span class="attachment"><Icon name="attach" /><span class="attachment-name">{file.name}</span><button class="icon-button" aria-label={`Remove ${file.name}`} onclick={() => files = files.filter((_, other) => other !== index)} type="button"><Icon name="close" size={13} /></button></span></li>{/each}</ul><p class="content-note">Attached files are sent but not saved with drafts.</p>{/if}
      </div>{/if}
    </fieldset>
    {#if error}<p class="error-box" role="alert">{error}</p>{/if}
    <footer class="compose-footer">
      {#if connected}<label class="tool attach-button" class:disabled={saving || sent}><Icon name="attach" /><span>Attach files</span><input class="sr-only" type="file" multiple disabled={saving || sent} onchange={attach} /></label>{/if}
      <button class="tool" type="button" aria-label="Copy message text" title="Copy message text" disabled={!body} onclick={copy}><Icon name={copied ? 'check' : 'copy'} /><span>{copied === null ? 'Copy' : copied ? 'Copied' : 'Copy failed'}</span></button>
      <span class="save-state" class:pending={dirty && !saving && !sent} role="status">{sent ? 'Accepted by mail server' : sending ? 'Sending…' : saving ? 'Saving draft…' : dirty ? 'Unsaved changes' : saved ? 'Draft saved' : id ? 'Saved by your mail service' : connected ? 'Not saved yet' : 'Sending is unavailable for this account'}</span>
      <button class="secondary" data-command="save" type="submit" disabled={saving || sent || (!!id && !dirty)}>{saving && !sending ? 'Saving…' : 'Save draft'}</button>
      {#if connected}<button class="primary" type="button" disabled={saving || sent} data-command="send" onclick={send}>{sent ? 'Sent' : sending ? 'Sending…' : 'Send'}<Icon name="sent" size={14} /></button>{/if}
    </footer>
  </form>
</section>
