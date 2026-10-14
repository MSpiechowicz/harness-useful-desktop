import type { DiffLine } from '$lib/ipc/bindings/DiffLine';
import type { FileDiff } from '$lib/ipc/bindings/FileDiff';

/** Changes whenever the diff's content does, so a quiet re-read that found nothing new keeps what the reader opened. */
export function diffKey(diff: FileDiff): string {
  return [diff.path, diff.staged, diff.additions, diff.deletions, ...diff.hunks.map((hunk) => `${hunk.oldStart},${hunk.oldLines},${hunk.newStart},${hunk.newLines}`)].join('|');
}

/** Unchanged lines between a hunk and what precedes it (the previous hunk, or the top of the file). */
export function skippedBefore(diff: FileDiff, hunk: number): number {
  const here = diff.hunks[hunk];
  const before = diff.hunks[hunk - 1];

  return Math.max(0, here.newStart - (before ? before.newStart + before.newLines : 1));
}

/**
 * The diff with the gap before each opened hunk filled in as context lines taken from the file's text, so the rest of
 * the view needs no idea that anything was expanded. The unchanged lines carry one offset between old and new numbers.
 */
export function expandGaps(diff: FileDiff, text: readonly string[] | null, opened: ReadonlySet<number>): FileDiff {
  if (text === null || opened.size === 0) {
    return diff;
  }

  const hunks = diff.hunks.map((hunk, index) => {
    const count = opened.has(index) ? skippedBefore(diff, index) : 0;
    if (count === 0) {
      return hunk;
    }

    const from = hunk.newStart - count;
    const offset = hunk.newStart - hunk.oldStart;
    const context: DiffLine[] = Array.from({ length: count }, (_, step) => ({
      kind: 'context',
      oldNo: from + step - offset,
      newNo: from + step,
      text: text[from + step - 1] ?? '',
    }));

    return {
      ...hunk,
      oldStart: hunk.oldStart - count,
      oldLines: hunk.oldLines + count,
      newStart: from,
      newLines: hunk.newLines + count,
      lines: [...context, ...hunk.lines],
    };
  });

  return { ...diff, hunks };
}
