import { t } from '$lib/i18n/i18n.svelte';

/** A chat's card text: a short name and a one-line description of what it is about. */
export interface ChatLabel {
  name: string;
  description: string;
}

export const NAME_MAX = 40;
export const DESCRIPTION_MAX = 110;
/** How much of the first message the naming helper sees: enough to name a chat, little enough to cost almost nothing. */
const PROMPT_EXCERPT_MAX = 500;

/** The placeholder a new chat is called until it has something to be named after: a creature, not a number. */
export function generatedName(number: number): string {
  const names = t('chat.names.list')
    .split(',')
    .map((name) => name.trim())
    .filter(Boolean);
  const index = Math.max(number - 1, 0);
  const round = Math.floor(index / names.length);

  return round === 0 ? names[index] : `${names[index % names.length]} ${round + 1}`;
}

/** One line of plain words: line breaks, markdown marks and runs of spaces collapsed. */
export function collapse(text: string): string {
  return text
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/[`*_#>]+/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

/** `text` cut at a word boundary within `max` characters, with an ellipsis when something was cut. */
export function crop(text: string, max: number): string {
  if (text.length <= max) {
    return text;
  }

  const cut = text.slice(0, max - 1);
  const space = cut.lastIndexOf(' ');

  return `${(space > max / 2 ? cut.slice(0, space) : cut).replace(/[\s,;:.\-–]+$/, '')}…`;
}

/** The name and description a first message gives a chat without any model: its opening words, and more of the same. */
export function labelFromPrompt(prompt: string): ChatLabel | null {
  const text = collapse(prompt);
  if (text === '' || text.startsWith('/') || text.startsWith('!')) {
    return null;
  }

  const name = crop(text, NAME_MAX);

  return { name, description: text === name ? '' : crop(text, DESCRIPTION_MAX) };
}

/** The one request that asks a helper agent to name a chat: fixed instructions and a short excerpt of the first message. */
export function namingPrompt(prompt: string): string {
  return [
    'Name this chat from its first message. Reply with JSON only, no other text, no tools:',
    `{"name": "2-4 words, at most ${NAME_MAX} characters", "description": "one sentence, at most ${DESCRIPTION_MAX} characters"}`,
    'Use the language of the message.',
    '',
    'Message:',
    crop(collapse(prompt), PROMPT_EXCERPT_MAX),
  ].join('\n');
}

/** The label in a helper's reply, tolerating fences and chatter around the JSON; null when it is not one. */
export function parseLabel(reply: string): ChatLabel | null {
  const start = reply.indexOf('{');
  const end = reply.lastIndexOf('}');
  if (start === -1 || end <= start) {
    return null;
  }

  try {
    const value: unknown = JSON.parse(reply.slice(start, end + 1));
    if (typeof value !== 'object' || value === null) {
      return null;
    }

    const { name, description } = value as Record<string, unknown>;
    const cleanName = typeof name === 'string' ? collapse(name) : '';
    if (cleanName === '') {
      return null;
    }

    return { name: crop(cleanName, NAME_MAX), description: typeof description === 'string' ? crop(collapse(description), DESCRIPTION_MAX) : '' };
  } catch {
    return null;
  }
}
