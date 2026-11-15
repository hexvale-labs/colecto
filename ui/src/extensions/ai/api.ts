// Types for the AI routes (`/api/v1/ai/*`). Keys are write-only.

export type Effort = 'none' | 'low' | 'medium' | 'high';
export type Field = 'sender' | 'subject' | 'body' | 'any';
export interface Rule { field: Field; pattern: string }
export interface LabelRule { enabled: boolean; model: boolean; description: string; rules: Rule[] }
export interface AiSettings {
  enabled: boolean;
  text: { base_url: string; model: string; max_tokens: number; effort: Effort | null };
  decision: { base_url: string; model: string; threshold: number };
  autolabel: boolean;
  /** Account id → local label name → rule. */
  labels: Record<string, Record<string, LabelRule>>;
}
export interface SettingsView { settings: AiSettings; keys: { text: boolean; decision: boolean } }
export interface Preview { errors: (string | null)[]; checked: number; count: number; matches: { subject: string; sender: string }[] }
export type Tone = 'casual' | 'lazy' | 'professional' | 'roast' | 'troll';

