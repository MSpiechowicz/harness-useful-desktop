import { t } from '$lib/i18n/i18n.svelte';
import type { DiffLine } from '$lib/ipc/bindings/DiffLine';
import type { FileDiff } from '$lib/ipc/bindings/FileDiff';

/** A line of the diff together with its position in the file's flat line order. */
export interface IndexedLine {
  index: number;
  line: DiffLine;
}

export interface HunkRow {
  kind: 'hunk';
  key: string;
  hunk: number;
  header: string;
  /** Flat index of the first line of the hunk, to find the hunk a visible row belongs to. */
  firstIndex: number;
}

export interface LineRow extends IndexedLine {
  kind: 'line';
  key: string;
  hunk: number;
}

export interface PairRow {
  kind: 'pair';
  key: string;
  hunk: number;
  left: IndexedLine | null;
  right: IndexedLine | null;
}

export interface NoteRow {
  kind: 'note';
  key: string;
  hunk: number;
}

export type UnifiedRow = HunkRow | LineRow | NoteRow;
export type SplitRow = HunkRow | PairRow | NoteRow;
export type DiffRow = UnifiedRow | SplitRow;

/** Every line of every hunk, in order. Comments anchor to positions in this list. */
export function flatLines(diff: FileDiff): DiffLine[] {
  return diff.hunks.flatMap((hunk) => hunk.lines);
}

/** Flat line indexes a row displays; the row shows comment threads anchored to any of them. */
export function rowIndexes(row: DiffRow): number[] {
  switch (row.kind) {
    case 'line':
      return [row.index];

    case 'pair':
      return [...new Set([row.left?.index, row.right?.index].filter((index): index is number => index !== undefined))];

    default:
      return [];
  }
}

function hunkRow(hunk: number, header: string, firstIndex: number): HunkRow {
  return { kind: 'hunk', key: `h${hunk}`, hunk, header, firstIndex };
}

/** One row per diff line, with a header row before each hunk. */
export function unifiedRows(diff: FileDiff): UnifiedRow[] {
  const rows: UnifiedRow[] = [];
  let index = 0;

  diff.hunks.forEach((hunk, hunkNo) => {
    rows.push(hunkRow(hunkNo, hunk.header, index));

    for (const line of hunk.lines) {
      rows.push(line.kind === 'noNewline' ? { kind: 'note', key: `n${index}`, hunk: hunkNo } : { kind: 'line', key: `l${index}`, hunk: hunkNo, index, line });
      index += 1;
    }
  });

  return rows;
}

/**
 * Side-by-side rows: context lines sit on both sides, and a run of removed lines is paired
 * with the added lines that follow it, so changed lines face their replacement.
 */
export function splitRows(diff: FileDiff): SplitRow[] {
  const rows: SplitRow[] = [];
  let index = 0;

  diff.hunks.forEach((hunk, hunkNo) => {
    rows.push(hunkRow(hunkNo, hunk.header, index));

    let removed: IndexedLine[] = [];
    let added: IndexedLine[] = [];

    const flush = (): void => {
      const count = Math.max(removed.length, added.length);

      for (let slot = 0; slot < count; slot += 1) {
        const left = removed[slot] ?? null;
        const right = added[slot] ?? null;
        rows.push({ kind: 'pair', key: `p${left?.index ?? 'x'}-${right?.index ?? 'x'}`, hunk: hunkNo, left, right });
      }

      removed = [];
      added = [];
    };

    for (const line of hunk.lines) {
      const entry = { index, line };
      index += 1;

      if (line.kind === 'remove') {
        // A remove after an add starts a new change block.
        if (added.length > 0) {
          flush();
        }
        removed.push(entry);
      } else if (line.kind === 'add') {
        added.push(entry);
      } else if (line.kind === 'noNewline') {
        flush();
        rows.push({ kind: 'note', key: `n${entry.index}`, hunk: hunkNo });
      } else {
        flush();
        rows.push({ kind: 'pair', key: `p${entry.index}-${entry.index}`, hunk: hunkNo, left: entry, right: entry });
      }
    }

    flush();
  });

  return rows;
}

/** The `+`/`−` marker shown beside a line, so a change never relies on color alone. */
export function lineSign(kind: DiffLine['kind']): string {
  if (kind === 'add') {
    return '+';
  }

  return kind === 'remove' ? '−' : '';
}

/** The same marker for screen readers. */
export function lineSpoken(kind: DiffLine['kind']): string {
  if (kind === 'add') {
    return t('components.diff.spoken.added');
  }

  return kind === 'remove' ? t('components.diff.spoken.removed') : '';
}
