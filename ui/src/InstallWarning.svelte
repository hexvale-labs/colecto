<script lang="ts">
  import Icon from './Icon.svelte';
  let { name, onconfirm, oncancel }: { name: string; onconfirm: () => void; oncancel: () => void } = $props();
  // A short, unskippable pause, so the warning is read rather than clicked through.
  let left = $state(5);
  function show(node: HTMLDialogElement) {
    node.showModal();
    const timer = setInterval(() => { left -= 1; if (left <= 0) clearInterval(timer); }, 1000);
    return { destroy() { clearInterval(timer); node.close(); } };
  }
</script>

<dialog class="install-warning" aria-labelledby="install-warning-title" aria-describedby="install-warning-text" use:show oncancel={(event) => { event.preventDefault(); oncancel(); }}>
  <span class="warning-mark" aria-hidden="true"><Icon name="alert" size={22} /></span>
  <h2 id="install-warning-title">Only install extensions you trust</h2>
  <div id="install-warning-text" class="warning-text">
    <p>You are about to install <strong>{name}</strong>.</p>
    <ul>
      <li>Extensions run in a sandbox and must ask before reading your mail, but an extension can still lie about what it is for. Its name, author and description come from whoever made it, and Colecto cannot verify them.</li>
      <li>Anything you let it read, it can collect and keep, and it can misuse what it is given.</li>
      <li>Prefer open-source extensions from authors you know, where others can read the code. If you are unsure, do not install it.</li>
    </ul>
    <p>It installs turned off, with no access to your accounts, until you allow it.</p>
  </div>
  <div class="warning-actions">
    <!-- svelte-ignore a11y_autofocus (the safe choice takes focus in a warning) -->
    <button class="secondary" autofocus onclick={oncancel}>Cancel</button>
    <button class="danger" disabled={left > 0} onclick={onconfirm}>{left > 0 ? `Install (${left})` : 'Install anyway'}</button>
  </div>
</dialog>
