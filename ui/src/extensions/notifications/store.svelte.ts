// In-app notifications. Callers check the owner's notification preferences before adding one.
export interface ToastAction { label: string; run: () => unknown; primary?: boolean }
export interface Toast {
  id: number; title: string; body?: string; actions?: ToastAction[];
  /** Milliseconds before it leaves on its own; 0 keeps it until acted on or dismissed. */
  duration: number;
  /** Replaces an earlier toast with the same key, such as one approval request. */
  key?: string;
  /** An `Icon` name; defaults to mail, or a shield for toasts that wait to be acted on. */
  icon?: import('../../Icon.svelte').IconName;
}
const MAX = 4;
let next = 0;
export const toasts = $state<Toast[]>([]);

export function notify(toast: Omit<Toast, 'id' | 'duration'> & { duration?: number }) {
  if (toast.key) dismissKey(toast.key);
  const id = ++next;
  toasts.push({ duration: 6000, ...toast, id });
  if (toasts.length > MAX) toasts.splice(0, toasts.length - MAX);
  return id;
}
export function dismiss(id: number) {
  const index = toasts.findIndex(toast => toast.id === id);
  if (index >= 0) toasts.splice(index, 1);
}
export function dismissKey(key: string) {
  const index = toasts.findIndex(toast => toast.key === key);
  if (index >= 0) toasts.splice(index, 1);
}

export const notificationPosition = $state<{ value: import('../../lib/api').Corner }>({ value: 'top' });
