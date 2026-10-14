import { describe, expect, it } from 'vitest';

import type { FileDiff } from '$lib/ipc/bindings/FileDiff';

import { expandGaps, skippedBefore } from './diff-expand';

function diff(): FileDiff {
  const line = (no: number, kind: 'context' | 'add' = 'context') => ({ kind, oldNo: kind === 'add' ? null : no, newNo: no, text: `t${no}` });

  return {
    path: 'a.txt',
    oldPath: null,
    status: 'modified',
    staged: false,
    isBinary: false,
    truncated: false,
    additions: 0,
    deletions: 0,
    hunks: [
      { header: '@@', oldStart: 1, oldLines: 2, newStart: 1, newLines: 2, lines: [line(1), line(2)] },
      { header: '@@', oldStart: 10, oldLines: 2, newStart: 10, newLines: 2, lines: [line(10), line(11)] },
    ],
  } as FileDiff;
}

describe('opening the unchanged lines between hunks', () => {
  const text = Array.from({ length: 20 }, (_, index) => `t${index + 1}`);

  it('counts the lines a hunk skips', () => {
    expect([skippedBefore(diff(), 0), skippedBefore(diff(), 1)]).toEqual([0, 7]);
  });

  it('fills the gap with numbered context lines and leaves the rest alone', () => {
    const input = diff();
    const opened = expandGaps(input, text, new Set([1]));

    expect(skippedBefore(opened, 1)).toBe(0);
    expect(opened.hunks[1].lines.slice(0, 7).map((line) => [line.oldNo, line.newNo, line.text])).toEqual([3, 4, 5, 6, 7, 8, 9].map((no) => [no, no, `t${no}`]));
    expect(opened.hunks[0]).toBe(input.hunks[0]);
    expect(input.hunks[1].lines).toHaveLength(2);
  });

  it('keeps the old numbers apart from the new ones when lines were added above', () => {
    const shifted = diff();
    shifted.hunks[1] = { ...shifted.hunks[1], oldStart: 8 };

    const opened = expandGaps(shifted, text, new Set([1]));

    expect(opened.hunks[1].lines[0]).toMatchObject({ oldNo: 1, newNo: 3 });
  });

  it('returns the same diff when nothing is open or the text is missing', () => {
    expect(expandGaps(diff(), null, new Set([1]))).toMatchObject({ path: 'a.txt' });
    const plain = diff();
    expect(expandGaps(plain, text, new Set())).toBe(plain);
  });
});
