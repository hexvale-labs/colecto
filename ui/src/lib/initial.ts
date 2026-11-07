const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
export function initial(name: string) {
  return segmenter.segment(name.trim())[Symbol.iterator]().next().value?.segment.toLocaleUpperCase() || '?';
}
