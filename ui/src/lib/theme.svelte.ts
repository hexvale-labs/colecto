export type Theme = 'dark' | 'light' | 'system';
function storedTheme(): Theme {
  try { const value = localStorage.getItem('colecto.theme'); if (value === 'light' || value === 'system') return value; } catch { /* Storage may be disabled. */ }
  return 'dark';
}
export const appearance = $state<{ theme: Theme }>({ theme: storedTheme() });
export function setTheme(value: Theme) {
  appearance.theme = value;
  try { localStorage.setItem('colecto.theme', value); } catch { /* Appearance still works without persistence. */ }
  applyTheme();
}
function applyTheme() {
  const resolved = appearance.theme === 'system' ? matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light' : appearance.theme;
  document.documentElement.dataset.theme = resolved;
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', resolved === 'dark' ? '#171b19' : '#fafaf8');
}
applyTheme();
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);
