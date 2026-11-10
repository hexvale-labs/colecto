// Mail dates arrive as RFC 2822, ISO 8601, or short display strings ("Nov 6", "Today").
export interface When { label: string; group: string; full: string }

const DAY = 86_400_000;
const startOfDay = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();

function parse(text: string, now: Date): { date: Date; timed: boolean } | null {
  // Appending a year makes V8 accept almost any word, so require a digit first.
  if (!/\d/.test(text)) return null;
  // Short strings without a year parse to 2001 in V8, so pin them to the most recent matching date.
  const yearless = !/\d{4}/.test(text);
  let value = Date.parse(yearless ? `${text} ${now.getFullYear()}` : text);
  if (Number.isNaN(value)) return null;
  if (yearless && value > now.getTime() + DAY) value = new Date(value).setFullYear(now.getFullYear() - 1);
  return { date: new Date(value), timed: /\d:\d\d/.test(text) };
}

export function when(text: string, now = new Date()): When {
  if (/^today$/i.test(text.trim())) return { label: 'Today', group: 'Today', full: 'Today' };
  const parsed = parse(text, now);
  if (!parsed) return { label: text, group: 'Earlier', full: text };
  const { date, timed } = parsed;
  const days = Math.round((startOfDay(now) - startOfDay(date)) / DAY);
  const sameYear = date.getFullYear() === now.getFullYear();
  const label = days === 0 && timed ? date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
    : days > 0 && days < 7 ? date.toLocaleDateString(undefined, { weekday: 'short' })
    : date.toLocaleDateString(undefined, sameYear ? { month: 'short', day: 'numeric' } : { month: 'short', day: 'numeric', year: 'numeric' });
  const group = days <= 0 ? 'Today' : days === 1 ? 'Yesterday' : days < 7 ? 'Previous 7 days'
    : date.toLocaleDateString(undefined, sameYear ? { month: 'long' } : { month: 'long', year: 'numeric' });
  const full = date.toLocaleString(undefined, timed ? { dateStyle: 'full', timeStyle: 'short' } : { dateStyle: 'full' });
  return { label, group, full };
}
