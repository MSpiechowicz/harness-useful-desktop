import type { KnowledgeIntake } from '$lib/ipc/bindings/KnowledgeIntake';
import type { KnowledgeMatch } from '$lib/ipc/bindings/KnowledgeMatch';
import type { KnowledgePlace } from '$lib/ipc/bindings/KnowledgePlace';
import type { KnowledgeReplacement } from '$lib/ipc/bindings/KnowledgeReplacement';
import type { KnowledgeSimilar } from '$lib/ipc/bindings/KnowledgeSimilar';
import { idKey } from '$lib/ipc/knowledge-id';

import { displayText } from './display-text';

/** One file of a selection and the stored document it is probably a version of. */
export interface MatchRow {
  path: string;
  /** The file name without its folder. */
  name: string;
  /** The size of a file chosen as a file. Not known for a file found in a chosen folder. */
  bytes: number | null;
  match: KnowledgeMatch | null;
  /** Why the file could not be compared. */
  error: string | null;
  /** A chosen folder, which is listed (with no match) until the comparison names its files. */
  folder?: boolean;
  /** The file was found in a chosen folder, not chosen itself. */
  fromFolder: boolean;
  /** The file may replace its match: it was chosen as a file (the server only accepts the paths of the selection), and no file before it replaces the same document. */
  replaceable: boolean;
}

function nameOf(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

/** A file name as the table shows it: without hidden characters that reorder it or draw nothing. */
function shown(name: string): string {
  return displayText(name, 300) || name;
}

/** The rows of a comparison, the files that look like a version of a document first, then the others, each in the order of the selection. */
export function matchRows(intake: KnowledgeIntake, similar: KnowledgeSimilar): MatchRow[] {
  const chosen = new Map(intake.files.map((file) => [file.path, file]));
  const claimed = new Set<string>();
  const seen = new Set<string>();

  // A folder and a file inside it come back as the same path twice: one row per file.
  const items = similar.items.filter((item) => !seen.has(item.path) && seen.add(item.path) !== undefined);

  const rows = items.map((item): MatchRow => {
    const file = chosen.get(item.path);
    const match = item.error ? null : (item.match ?? null);
    const free = match !== null && !claimed.has(idKey(match.id));

    if (match !== null && file !== undefined) {
      claimed.add(idKey(match.id));
    }

    return { path: item.path, name: shown(file?.name ?? nameOf(item.path)), bytes: file?.bytes ?? null, match, error: item.error ?? null, fromFolder: file === undefined, replaceable: free && file !== undefined };
  });

  return [...rows.filter((row) => row.match !== null), ...rows.filter((row) => row.match === null)];
}

/**
 * Where the import files its documents: a place when both are in use, `one` when only one is (so no match is in another place), and
 * `unknown` until the routing is known (every match then counts as being in another place).
 */
export type Landing = KnowledgePlace | 'one' | 'unknown';

/** What the user set per file and match: true updates the match, false adds the file as new. A file not in it takes its default. */
export type MatchChoices = Readonly<Record<string, boolean>>;

/** The key of a choice: the file and the match it was made for, so a comparison that finds another match does not inherit it. Null for a file with no match. */
export function choiceKey(row: MatchRow): string | null {
  return row.match === null ? null : `${row.path}|${idKey(row.match.id)}`;
}

/** The match lies in another place than the import files its documents in. The server refuses a replacement across places, so such a file is only ever added as new. */
export function inOtherPlace(row: MatchRow, landing: Landing): boolean {
  return row.match !== null && landing !== 'one' && row.match.place !== landing;
}

/** The import may replace the match of `row`: the file was chosen, and the match lies in the place the import goes to. */
export function canReplace(row: MatchRow, landing: Landing): boolean {
  return row.replaceable && !inOtherPlace(row, landing);
}

/** A file updates its match by default only when the match is no guess (not just the same title) and lies in the place the import goes to. */
export function updatesByDefault(row: MatchRow, landing: Landing): boolean {
  return row.match !== null && row.match.reason !== 'title' && !inOtherPlace(row, landing);
}

/** Whether the import replaces the match of `row`: it can, and the user chose so or left the default. */
export function updates(row: MatchRow, choices: MatchChoices, landing: Landing): boolean {
  if (!canReplace(row, landing)) {
    return false;
  }

  const key = choiceKey(row);

  return (key === null ? undefined : choices[key]) ?? updatesByDefault(row, landing);
}

/** What the import sends as `replacing`: the files set to update, each with the document it replaces. */
export function replacementsOf(rows: readonly MatchRow[], choices: MatchChoices, landing: Landing): KnowledgeReplacement[] {
  return rows.flatMap((row) => (row.match !== null && updates(row, choices, landing) ? [{ path: row.path, id: row.match.id }] : []));
}

/** How many files update a document and how many are added. A file that could not be compared is neither. */
export function summaryOf(rows: readonly MatchRow[], choices: MatchChoices, landing: Landing): { update: number; add: number } {
  const update = rows.filter((row) => updates(row, choices, landing)).length;
  const add = rows.filter((row) => row.error === null && !updates(row, choices, landing)).length;

  return { update, add };
}

/**
 * Whether any document of the import is filed in the team's database. A file set to update goes where the document it replaces is kept (the server files the
 * replacement there), every other file goes to `newPlace`, the place the import files new documents in. Before the comparison every file counts as new.
 */
export function goesToTeam(rows: readonly MatchRow[] | null, choices: MatchChoices, landing: Landing, newPlace: KnowledgePlace): boolean {
  if (rows === null || rows.length === 0) {
    return newPlace === 'remote';
  }

  // A file that could not be compared is not imported (see `summaryOf`), so it never sends a link anywhere.
  return rows.some((row) => row.error === null && (updates(row, choices, landing) ? (row.match?.place ?? newPlace) : newPlace) === 'remote');
}
