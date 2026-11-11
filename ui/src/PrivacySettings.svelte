<script lang="ts">
  let { remoteImages, onchange }: { remoteImages: boolean; onchange: (next: boolean) => void } = $props();
</script>

<section class="settings-section" aria-labelledby="privacy-title">
  <header class="section-head"><h2 id="privacy-title">Privacy</h2>
  <p>Choose what Colecto fetches from other servers when you open a message.</p></header>
  <div class="settings-card setting-row">
    <div><h3>Load remote images automatically</h3><p>Senders can use remote images to see when and where you open their mail. When this is off, each message asks first.</p></div>
    <label class="switch"><input type="checkbox" checked={remoteImages} onchange={(event) => onchange(event.currentTarget.checked)} /><span class="sr-only">Load remote images automatically</span></label>
  </div>
</section>
