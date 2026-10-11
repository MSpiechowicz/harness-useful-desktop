/**
 * Who made a model, ported from the Harness Dashboard (src/core/models.ts: normalizeModel, modelMaker) so a model
 * wears the same maker color in both apps whichever harness or router reported it.
 */
export type Maker = 'anthropic' | 'openai';

/** Normalizes a model id for maker detection: router prefixes, snapshot dates and dotted Claude versions go. */
export function normalizeModel(model: string | null | undefined): string {
  if (!model) {
    return 'unknown';
  }

  let m = model.trim().toLowerCase();
  m = m.replace(/\[[^\]]*\]$/, ''); // "claude-opus-4-6[1m]"
  m = m.replace(/^(us|eu|apac|global)\.(?=anthropic\.)/, '');
  m = m.replace(/^anthropic\./, '');
  m = m.slice(m.lastIndexOf('/') + 1); // "github-copilot/", "openrouter/anthropic/", "models/"
  m = m.replace(/@\d{8}$/, '').replace(/-v\d+:\d+$/, ''); // Bedrock's "-v1:0"

  if (m.startsWith('claude')) {
    m = m.replace(/-v\d+$/, ''); // "claude-3-5-sonnet-v2", but "deepseek-v3" keeps its version
  }

  m = m.replace(/-(\d{8})$/, ''); // date-suffixed snapshot ids

  if (m.startsWith('claude-')) {
    m = m.replaceAll('.', '-'); // "claude-opus-5.5" -> "claude-opus-5-5"
  }

  return m || 'unknown';
}

const MAKERS: [RegExp, Maker][] = [
  [/^claude/, 'anthropic'],
  [/^(gpt|o\d|codex|chatgpt)/, 'openai'],
];

/** Who made a model, whichever harness or plan it ran under; null for makers without a color of their own. */
export function modelMaker(model: string | null | undefined): Maker | null {
  const m = normalizeModel(model);

  return MAKERS.find(([re]) => re.test(m))?.[1] ?? null;
}
