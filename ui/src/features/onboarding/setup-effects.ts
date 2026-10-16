/** What a line of the dashboard installer's side effects is about, for its icon. Anything unrecognised is `other`. */
export type EffectKind = 'stop' | 'path' | 'menu' | 'install' | 'register' | 'other';

export interface Effect {
  kind: EffectKind;
  /** The sentence up to where it reads well as a headline. */
  title: string;
  /** What is left of the sentence, for a quieter second line; empty when the sentence is kept whole. */
  detail: string;
}

/** The kind by what the sentence says, most specific first; the backend's wording is English, so this reads English only. */
function kindOf(text: string): EffectKind {
  if (/^(stops?|quits?|closes?)\b/i.test(text)) {
    return 'stop';
  }

  if (/\bPATH\b|\bshell profile\b/.test(text)) {
    return 'path';
  }

  if (/\b(applications? menu|launcher|desktop entry|shortcut)\b/i.test(text)) {
    return 'menu';
  }

  if (/^registers?\b/i.test(text)) {
    return 'register';
  }

  if (/^(installs?|copies|writes|downloads?)\b/i.test(text)) {
    return 'install';
  }

  return 'other';
}

/** Where a sentence of the backend breaks into a headline and an aside; anything else stays whole. */
const BREAKS = ['; ', ', if '] as const;

/** Splits a side effect into a headline and an aside (never dropping a character of it) and names its kind. */
export function describeEffect(text: string): Effect {
  const kind = kindOf(text);

  for (const mark of BREAKS) {
    const at = text.indexOf(mark);

    if (at > 0) {
      const detail = mark === ', if ' ? `If ${text.slice(at + mark.length)}` : text.slice(at + mark.length);

      return { kind, title: text.slice(0, at), detail: detail.charAt(0).toUpperCase() + detail.slice(1) };
    }
  }

  return { kind, title: text, detail: '' };
}

export interface Piece {
  text: string;
  /** A path or a command inside the sentence, shown in the code font. */
  code: boolean;
}

/** A sentence cut into plain text and the paths and `commands` in it. */
export function pieces(text: string): Piece[] {
  const found = text.split(/(`[^`]+`|~\/[\w./-]*[\w/-])/);

  return found
    .filter((part) => part !== '')
    .map((part) => (part.startsWith('`') ? { text: part.slice(1, -1), code: true } : { text: part, code: part.startsWith('~/') }));
}
