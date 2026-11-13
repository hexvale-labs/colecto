// Client for the Host's owner-only AI routes (`/api/v1/ai/*`). Keys are write-only.
import { request } from '../../lib/api';

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

// Settings are Host controls; summaries and replies are the bundled `ai` extension's own calls.
export const aiHost = {
  settings: () => request<SettingsView>('host', 'demo', '/ai/settings'),
  save: (settings: AiSettings) => request<SettingsView>('host', 'demo', '/ai/settings', settings),
  key: (kind: 'text' | 'decision', key: string | null) => request<SettingsView>('host', 'demo', '/ai/keys', { kind, key }),
  test: (kind: 'text' | 'decision') => request<{ ok: true }>('host', 'demo', '/ai/test', { kind }),
  preview: (account: string, rule: LabelRule) => request<Preview>('host', account, '/ai/labels/preview', { rule }),
};
const path = (id: string) => `/ai/summaries/${encodeURIComponent(id)}`;
export const aiService = (account: string) => ({
  summary: (id: string) => request<{ summary: string | null }>('ai', account, path(id)),
  summarize: (id: string) => request<{ summary: string }>('ai', account, path(id), {}),
  forget: (id: string) => request<{ summary: null }>('ai', account, path(id), {}, 'DELETE'),
  reply: (message_id: string, input: string, tone: Tone) => request<{ text: string }>('ai', account, '/ai/reply', { message_id, input, tone }),
  labels: () => request<Record<string, string[]>>('ai', account, '/labels'),
});
