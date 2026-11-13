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

{#snippet toggle(name: string, description: string, checked: boolean, onchange: (on: boolean) => void, disabled = false)}
  <div class="setting setting-row">
    <div class="setting-text"><h3>{name}</h3><p>{description}</p></div>
    <label class="switch"><input type="checkbox" {checked} {disabled} onchange={(event) => onchange(event.currentTarget.checked)} /><span class="sr-only">{name}</span></label>
  </div>
{/snippet}

<section class="settings-section" aria-labelledby="notifications-title">
  <header class="section-head"><h2 id="notifications-title">Notifications</h2>
  <p>Colecto tells you about new mail, automation waiting for your approval and new releases. Notifications show over everything in the window, Settings included.</p></header>
  <div class="settings-group">
    {@render toggle('Show notifications', 'Turn every in-app and desktop notification on or off.', value.enabled, (enabled) => set({ enabled }))}
  </div>
  <fieldset class="settings-group" disabled={!value.enabled}>
    <legend class="group-label">Notify me about</legend>
    {@render toggle('New mail', 'When a message arrives in any connected account.', value.mail, (mail) => set({ mail }))}
    {@render toggle('Send approvals', 'When automation asks to send a message. Approve or reject it from the notification.', value.approvals, (approvals) => set({ approvals }))}
    {@render toggle('New updates', 'When a newer Colecto release is published. Off means Colecto never checks.', value.updates, (updates) => set({ updates }))}
  </fieldset>
  <fieldset class="settings-group" disabled={!value.enabled}>
    <legend class="group-label">Delivery</legend>
    <div class="setting setting-row">
      <div class="setting-text"><h3>Desktop notifications</h3><p>{supported ? 'Also alert your desktop, but only while Colecto is in the background.' : 'This window cannot show desktop notifications.'}</p>{#if note}<p class="content-note" role="status">{note}</p>{/if}</div>
      <label class="switch"><input type="checkbox" checked={value.system} disabled={!supported} onchange={(event) => void system(event.currentTarget.checked)} /><span class="sr-only">Desktop notifications</span></label>
    </div>
    <div class="setting">
      <div class="setting-text"><h3>Position</h3><p>Where notifications appear in the window.</p></div>
      <CornerPicker value={value.position} disabled={!value.enabled} onchange={(position) => set({ position })} />
      <button type="button" class="text-button" onclick={() => notify({ title: 'This is where notifications appear', body: 'Pick another spot above to move them.' })}>Show a test notification</button>
    </div>
  </fieldset>
</section>
