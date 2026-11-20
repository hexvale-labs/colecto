/** Open a native modal `<dialog>` on mount and close it when the element is destroyed. */
export function modal(node: HTMLDialogElement) {
  node.showModal();
  return { destroy() { node.close(); } };
}

/** Live check for the user's reduced-motion preference. */
export const prefersReducedMotion = () =>
  typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
