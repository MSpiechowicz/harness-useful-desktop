/** The opening tag of a harness notice, e.g. `<system-notice>`: lowercase words joined by `-` or `_`. */
const OPENING = /^<([a-z][\w-]*)>/i;

/** The line a harness notice ends its headline with to tell the agent what to do; it means nothing to the reader. */
const AGENT_CUE = /^resume your work using the result below\.?$/i;

export interface Unwrapped {
  /** The tag's name in words ("System notice"), or null when the text is not wrapped. */
  title: string | null;
  /** The first line (a wrapped notice) or the whole text (anything else). */
  body: string;
  /** What follows the first line of a wrapped notice (command output, an error), or null. */
  details: string | null;
}

/** Splits `text` after its first line; the agent cue after the headline is dropped. */
function headline(text: string): { first: string; rest: string | null } {
  const [first = '', ...lines] = text.split('\n');
  const sentences = first.split(/(?<=\.)\s+/);
  const head = sentences.filter((sentence) => !AGENT_CUE.test(sentence)).join(' ');
  const rest = lines.join('\n').trim();

  return { first: head, rest: rest === '' ? null : rest };
}

/**
 * Takes the wrapper tag off a harness notice, so the chat shows a title, its headline and its details apart instead of the raw tags.
 * Only a text that is one block, from its opening tag to the matching closing one, is a notice. Read in one pass: the text can hold any amount of output.
 */
export function unwrapNotice(text: string): Unwrapped {
  const trimmed = text.trim();
  const opening = OPENING.exec(trimmed);
  const closing = opening ? `</${opening[1]}>` : null;

  if (!opening || closing === null || !trimmed.endsWith(closing) || trimmed.length < opening[0].length + closing.length) {
    return { title: null, body: text, details: null };
  }

  const inside = trimmed.slice(opening[0].length, trimmed.length - closing.length);

  // Another opening tag of the same name inside is not one block.
  if (inside.includes(closing)) {
    return { title: null, body: text, details: null };
  }

  const words = opening[1].replace(/[-_]+/g, ' ');
  const { first, rest } = headline(inside.trim());

  return { title: words.charAt(0).toUpperCase() + words.slice(1), body: first, details: rest };
}
