/** Extra space around text (Settings → Appearance → Text), shared by the app's styles and
 *  the mail frame. Each preference is a step; the largest match WCAG 1.4.12 Text Spacing. */
export interface TextSpacing { letter: number; word: number; line: number; paragraph: number }
export const NO_SPACING: TextSpacing = { letter: 0, word: 0, line: 0, paragraph: 0 };

/** Lengths per step: added letter and word spacing, a line-height multiplier, and space
 *  added between paragraphs on top of their usual gap. */
export const STEPS = {
  letter: ['0em', '0.06em', '0.12em'],
  word: ['0em', '0.08em', '0.16em'],
  line: ['1', '1.15', '1.3'],
  paragraph: ['0em', '0.5em', '1em'],
} as const;
const at = (kind: keyof typeof STEPS, step: number) => STEPS[kind][step] ?? STEPS[kind][0];

export const textSpacing = $state<TextSpacing>({ ...NO_SPACING });

/** Set the shared spacing and the CSS variables the stylesheet scales by. */
export function applySpacing(next: TextSpacing) {
  Object.assign(textSpacing, next);
  const root = document.documentElement.style;
  root.setProperty('--letter-spacing', at('letter', next.letter));
  root.setProperty('--word-spacing', at('word', next.word));
  root.setProperty('--line-scale', at('line', next.line));
  root.setProperty('--paragraph-gap', at('paragraph', next.paragraph));
}

/** The same spacing as CSS for a sealed mail frame, which cannot see the app's variables.
 *  Designed (sender-styled) mail keeps its own line heights; only plain mail scales them. */
export function frameSpacing(spacing: TextSpacing, designed: boolean) {
  const line = Number(at('line', spacing.line));
  return `body{letter-spacing:${at('letter', spacing.letter)};word-spacing:${at('word', spacing.word)}${designed ? '' : `;line-height:${(1.65 * line).toFixed(3)}`}}`
    + `p{margin-block:calc(1em + ${at('paragraph', spacing.paragraph)})}`;
}
