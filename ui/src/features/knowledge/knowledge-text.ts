/** A run of text and whether it is a word the search looked for. */
export interface Piece {
  text: string;
  match: boolean;
}

/** One block of a chunk's text: a paragraph run, or a fenced code block kept as it was written. */
export interface Block {
  kind: 'text' | 'code';
  text: string;
}

const WORD = /[\p{L}\p{N}_]+/gu;

/** The words of a search, lowercase: the list matches on the start of a word, so a word of the text matches when it starts with one. */
export function queryWords(query: string): string[] {
  return [...new Set(query.toLowerCase().match(WORD) ?? [])];
}

/** `text` cut into the words that match `query` (case-insensitive, from the start of a word) and what lies between them. Plain strings only: render them as text. */
export function highlightPieces(text: string, query: string): Piece[] {
  const words = queryWords(query);

  if (words.length === 0) {
    return [{ text, match: false }];
  }

  const pieces: Piece[] = [];
  let from = 0;

  for (const found of text.matchAll(WORD)) {
    const start = found.index ?? 0;
    const lower = found[0].toLowerCase();

    if (!words.some((word) => lower.startsWith(word))) {
      continue;
    }

    if (start > from) {
      pieces.push({ text: text.slice(from, start), match: false });
    }

    pieces.push({ text: found[0], match: true });
    from = start + found[0].length;
  }

  if (from < text.length) {
    pieces.push({ text: text.slice(from), match: false });
  }

  return pieces;
}

/** A chunk's text as paragraphs and fenced code blocks (the fence lines themselves are dropped). An unclosed fence runs to the end. */
export function blocksOf(text: string): Block[] {
  const blocks: Block[] = [];
  let kind: Block['kind'] = 'text';
  let lines: string[] = [];

  const flush = () => {
    const body = lines.join('\n').replace(/^\n+|\s+$/g, '');

    if (body !== '') {
      blocks.push({ kind, text: body });
    }

    lines = [];
  };

  for (const line of text.split('\n')) {
    if (line.trimStart().startsWith('```')) {
      flush();
      kind = kind === 'text' ? 'code' : 'text';
      continue;
    }

    lines.push(line);
  }

  flush();

  return blocks;
}
