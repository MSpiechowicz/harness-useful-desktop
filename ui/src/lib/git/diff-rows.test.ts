import { describe, expect, it } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import type { DiffLine } from '$lib/ipc/bindings/DiffLine';
import type { FileDiff } from '$lib/ipc/bindings/FileDiff';
import { fixtures } from '$lib/ipc/mock/fixtures';

import { flatLines, lineSpoken, rowIndexes, splitRows, unifiedRows } from './diff-rows';

function line(kind: DiffLine['kind'], oldNo: number | null, newNo: number | null, text = ''): DiffLine {
  return { kind, oldNo, newNo, text };
}

function diffOf(...hunks: DiffLine[][]): FileDiff {
  return {
    path: 'a.txt',
    oldPath: null,
    status: 'modified',
    staged: false,
    isBinary: false,
    truncated: false,
    additions: 0,
    deletions: 0,
    hunks: hunks.map((lines) => ({ header: '@@', oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines })),
  };
}

describe('unified rows', () => {
  it('puts a header row before every hunk and numbers lines in file order', () => {
    const diff = diffOf([line('context', 1, 1), line('add', null, 2)], [line('remove', 9, null)]);

    const rows = unifiedRows(diff);

    expect(rows.map((row) => row.kind)).toEqual(['hunk', 'line', 'line', 'hunk', 'line']);
    expect(rows.flatMap((row) => (row.kind === 'line' ? [row.index] : []))).toEqual([0, 1, 2]);
  });

  it('shows a "no newline" marker as a note, not as a numbered line', () => {
    const rows = unifiedRows(diffOf([line('remove', 3, null, 'x'), line('noNewline', null, null), line('add', null, 3, 'x')]));

    expect(rows.map((row) => row.kind)).toEqual(['hunk', 'line', 'note', 'line']);
    // The marker still occupies a slot in the flat order, so later lines keep their index.
    expect(rows.flatMap((row) => (row.kind === 'line' ? [row.index] : []))).toEqual([0, 2]);
  });

  it('indexes every line of the sample diff consistently with the flat order', () => {
    for (const diff of fixtures.git.diff) {
      const flat = flatLines(diff);
      const rows = unifiedRows(diff);

      for (const row of rows) {
        if (row.kind === 'line') {
          expect(flat[row.index]).toBe(row.line);
        }
      }
    }
  });
});

describe('split rows', () => {
  it('faces a removed line with the added line that replaces it', () => {
    const rows = splitRows(diffOf([line('context', 1, 1, 'keep'), line('remove', 2, null, 'old'), line('add', null, 2, 'new')]));

    const pairs = rows.filter((row) => row.kind === 'pair');

    expect(pairs).toHaveLength(2);
    expect(pairs[0].left?.line.text).toBe('keep');
    expect(pairs[0].right?.line.text).toBe('keep');
    expect(pairs[1].left?.line.text).toBe('old');
    expect(pairs[1].right?.line.text).toBe('new');
  });

  it('leaves the other side empty for unmatched additions and removals', () => {
    const rows = splitRows(diffOf([line('add', null, 1, 'a'), line('add', null, 2, 'b'), line('remove', 1, null, 'c')]));

    const pairs = rows.filter((row) => row.kind === 'pair');

    expect(pairs.map((row) => [row.left?.line.text ?? null, row.right?.line.text ?? null])).toEqual([
      [null, 'a'],
      [null, 'b'],
      ['c', null],
    ]);
  });

  it('reports every flat index a row displays, once', () => {
    const rows = splitRows(diffOf([line('context', 1, 1), line('remove', 2, null), line('add', null, 2)]));
    const pairs = rows.filter((row) => row.kind === 'pair');

    expect(rowIndexes(pairs[0])).toEqual([0]);
    expect(rowIndexes(pairs[1])).toEqual([1, 2]);
  });
});

describe('spoken line markers', () => {
  it('say added and removed in the current language', async () => {
    expect([lineSpoken('add'), lineSpoken('remove'), lineSpoken('context')]).toEqual(['Added: ', 'Removed: ', '']);

    await i18n.load('pl');

    expect([lineSpoken('add'), lineSpoken('remove'), lineSpoken('context')]).toEqual(['Dodano: ', 'Usunięto: ', '']);
  });
});
