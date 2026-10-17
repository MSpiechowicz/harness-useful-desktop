export type DiffKind = 'same' | 'add' | 'remove';

export interface DiffLine {
  kind: DiffKind;
  text: string;
  /** The 1-based line number in the old text, `null` for an added line. */
  oldNo: number | null;
  /** The 1-based line number in the new text, `null` for a removed line. */
  newNo: number | null;
}

/** A run of unchanged lines that the diff leaves out. */
export interface DiffGap {
  kind: 'gap';
  hidden: number;
}

export type DiffRow = DiffLine | DiffGap;

export interface LineDiff {
  rows: DiffRow[];
  added: number;
  removed: number;
}

/** Unchanged lines kept around each change. */
export const DIFF_CONTEXT = 3;

/** Largest table the middle part of a diff may fill (cells). A script holds at most a few thousand lines, so this is never reached by one; a bigger input shows as replaced as a whole. */
const MAX_CELLS = 4_000_000;

function linesOf(text: string): string[] {
  if (text === '') {
    return [];
  }

  // A lone carriage return starts a new line in a terminal, so it does here too: it cannot hide code inside a comment.
  const lines = text.split(/\r\n|\r|\n/);
  // A final newline ends the last line, it does not start an empty one.
  if (lines[lines.length - 1] === '') {
    lines.pop();
  }

  return lines;
}

/** Which lines of `before` and `after` the longest common subsequence keeps, as `[beforeIndex, afterIndex]` pairs. */
function commonPairs(before: string[], after: string[]): [number, number][] {
  const width = after.length + 1;
  const table = new Uint32Array((before.length + 1) * width);

  for (let row = before.length - 1; row >= 0; row -= 1) {
    for (let column = after.length - 1; column >= 0; column -= 1) {
      table[row * width + column] =
        before[row] === after[column] ? table[(row + 1) * width + column + 1] + 1 : Math.max(table[(row + 1) * width + column], table[row * width + column + 1]);
    }
  }

  const pairs: [number, number][] = [];
  let row = 0;
  let column = 0;

  while (row < before.length && column < after.length) {
    if (before[row] === after[column]) {
      pairs.push([row, column]);
      row += 1;
      column += 1;
    } else if (table[(row + 1) * width + column] >= table[row * width + column + 1]) {
      row += 1;
    } else {
      column += 1;
    }
  }

  return pairs;
}

/** Every line of both texts in order, each marked as kept, removed or added. */
function lineByLine(before: string[], after: string[]): DiffLine[] {
  let start = 0;
  while (start < before.length && start < after.length && before[start] === after[start]) {
    start += 1;
  }

  let end = 0;
  while (end < before.length - start && end < after.length - start && before[before.length - 1 - end] === after[after.length - 1 - end]) {
    end += 1;
  }

  const middleBefore = before.slice(start, before.length - end);
  const middleAfter = after.slice(start, after.length - end);
  const pairs = (middleBefore.length + 1) * (middleAfter.length + 1) <= MAX_CELLS ? commonPairs(middleBefore, middleAfter) : [];

  const lines: DiffLine[] = [];
  let oldNo = 1;
  let newNo = 1;

  const keep = (text: string): void => {
    lines.push({ kind: 'same', text, oldNo, newNo });
    oldNo += 1;
    newNo += 1;
  };
  const remove = (text: string): void => {
    lines.push({ kind: 'remove', text, oldNo, newNo: null });
    oldNo += 1;
  };
  const add = (text: string): void => {
    lines.push({ kind: 'add', text, oldNo: null, newNo });
    newNo += 1;
  };

  before.slice(0, start).forEach(keep);

  let row = 0;
  let column = 0;
  for (const [pairRow, pairColumn] of [...pairs, [middleBefore.length, middleAfter.length] as [number, number]]) {
    middleBefore.slice(row, pairRow).forEach(remove);
    middleAfter.slice(column, pairColumn).forEach(add);

    if (pairRow < middleBefore.length) {
      keep(middleBefore[pairRow]);
    }

    row = pairRow + 1;
    column = pairColumn + 1;
  }

  before.slice(before.length - end).forEach(keep);

  return lines;
}

/** Folds the unchanged lines that sit further than `context` lines from a change into one gap row each. */
function foldUnchanged(lines: DiffLine[], context: number): DiffRow[] {
  const near = new Array<boolean>(lines.length).fill(false);

  lines.forEach((line, index) => {
    if (line.kind === 'same') {
      return;
    }

    for (let around = Math.max(0, index - context); around <= Math.min(lines.length - 1, index + context); around += 1) {
      near[around] = true;
    }
  });

  // A gap row takes the room of the one line it would hide, so a lone line stays.
  lines.forEach((_, index) => {
    if (!near[index] && near[index - 1] !== false && near[index + 1] !== false) {
      near[index] = true;
    }
  });

  const rows: DiffRow[] = [];
  let hidden = 0;

  lines.forEach((line, index) => {
    if (near[index]) {
      if (hidden > 0) {
        rows.push({ kind: 'gap', hidden });
        hidden = 0;
      }
      rows.push(line);
    } else {
      hidden += 1;
    }
  });

  if (hidden > 0) {
    rows.push({ kind: 'gap', hidden });
  }

  return rows;
}

/** A unified line diff of two texts: what was removed, what was added and `context` unchanged lines around each change. Identical texts have no rows. */
export function lineDiff(before: string, after: string, context: number = DIFF_CONTEXT): LineDiff {
  const lines = lineByLine(linesOf(before), linesOf(after));
  const added = lines.filter((line) => line.kind === 'add').length;
  const removed = lines.filter((line) => line.kind === 'remove').length;

  return { rows: added + removed === 0 ? [] : foldUnchanged(lines, context), added, removed };
}
