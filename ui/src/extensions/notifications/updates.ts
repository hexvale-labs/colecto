// Once per app start, tell the owner about a newer stable Colecto release, once per version.
// The Host asks GitHub (the UI's content security policy only allows its own origin) and caches
// the answer; with update notifications off, nothing asks at all.
import { hostService } from '../../lib/api';
import { notify } from './store.svelte';

export async function checkForUpdate() {
  const saved = await hostService.preferences();
  if (!saved.notifications.enabled || !saved.notifications.updates) return;
  const { current, latest } = await hostService.latestRelease();
  if (!latest || saved.notified_release === latest.version) return;
  // In the desktop app, new windows open in the system browser.
  const url = latest.url;
  notify({ title: `Colecto ${latest.version} is available`, body: `You are using ${current}.`, icon: 'download', duration: 15000,
    actions: [{ label: 'View release', primary: true, run: () => window.open(url, '_blank', 'noopener') }] });
  await hostService.savePreferences({ notified_release: latest.version });
}
