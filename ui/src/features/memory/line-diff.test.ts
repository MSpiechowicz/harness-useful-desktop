import { describe, expect, it } from 'vitest';

import { type DiffRow, lineDiff } from './line-diff';

/** One line per row as a diff prints it: `+`, `-`, two spaces for kept lines and `…n` for folded ones. */
function printed(rows: DiffRow[]): string[] {
  return rows.map((row) => {
    if (row.kind === 'gap') {
      return `…${row.hidden}`;
    }

    return `${{ add: '+', remove: '-', same: ' ' }[row.kind]} ${row.text}`;
  });
}

describe('line diff', () => {
  it('has no rows for identical text', () => {
    expect(lineDiff('a\nb\n', 'a\nb\n')).toEqual({ rows: [], added: 0, removed: 0 });
    expect(lineDiff('', '')).toEqual({ rows: [], added: 0, removed: 0 });
  });

  it('marks a changed line as removed then added, and numbers each side', () => {
    const { rows, added, removed } = lineDiff('echo one\ncurl http://a\necho done\n', 'echo one\ncurl http://b\necho done\n');

    expect(printed(rows)).toEqual(['  echo one', '- curl http://a', '+ curl http://b', '  echo done']);
    expect([added, removed]).toEqual([1, 1]);
    expect(rows[1]).toMatchObject({ oldNo: 2, newNo: null });
    expect(rows[2]).toMatchObject({ oldNo: null, newNo: 2 });
    expect(rows[3]).toMatchObject({ oldNo: 3, newNo: 3 });
  });

  it('shows lines an agent or teammate appended or dropped', () => {
    expect(printed(lineDiff('a\nb\n', 'a\nb\nc\nd\n').rows)).toEqual(['  a', '  b', '+ c', '+ d']);
    expect(printed(lineDiff('a\nb\nc\n', 'b\n').rows)).toEqual(['- a', '  b', '- c']);
  });

  it('shows a whole new body as added and a deleted one as removed', () => {
    expect(printed(lineDiff('', 'rm -rf /tmp/x\n').rows)).toEqual(['+ rm -rf /tmp/x']);
    expect(printed(lineDiff('ls\n', '').rows)).toEqual(['- ls']);
  });

  it('keeps unchanged lines in order around a change in the middle, and a lone line the fold would only replace', () => {
    const before = ['a', 'b', 'c', 'd', 'e', 'f'].join('\n');
    const after = ['a', 'b', 'X', 'c', 'd', 'e', 'f'].join('\n');

    expect(printed(lineDiff(before, after).rows)).toEqual(['  a', '  b', '+ X', '  c', '  d', '  e', '  f']);
  });

  it('folds unchanged lines further than three lines from a change into one gap', () => {
    const before = Array.from({ length: 20 }, (_, index) => `line ${index + 1}`);
    const after = [...before];
    after[9] = 'changed';

    const { rows } = lineDiff(before.join('\n'), after.join('\n'));

    expect(printed(rows)).toEqual(['…6', '  line 7', '  line 8', '  line 9', '- line 10', '+ changed', '  line 11', '  line 12', '  line 13', '…7']);
  });

  it('treats a missing final newline like a present one', () => {
    expect(lineDiff('a\nb', 'a\nb\n')).toEqual({ rows: [], added: 0, removed: 0 });
  });

  it('replaces the whole text when it is too large to compare line by line', () => {
    const before = Array.from({ length: 2500 }, (_, index) => `old ${index}`).join('\n');
    const after = Array.from({ length: 2500 }, (_, index) => `new ${index}`).join('\n');

    const { added, removed, rows } = lineDiff(before, after);

    expect([added, removed]).toEqual([2500, 2500]);
    expect(rows.every((row) => row.kind !== 'same')).toBe(true);
  });

  it('splits on a lone carriage return too, so it cannot hide a line inside a comment', () => {
    const diff = lineDiff('x = 1  # note\n', 'x = 1  # note\rprint("CR-HIDDEN")\n');

    expect(printed(diff.rows)).toEqual(['  x = 1  # note', '+ print("CR-HIDDEN")']);
    expect(lineDiff('a\r\nb\r\n', 'a\nb\n')).toEqual({ rows: [], added: 0, removed: 0 });
  });
});
