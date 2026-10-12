import { describe, expect, it } from 'vitest';

import type { DiffLine } from '$lib/ipc/bindings/DiffLine';
import { i18n } from '$lib/i18n/i18n.svelte';
import { fixtures } from '$lib/ipc/mock/fixtures';

import { anchorIndex, commentHeading, coveredIndexes, MAX_SNIPPET_LINES, rangeTarget } from './comment-range';
import { flatLines } from './diff-rows';

/** context 1/1, remove 2, add 2, add 3, context 3/4 */
const LINES: DiffLine[] = [
  { kind: 'context', oldNo: 1, newNo: 1, text: 'one' },
  { kind: 'remove', oldNo: 2, newNo: null, text: 'two' },
  { kind: 'add', oldNo: null, newNo: 2, text: 'TWO' },
  { kind: 'add', oldNo: null, newNo: 3, text: 'extra' },
  { kind: 'context', oldNo: 3, newNo: 4, text: 'three' },
];

describe('rangeTarget', () => {
  it('names a single line on the side that was picked', () => {
    expect(rangeTarget(LINES, 2, 2, 'new')).toEqual({ side: 'new', startLine: 2, endLine: 2, snippet: ['TWO'] });
    expect(rangeTarget(LINES, 1, 1, 'old')).toEqual({ side: 'old', startLine: 2, endLine: 2, snippet: ['two'] });
  });

  it('spans a dragged range in either direction, skipping lines the side does not have', () => {
    const forward = rangeTarget(LINES, 0, 4, 'new');
    const backward = rangeTarget(LINES, 4, 0, 'new');

    expect(forward).toEqual({ side: 'new', startLine: 1, endLine: 4, snippet: ['one', 'TWO', 'extra', 'three'] });
    expect(backward).toEqual(forward);
    expect(rangeTarget(LINES, 0, 4, 'old')).toEqual({ side: 'old', startLine: 1, endLine: 3, snippet: ['one', 'two', 'three'] });
  });

  it('has no target when the range holds no line of that side', () => {
    expect(rangeTarget(LINES, 1, 1, 'new')).toBeNull();
    expect(rangeTarget(LINES, 2, 3, 'old')).toBeNull();
  });

  it('never holds more lines than the review contract allows', () => {
    const many: DiffLine[] = Array.from({ length: 100 }, (_, index) => ({ kind: 'add', oldNo: null, newNo: index + 1, text: `line ${index + 1}` }));

    const target = rangeTarget(many, 0, 99, 'new');

    expect(target?.snippet).toHaveLength(MAX_SNIPPET_LINES);
    expect(target?.endLine).toBe(MAX_SNIPPET_LINES);
  });
});

describe('anchors and highlights', () => {
  it('finds the line a comment ends on, per side', () => {
    expect(anchorIndex(LINES, 'new', 2)).toBe(2);
    expect(anchorIndex(LINES, 'old', 2)).toBe(1);
    expect(anchorIndex(LINES, 'new', 99)).toBe(-1);
  });

  it('covers exactly the lines of the range that exist on the side', () => {
    expect([...coveredIndexes(LINES, { side: 'new', startLine: 2, endLine: 3 })]).toEqual([2, 3]);
    expect([...coveredIndexes(LINES, { side: 'old', startLine: 1, endLine: 3 })]).toEqual([0, 1, 4]);
  });

  it('describes a range for people', () => {
    expect(commentHeading({ side: 'new', startLine: 4, endLine: 4 })).toBe('Line 4 (new)');
    expect(commentHeading({ side: 'old', startLine: 3, endLine: 5 })).toBe('Lines 3–5 (old)');
  });

  it('describes a range in the chosen language', async () => {
    await i18n.load('pl');

    expect(commentHeading({ side: 'new', startLine: 4, endLine: 4 })).toBe('Linia 4 (po)');
    expect(commentHeading({ side: 'old', startLine: 3, endLine: 5 })).toBe('Linie 3–5 (przed)');
  });

  it('resolves every line of the sample diffs back to its numbers', () => {
    for (const diff of fixtures.git.diff) {
      const lines = flatLines(diff);

      lines.forEach((line, index) => {
        if (line.newNo !== null) {
          expect(anchorIndex(lines, 'new', line.newNo)).toBe(index);
        }
      });
    }
  });
});
