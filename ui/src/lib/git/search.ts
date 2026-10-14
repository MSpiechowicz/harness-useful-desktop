import type { FileDiff } from '$lib/ipc/bindings/FileDiff';

/** A typed query, trimmed and lower-cased once; `null` when there is nothing to look for. */
export function normalizeQuery(query: string): string | null {
  const trimmed = query.trim().toLowerCase();

  return trimmed === '' ? null : trimmed;
}

/** The `[start, end)` spans of every case-insensitive occurrence of an already-normalized query in `text`. */
export function matchRanges(text: string, needle: string): [number, number][] {
  const lower = text.toLowerCase();
  const spans: [number, number][] = [];

  for (let from = lower.indexOf(needle); from !== -1; from = lower.indexOf(needle, from + needle.length)) {
    spans.push([from, from + needle.length]);
  }

  return spans;
}

/** One piece of a line: plain, or a stretch the search matched. */
export interface Piece {
  text: string;
  hit: boolean;
}

/** Cuts `content` (which starts at `offset` in its line) at the edges of the match spans. */
export function cutPieces(content: string, offset: number, spans: readonly [number, number][]): Piece[] {
  const pieces: Piece[] = [];
  let at = 0;

  for (const [start, end] of spans) {
    const from = Math.max(start - offset, at);
    const to = Math.min(end - offset, content.length);

    if (to <= from) {
      continue;
    }

    if (from > at) {
      pieces.push({ text: content.slice(at, from), hit: false });
    }

    pieces.push({ text: content.slice(from, to), hit: true });
    at = to;
  }

  if (at < content.length) {
    pieces.push({ text: content.slice(at), hit: false });
  }

  return pieces;
}

/** Flat indexes (as `flatLines` numbers them) of the lines of one diff whose text contains the query. */
export function matchingLines(diff: FileDiff, needle: string): number[] {
  const found: number[] = [];

  diff.hunks.flatMap((hunk) => hunk.lines).forEach((line, index) => {
    if (line.kind !== 'noNewline' && line.text.toLowerCase().includes(needle)) {
      found.push(index);
    }
  });

  return found;
}

/** What a query found in one changed file: whether the path matched and how many lines of its diff did. */
export interface FileHit {
  path: boolean;
  lines: number;
}
