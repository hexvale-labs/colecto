<script lang="ts">
  import CornerPicker from './CornerPicker.svelte';
  import type { NotificationPreferences } from './lib/api';
  import { notify } from './extensions/notifications/store.svelte';
  let { value, onchange }: { value: NotificationPreferences; onchange: (next: NotificationPreferences) => void } = $props();
  let note = $state('');
  const supported = typeof Notification === 'function';
  const set = (change: Partial<NotificationPreferences>) => onchange({ ...value, ...change });
  async function system(on: boolean) {
    note = '';
    if (on && supported && Notification.permission !== 'granted') {
      if (await Notification.requestPermission() !== 'granted') { note = 'Your system blocked notifications for Colecto. Allow them in its settings, then try again.'; return; }
    }
    set({ system: on });
  }
</script>

<section class="settings-section" aria-labelledby="notifications-title">
  <header class="section-head"><h2 id="notifications-title">Notifications</h2>
  <p>Colecto tells you about new mail and about automation waiting for your approval. Notifications always show in the window, including over Settings.</p></header>
  <div class="settings-card setting-row">
    <div><h3>Show notifications</h3><p>Turn every in-app and desktop notification on or off.</p></div>
    <label class="switch"><input type="checkbox" checked={value.enabled} onchange={(event) => set({ enabled: event.currentTarget.checked })} /><span class="sr-only">Show notifications</span></label>
  </div>
  <fieldset class="settings-stack" disabled={!value.enabled}>
    <legend class="sr-only">Notification types</legend>
    <div class="settings-card setting-row">
      <div><h3>New mail</h3><p>When a message arrives in any connected account.</p></div>
      <label class="switch"><input type="checkbox" checked={value.mail} onchange={(event) => set({ mail: event.currentTarget.checked })} /><span class="sr-only">New mail</span></label>
    </div>
    <div class="settings-card setting-row">
      <div><h3>Send approvals</h3><p>When automation asks to send a message. Approve or reject it right from the notification.</p></div>
      <label class="switch"><input type="checkbox" checked={value.approvals} onchange={(event) => set({ approvals: event.currentTarget.checked })} /><span class="sr-only">Send approvals</span></label>
    </div>
    <div class="settings-card setting-row">
      <div><h3>Desktop notifications</h3><p>{supported ? 'Also alert your desktop, but only while Colecto is in the background.' : 'This window cannot show desktop notifications.'}</p>{#if note}<p class="content-note" role="status">{note}</p>{/if}</div>
      <label class="switch"><input type="checkbox" checked={value.system} disabled={!supported} onchange={(event) => void system(event.currentTarget.checked)} /><span class="sr-only">Desktop notifications</span></label>
    </div>
    <div class="settings-card">
      <h3>Position</h3>
      <p>Pick where notifications appear in the window.</p>
      <CornerPicker value={value.position} disabled={!value.enabled} onchange={(position) => set({ position })} />
      <div class="button-row"><button type="button" class="secondary" onclick={() => notify({ title: 'This is where notifications appear', body: 'Pick another spot above to move them.' })}>Show a test notification</button></div>
    </div>
  </fieldset>
</section>
