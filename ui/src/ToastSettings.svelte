<script lang="ts">
  // Where toasts appear and whether they also reach the desktop. The notifications extension
  // decides what to announce; these settings belong to the core toast surface.
  import CornerPicker from './CornerPicker.svelte';
  import type { NotificationPreferences } from './lib/api';
  import { notify } from './shell/toast-store.svelte';
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

<div class="settings-group">
  <h3 class="group-label">How they appear</h3>
  <div class="setting setting-row">
    <div class="setting-text"><h4>Desktop notifications</h4><p>{supported ? 'Also alert your desktop, but only while Colecto is in the background.' : 'This window cannot show desktop notifications.'}</p>{#if note}<p class="content-note" role="status">{note}</p>{/if}</div>
    <label class="switch"><input type="checkbox" checked={value.system} disabled={!supported} onchange={(event) => void system(event.currentTarget.checked)} /><span class="sr-only">Desktop notifications</span></label>
  </div>
  <div class="setting">
    <div class="setting-text"><h4>Position</h4><p>Where notifications appear in the window.</p></div>
    <CornerPicker value={value.position} onchange={(position) => set({ position })} />
    <button type="button" class="text-button" onclick={() => notify({ title: 'This is where notifications appear', body: 'Pick another spot above to move them.' })}>Show a test notification</button>
  </div>
</div>
