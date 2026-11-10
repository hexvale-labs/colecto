const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
export function initial(name: string) {
  return segmenter.segment(name.trim())[Symbol.iterator]().next().value?.segment.toLocaleUpperCase() || '?';
}
/** A stable hue per correspondent, so the same sender is recognisable at a glance. */
export function hue(name: string) {
  let hash = 0;
  for (const char of name.trim().toLowerCase()) hash = (hash * 31 + char.codePointAt(0)!) >>> 0;
  return hash % 360;
}
