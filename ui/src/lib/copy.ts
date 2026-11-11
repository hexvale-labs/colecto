/** Copy text and report whether it worked; the caller shows "Copied" briefly. */
export async function copyText(text: string) {
  try { await navigator.clipboard.writeText(text); return true; } catch { return false; }
}
