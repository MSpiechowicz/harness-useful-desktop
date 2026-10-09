import type { CommentSide } from '$lib/ipc/bindings/CommentSide';
import type { DiffLine } from '$lib/ipc/bindings/DiffLine';

/** The contract caps a comment's snippet at this many lines; a selection never grows past it. */
export const MAX_SNIPPET_LINES = 40;

/** A commented line range, ready to become a `ReviewComment` once it has a body. */
export interface RangeTarget {
  side: CommentSide;
  startLine: number;
  endLine: number;
  snippet: string[];
}

/** The line number a diff line has on one side: removals only on the old side, additions only on the new. */
export function lineNumber(line: DiffLine, side: CommentSide): number | null {
  return side === 'old' ? line.oldNo : line.newNo;
}

/**
 * The range spanned by two flat line indexes on one side. Lines without a number on that side
 * (the other side's additions or removals, "no newline" markers) are skipped, so a selection
 * dragged across a change still names real lines of the chosen side.
 */
export function rangeTarget(lines: readonly DiffLine[], from: number, to: number, side: CommentSide): RangeTarget | null {
  const low = Math.min(from, to);
  const high = Math.max(from, to);
  const picked: { no: number; text: string }[] = [];

  for (let index = low; index <= high && index < lines.length; index += 1) {
    const no = lineNumber(lines[index], side);
    if (no !== null && lines[index].kind !== 'noNewline') {
      picked.push({ no, text: lines[index].text });
    }
  }

  const kept = picked.slice(0, MAX_SNIPPET_LINES);
  if (kept.length === 0) {
    return null;
  }

  return { side, startLine: kept[0].no, endLine: kept[kept.length - 1].no, snippet: kept.map((entry) => entry.text) };
}

/** Flat index of the line carrying `lineNo` on `side`, or -1 when the diff does not show it. */
export function anchorIndex(lines: readonly DiffLine[], side: CommentSide, lineNo: number): number {
  return lines.findIndex((line) => line.kind !== 'noNewline' && lineNumber(line, side) === lineNo);
}

/** Flat indexes covered by a range, for highlighting the selection. */
export function coveredIndexes(lines: readonly DiffLine[], target: Pick<RangeTarget, 'side' | 'startLine' | 'endLine'>): Set<number> {
  const covered = new Set<number>();

  lines.forEach((line, index) => {
    const no = lineNumber(line, target.side);
    if (no !== null && line.kind !== 'noNewline' && no >= target.startLine && no <= target.endLine) {
      covered.add(index);
    }
  });

  return covered;
}

/** A saved comment as the diff shows it. */
export interface ThreadComment {
  id: number;
  side: CommentSide;
  startLine: number;
  endLine: number;
  body: string;
}

/** "Line 4 (new)" or "Lines 3–5 (old)". */
export function commentHeading(range: Pick<RangeTarget, 'side' | 'startLine' | 'endLine'>): string {
  const lines = range.startLine === range.endLine ? `Line ${range.startLine}` : `Lines ${range.startLine}–${range.endLine}`;

  return `${lines} (${range.side})`;
}

/** What a line-number gutter reports while the user picks a line or drags out a range. */
export interface PickHandlers {
  /** A gutter was pressed; `extend` (shift) grows the previous selection instead of starting a new one. */
  start: (index: number, side: CommentSide, extend: boolean) => void;
  /** The pointer entered another gutter while a range is being dragged. */
  extend: (index: number, side: CommentSide) => void;
  /** The selection is complete (pointer released, or a keyboard activation). */
  finish: () => void;
}
