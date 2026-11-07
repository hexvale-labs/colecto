<script lang="ts">
  import { untrack } from 'svelte';
  import type { Draft, composerService } from '../lib/api';
  let { connected, draft, service, onback, ondirty, onbusy, onsaved }: {
    connected: boolean; draft: Draft | null; service: typeof composerService; onback: () => void;
    ondirty: (dirty: boolean) => void; onbusy: (busy: boolean) => void; onsaved: (draft: Draft) => void;
  } = $props();
  // The shell keys each editor instance; take a snapshot only when it opens.
  const initial = untrack(() => draft);
  let id = $state(initial?.id);
  let recipient = $state(initial?.recipient ?? '');
  let subject = $state(initial?.subject ?? '');
  let body = $state(initial?.body ?? '');
  let baseline = $state(JSON.stringify([initial?.recipient ?? '', initial?.subject ?? '', initial?.body ?? '']));
  let dirty = $derived(JSON.stringify([recipient, subject, body]) !== baseline);
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
      const result = await service.save({ id, recipient, subject, body });
      id = result.id;
      baseline = JSON.stringify([recipient, subject, body]);
      saved = true; ondirty(false); onsaved(result);
    } catch (e) { error = (e as Error).message; }
    finally { saving = false; onbusy(false); }
  }
  async function send() {
    if (saving || sent) return;
    saving = true; sending = true; onbusy(true); error = '';
    try {
      await service.send({ recipient, subject, body });
      sent = true; baseline = JSON.stringify([recipient, subject, body]); ondirty(false);
    } catch (e) { error = (e as Error).message; }
    finally { saving = false; sending = false; onbusy(false); }
  }

</script>
<section class="detail composer" aria-label="Draft editor">
  <header class="detail-toolbar"><button class="text-button back" disabled={saving} onclick={onback}>← Back to list</button><span class="toolbar-label">A THOUGHT IN PROGRESS</span><span class="pill">Draft</span></header>
  <form class="compose-form" onsubmit={save}>
    <div class="compose-heading"><div class="eyebrow">MAKE A LITTLE CONNECTION</div><h2>{id ? 'Your draft' : 'A fresh page.'}</h2><p>Take your time. Find the words.</p></div>
    <fieldset disabled={saving || sent}>
      <div class="input-row"><label for="recipient">To</label><input id="recipient" autocomplete="email" autocapitalize="none" type="text" inputmode="email" placeholder="someone@example.com" maxlength="320" bind:value={recipient} /></div>
      <div class="input-row"><label for="subject">Subject</label><input id="subject" placeholder="What’s on your mind?" maxlength="998" bind:value={subject} /></div>
      <label class="sr-only" for="body">Message</label><textarea id="body" placeholder="Start a conversation…" maxlength="100000" bind:value={body}></textarea>
    </fieldset>
    {#if error}<p class="error-box" role="alert">{error}</p>{/if}
    <footer class="compose-footer"><div><span class="save-state" role="status">{sent ? 'Accepted by mail server' : sending ? 'Sending…' : saving ? 'Saving draft…' : dirty ? 'Unsaved changes' : saved ? 'Draft saved' : id ? 'Saved on this computer' : 'Not saved yet'}</span><p>{connected ? 'IMAP · SMTP · POP3 will not be supported' : 'Local demo · sending is unavailable'}</p></div><button class="primary" type="submit" disabled={saving || sent || (!!id && !dirty)}>{saving ? 'Saving…' : 'Save draft'}</button>{#if connected}<button class="primary" type="button" disabled={saving || sent} onclick={send}>{sent ? 'Sent' : sending ? 'Sending…' : 'Send'}</button>{/if}</footer>
  </form>
</section>
