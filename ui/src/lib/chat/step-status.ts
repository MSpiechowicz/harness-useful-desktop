/** Where the steps status sits in a text block of a reply. */
export interface StepStatusParts {
  /** The text before the table. */
  before: string;
  /** The table as Markdown, empty while it is arriving and not yet known to be the status. */
  table: string;
  /** The text after the table. */
  after: string;
}

/** The header the steps intro (`business/src/hooks/steps.rs`) asks for, without its spaces. */
const HEADER = '|Step|Status|';

/** The labels of the family hooks' status lines (`business/src/hooks/family_text.rs`). */
const LABELS = new Set(['Limits', 'Branch', 'Complexity', 'Notes', 'Scripts', 'Knowledge', 'Code Graph']);

const FENCE = /^ {0,3}(`{3,}|~{3,})/;
const DELIMITER = /^\|?[\s:|-]*-[\s:|-]*\|?$/;

const compact = (line: string): string => line.replace(/\s+/g, '');
const isRow = (line: string): boolean => line.trimStart().startsWith('|');

/** The first column of a table row, which holds the label of a status line. */
function labelOf(row: string): string {
  return row.trim().slice(1).split('|')[0].trim();
}

/** The next line from `start` that is the table's header, skipping fenced code, or the first that may still become it. */
function findHeader(lines: string[], streaming: boolean, start = 0): { index: number; whole: boolean } | null {
  let fence: string | null = null;

  for (const [index, line] of lines.entries()) {
    const marker = FENCE.exec(line)?.[1];

    if (marker) {
      // A fence closes only with its own character, at least as long: a ~~~ inside a ``` block is text.
      if (fence === null) {
        fence = marker;
      } else if (marker[0] === fence[0] && marker.length >= fence.length) {
        fence = null;
      }
    } else if (fence === null && index >= start) {
      if (compact(line) === HEADER) {
        return { index, whole: true };
      }

      if (streaming && index === lines.length - 1 && isRow(line) && HEADER.startsWith(compact(line))) {
        return { index, whole: false };
      }
    }
  }

  return null;
}

/**
 * Finds the steps status the family hooks ask an agent to write at the start of its final answer, so the chat can show it as
 * a card of its own. Null when the text holds none: another table with these headers stays in the answer, because a status
 * has at least one row with a label of a status line.
 *
 * While the text is `streaming` the last line may be cut, so a table that may still turn out to be the status is held back
 * rather than shown as text, and a row joins the table once its line is complete.
 */
export function splitStepStatus(text: string, streaming = false): StepStatusParts | null {
  const lines = text.split('\n');

  // Another table with these headers may come first, and it stays in the answer.
  for (let found = findHeader(lines, streaming); found; found = findHeader(lines, streaming, found.index + 1)) {
    const parts = splitAt(lines, found, streaming);

    if (parts) {
      return parts;
    }
  }

  return null;
}

/** The status at the header `found`; null when the table there is not the status. */
function splitAt(lines: string[], found: { index: number; whole: boolean }, streaming: boolean): StepStatusParts | null {
  const before = lines.slice(0, found.index).join('\n').trim();
  const held: StepStatusParts = { before, table: '', after: '' };

  if (!found.whole) {
    return held;
  }

  const delimiter = lines[found.index + 1];

  if (delimiter === undefined || (streaming && found.index + 1 === lines.length - 1 && /^[\s:|-]*$/.test(delimiter) && !isComplete(delimiter))) {
    return streaming ? held : null;
  }

  if (!DELIMITER.test(delimiter.trim())) {
    return null;
  }

  const first = found.index + 2;
  let end = first;

  while (end < lines.length && isRow(lines[end])) {
    end += 1;
  }

  // The last row is still arriving when the text stops inside it.
  const open = streaming && end === lines.length && end > first;
  const stop = open ? end - 1 : end;

  if (!lines.slice(first, stop).some((row) => LABELS.has(labelOf(row)))) {
    return streaming && end >= lines.length - 1 ? held : null;
  }

  return { before, table: lines.slice(found.index, stop).join('\n'), after: lines.slice(end).join('\n').trim() };
}

/** A delimiter line that ends its last column, which is the shape the table's two columns give it. */
function isComplete(line: string): boolean {
  return compact(line).split('|').filter((cell) => cell.includes('-')).length >= 2 && compact(line).endsWith('|');
}

/** The complexity as the card says it. Agents write small, medium or full, the words of the hook text. */
export type StepLevel = 'easy' | 'medium' | 'hard';

const LEVELS: Record<string, StepLevel> = { small: 'easy', easy: 'easy', medium: 'medium', full: 'hard', hard: 'hard' };
const LEVEL = /^(small|easy|medium|full|hard)\b[\s,:;\u2013\u2014-]*(.*)$/i;

export interface StepRow {
  label: string;
  value: string;
}

/** What a status table says. */
export interface StepStatus {
  /** Null when the agent wrote no level the card knows, so the card shows no badge. */
  level: StepLevel | null;
  /** Why it is that level; the whole value when it names no level. */
  reason: string | null;
  /** The other rows, in the order of the table. */
  rows: StepRow[];
}

/** Whether a status line says nothing was used: `none fit`, `none`. */
export function saysNone(value: string): boolean {
  return /^none\b/i.test(value);
}

function rowOf(line: string): StepRow {
  const [label, ...rest] = line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|');

  return { label: label.trim(), value: rest.join('|').trim() };
}

/** Reads the table `splitStepStatus` found. A limit that is not close says nothing worth a row. */
export function readStepStatus(table: string): StepStatus {
  const rows = table.split('\n').slice(2).map(rowOf);
  const complexity = rows.find((row) => row.label === 'Complexity')?.value ?? null;
  const match = complexity === null ? null : LEVEL.exec(complexity);

  return {
    level: match ? LEVELS[match[1].toLowerCase()] : null,
    reason: (match ? match[2] : complexity)?.trim() || null,
    rows: rows.filter((row) => row.label !== 'Complexity' && !(row.label === 'Limits' && saysNone(row.value))),
  };
}
