<script lang="ts">
  import { flip } from 'svelte/animate';
  import { quintOut } from 'svelte/easing';
  import { onMount } from 'svelte';
  import { extensionBus, extensionName } from '../../lib/bus';
  import Icon from '../../Icon.svelte';
  import { dismiss, notificationPosition, notify, toasts, type Toast } from './store.svelte';
  import { checkForUpdate } from './updates';
  const bus = extensionBus('notifications');
  let region: HTMLElement;
  let position = $derived(notificationPosition.value);
  const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  // The region lives in the browser's top layer, so notifications show over Settings and other
  // overlays. A modal dialog opened later goes above it; a new notification raises it again.
  const popover = () => typeof region?.showPopover === 'function';
  function raise() {
    if (!popover() || !region.isConnected) return;
    if (region.matches(':popover-open')) {
      if (!document.querySelector('dialog:modal')) return;
      region.hidePopover();
    }
    region.showPopover();
  }
  onMount(() => {
    raise();
    void checkForUpdate().catch(() => { /* An update check that fails stays quiet. */ });
    return bus.handle('v1/notify', async (p, from) => {
      if (typeof p?.title === 'string' && typeof p?.body === 'string') notify({ title: `${await extensionName(from)}: ${p.title}`, body: p.body });
      return null;
    });
  });
  let shown = 0;
  $effect(() => { const count = toasts.length; if (count > shown) raise(); shown = count; });
  // Enter and leave toward the edge the region is docked to. Only translate, scale and opacity
  // animate: Svelte keeps a leaving toast in place with a `transform`, which these compose with,
  // so it fades where it is while the others slide into its space.
  function offset(): [number, number] {
    return position === 'left' ? [-16, 0] : position === 'right' ? [16, 0] : position.startsWith('top') ? [0, -10] : [0, 10];
  }
  function slide(_node: HTMLElement, { duration }: { duration: number }) {
    const [x, y] = offset();
    return { duration: reduced ? 0 : duration, easing: quintOut, css: (t: number) => `opacity:${t};translate:${(1 - t) * x}px ${(1 - t) * y}px;scale:${0.97 + 0.03 * t}` };
  }
  // A toast waits while the pointer or focus is on it, so nobody loses a message mid-read.
  function timer(node: HTMLElement, toast: Toast) {
    if (!toast.duration) return;
    let left = toast.duration; let started = Date.now(); let handle = setTimeout(() => dismiss(toast.id), left);
    const pause = () => { clearTimeout(handle); left -= Date.now() - started; };
    const resume = () => { if (node.matches(':hover, :focus-within')) return; started = Date.now(); handle = setTimeout(() => dismiss(toast.id), Math.max(left, 1500)); };
    for (const name of ['pointerenter', 'focusin']) node.addEventListener(name, pause);
    for (const name of ['pointerleave', 'focusout']) node.addEventListener(name, resume);
    return { destroy() { clearTimeout(handle); } };
  }
  async function act(toast: Toast, run: () => unknown) { dismiss(toast.id); await run(); }
</script>

<section class="toasts" bind:this={region} popover="manual" data-position={position} aria-label="Notifications">
  {#each toasts as toast (toast.id)}
    <div class="toast" role={toast.duration === 0 ? 'alertdialog' : 'status'} aria-label={toast.title} animate:flip={{ duration: reduced ? 0 : 240, easing: quintOut }} in:slide={{ duration: 260 }} out:slide={{ duration: 180 }} use:timer={toast}>
      <span class="toast-mark" aria-hidden="true"><Icon name={toast.icon ?? (toast.duration === 0 ? 'shield' : 'mail')} size={15} /></span>
      <div class="toast-text"><strong>{toast.title}</strong>{#if toast.body}<p>{toast.body}</p>{/if}</div>
      <button class="toast-close" aria-label="Dismiss notification" onclick={() => dismiss(toast.id)}><Icon name="close" size={13} /></button>
      {#if toast.actions?.length}<div class="toast-actions">{#each toast.actions as action (action.label)}<button class={action.primary ? 'primary' : 'secondary'} onclick={() => void act(toast, action.run)}>{action.label}</button>{/each}</div>{/if}
    </div>
  {/each}
</section>
