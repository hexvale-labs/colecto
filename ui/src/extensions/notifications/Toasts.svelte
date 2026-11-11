<script lang="ts">
  import { flip } from 'svelte/animate';
  import Icon from '../../Icon.svelte';
  import { dismiss, toasts, type Toast } from './store.svelte';
  import type { Corner } from '../../lib/api';
  import { notificationPosition } from './store.svelte';
  let position = $derived(notificationPosition.value);
  const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  // Leave toward the edge the toast came from, quicker than it arrived.
  function leave(node: HTMLElement) {
    const [x, y] = position === 'left' ? [-16, 0] : position === 'right' ? [16, 0] : position.startsWith('top') ? [0, -10] : [0, 10];
    return { duration: reduced ? 0 : 160, easing: (t: number) => 1 - (1 - t) ** 3, css: (t: number) => `opacity:${t};transform:translate(${(1 - t) * x}px,${(1 - t) * y}px) scale(${0.97 + 0.03 * t})` };
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

<section class="toasts" data-position={position} aria-label="Notifications">
  {#each toasts as toast (toast.id)}
    <div class="toast" role={toast.duration === 0 ? 'alertdialog' : 'status'} aria-label={toast.title} animate:flip={{ duration: reduced ? 0 : 220 }} out:leave use:timer={toast}>
      <span class="toast-mark" aria-hidden="true"><Icon name={toast.duration === 0 ? 'shield' : 'mail'} size={15} /></span>
      <div class="toast-text"><strong>{toast.title}</strong>{#if toast.body}<p>{toast.body}</p>{/if}</div>
      <button class="toast-close" aria-label="Dismiss notification" onclick={() => dismiss(toast.id)}><Icon name="close" size={13} /></button>
      {#if toast.actions?.length}<div class="toast-actions">{#each toast.actions as action (action.label)}<button class={action.primary ? 'primary' : 'secondary'} onclick={() => void act(toast, action.run)}>{action.label}</button>{/each}</div>{/if}
    </div>
  {/each}
</section>
