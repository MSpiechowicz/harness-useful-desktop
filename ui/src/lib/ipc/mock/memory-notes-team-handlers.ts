import type { LocalNoteIds } from '../bindings/LocalNoteIds';
import type { MemoryCopyKind } from '../bindings/MemoryCopyKind';
import type { MemoryReposPage } from '../bindings/MemoryReposPage';
import type { MemoryScriptsAvailability } from '../bindings/MemoryScriptsAvailability';
import type { NoteContent } from '../bindings/NoteContent';
import type { NotesCopyCounts } from '../bindings/NotesCopyCounts';
import type { NotesCopyFilter } from '../bindings/NotesCopyFilter';
import type { NotesCopyPreview } from '../bindings/NotesCopyPreview';
import type { NotesCopyResult } from '../bindings/NotesCopyResult';
import type { NotesCopyRow } from '../bindings/NotesCopyRow';
import type { NotesCopyRows } from '../bindings/NotesCopyRows';
import type { ScriptDetail } from '../bindings/ScriptDetail';
import type { CommandArgs } from '../commands';
import { IpcFailure } from '../transport';
import { requireServer } from './memory-delete-all';
import { reposPage } from './memory-reach';

/** What a note and a script share for hiding, discarding and copying: where they apply, where they are kept, and whether this computer hides them. */
export interface TeamEntry {
  id: number;
  scope: 'repo' | 'global';
  repo: string | null;
  updatedAt: string;
  place: 'team' | 'local';
  hidden: boolean;
}

/** Where the notes or the scripts of the mock backend live: the handlers read them and write the next list back. */
export interface TeamStore<T extends TeamEntry> {
  read(): T[];
  write(next: T[]): void;
  availability(): MemoryScriptsAvailability;
  /** The line that names an entry in a copy plan: a note's title, a script's name. */
  label(entry: T): string;
}

/** The stores a copy reads, by kind. The notes and the scripts handlers each add theirs, so one set of copy commands serves both. */
export type CopyStores = Partial<Record<MemoryCopyKind, TeamStore<TeamEntry>>>;

/** The commands of the team side of the notes, each answering at once. */
export interface TeamNotesHandlers extends CopyHandlers {
  memory_note_hide: (args: CommandArgs<'memory_note_hide'>) => null;
  memory_notes_discard_local: (args: CommandArgs<'memory_notes_discard_local'>) => number;
}

/** The commands of the team side of the scripts. Copying is the notes handlers', which read both kinds. */
export interface TeamScriptsHandlers {
  memory_script_hide: (args: CommandArgs<'memory_script_hide'>) => null;
  memory_scripts_discard_local: (args: CommandArgs<'memory_scripts_discard_local'>) => number;
}

/** The commands that copy the entries of a kind to the team database, and the one that lists the repositories they are in. */
export interface CopyHandlers {
  memory_repos_list: (args: CommandArgs<'memory_repos_list'>) => MemoryReposPage;
  memory_notes_copy_preview: (args: CommandArgs<'memory_notes_copy_preview'>) => NotesCopyPreview;
  memory_notes_copy_rows: (args: CommandArgs<'memory_notes_copy_rows'>) => NotesCopyRows;
  memory_notes_copy_apply: (args: CommandArgs<'memory_notes_copy_apply'>) => NotesCopyResult;
}

/** What a copy does with a local entry, by its place in the plan: most are added, some are already there, some differ or are older than the team's. */
const FATES = ['add', 'identical', 'conflict', 'add', 'teamNewer', 'conflict'] as const;

const DAY_MS = 86_400_000;

/** Why a copy cannot take a local entry: the remote has one of the same name that this computer hides, so it would collide with it. Nothing when it can. */
function blockOf<T extends TeamEntry>(store: TeamStore<T>, entry: T): string | null {
  const label = store.label(entry);
  const hidden = store.read().find((other) => other.place === 'team' && other.hidden && store.label(other) === label);

  return hidden ? `a remote entry named “${label}” is hidden on this computer, so unhide it to copy this one` : null;
}

function rowOf(entry: TeamEntry, title: string, fate: (typeof FATES)[number], failure: string | null): NotesCopyRow {
  const team = fate === 'add' ? null : new Date(Date.parse(entry.updatedAt) + (fate === 'teamNewer' ? DAY_MS : -DAY_MS)).toISOString();

  return {
    localId: entry.id,
    scope: entry.scope,
    repo: entry.repo,
    title,
    action: failure || fate === 'conflict' ? 'keep' : fate === 'teamNewer' ? 'teamNewer' : fate,
    conflict: !failure && fate === 'conflict',
    failed: failure !== null,
    failure,
    localUpdatedAt: entry.updatedAt,
    teamUpdatedAt: team,
  };
}

/** Global entries first, then by repository, then by title: how the real server groups the rows. */
function inPlanOrder(a: NotesCopyRow, b: NotesCopyRow): number {
  return (a.scope === b.scope ? 0 : a.scope === 'global' ? -1 : 1) || (a.repo ?? '').localeCompare(b.repo ?? '') || a.title.localeCompare(b.title);
}

function countsOf(rows: readonly NotesCopyRow[]): NotesCopyCounts {
  const count = (action: NotesCopyRow['action']) => rows.filter((row) => !row.failed && row.action === action).length;

  return {
    added: count('add'),
    updated: 0,
    overwritten: 0,
    identical: count('identical'),
    teamNewer: count('teamNewer'),
    conflicts: rows.filter((row) => row.conflict).length,
    failed: rows.filter((row) => row.failed).length,
  };
}

function inFilter(row: NotesCopyRow, filter: NotesCopyFilter | undefined): boolean {
  switch (filter) {
    case 'conflicts':
      return row.conflict;
    case 'failed':
      return row.failed;
    default:
      return true;
  }
}

/** Hides a team entry on this computer, or shows it again: one that is saved on this computer cannot be hidden. */
function hideEntry<T extends TeamEntry>({ read }: TeamStore<T>, noun: string, { id, hidden }: { id: number; hidden: boolean }): null {
  const found = read().find((entry) => entry.id === id);

  if (!found) {
    throw IpcFailure.from({ code: 'notFound', message: `${noun} ${id} was not found` });
  }

  if (found.place === 'local') {
    throw IpcFailure.from({ code: 'process', message: `${noun} ${id} is saved on this computer, delete it instead` });
  }

  found.hidden = hidden;

  return null;
}

/** Deletes entries saved on this computer, where a scope routed to the team database no longer reads them: the ones named, or all. */
function discardEntries<T extends TeamEntry>({ read, write, availability }: TeamStore<T>, noun: string, { ids }: { ids: LocalNoteIds }): number {
  requireServer(availability());

  if (ids === 'none' || (Array.isArray(ids) && ids.length === 0)) {
    throw IpcFailure.from({ code: 'invalidInput', message: `name the ${noun}s to delete` });
  }

  const gone = read().filter((entry) => entry.place === 'local' && (ids === 'all' || ids.includes(entry.id)));
  write(read().filter((entry) => !gone.includes(entry)));

  return gone.length;
}

/**
 * The copy commands of the mock backend: a plan of the entries saved on this computer, one at a time per kind and used up by its copy, like the
 * real commands. The stores are read when a command runs, so the notes and the scripts handlers may be made in any order.
 */
export function copyHandlers(stores: CopyStores): CopyHandlers {
  const plans: Partial<Record<MemoryCopyKind, { id: string; rows: NotesCopyRow[] }>> = {};
  let counter = 0;

  const storeOf = (kind: MemoryCopyKind): TeamStore<TeamEntry> => {
    const store = stores[kind];

    if (!store) {
      throw IpcFailure.from({ code: 'unsupported', message: `the mock backend keeps no ${kind}` });
    }

    return store;
  };

  const current = (kind: MemoryCopyKind, id: string): NotesCopyRow[] => {
    const plan = plans[kind];

    if (!plan || plan.id !== id) {
      throw IpcFailure.from({ code: 'invalidInput', message: 'this review has expired, so compare again' });
    }

    return plan.rows;
  };

  return {
    memory_repos_list: ({ kind }) => {
      const store = storeOf(kind);
      const availability = store.availability();

      return availability.kind === 'available' ? reposPage(store.read()) : { availability, repos: [], global: 0 };
    },
    memory_notes_copy_preview: ({ kind }) => {
      const store = storeOf(kind);
      requireServer(store.availability());

      counter += 1;
      const rows = store
        .read()
        .filter((entry) => entry.place === 'local')
        .map((entry, index) => rowOf(entry, store.label(entry), FATES[index % FATES.length], blockOf(store, entry)))
        .sort(inPlanOrder);
      const plan = { id: `copy-${counter}`, rows };
      plans[kind] = plan;

      return { previewId: plan.id, nothing: rows.length === 0 ? `no ${kind} are saved on this computer` : null, counts: countsOf(rows), total: rows.length, rows: rows.slice(0, 50) };
    },
    memory_notes_copy_rows: ({ kind, previewId, filter, offset, limit }) => {
      const rows = current(kind, previewId).filter((row) => inFilter(row, filter));

      return { total: rows.length, rows: rows.slice(offset, offset + limit) };
    },
    memory_notes_copy_apply: ({ kind, previewId, overwrite }) => {
      const store = storeOf(kind);
      requireServer(store.availability());

      const rows = current(kind, previewId);
      const conflicts = rows.filter((row) => row.conflict);
      const picked = overwrite === 'all' ? conflicts : overwrite === 'none' ? [] : conflicts.filter((row) => overwrite.includes(row.localId));

      if (Array.isArray(overwrite) && overwrite.some((id) => !conflicts.some((row) => row.localId === id))) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'an entry picked to overwrite is not a conflict of this review' });
      }

      plans[kind] = undefined;

      // A copied entry is the team's from now on. One the team has the same is dropped from this computer. The others stay.
      const byId = new Map(store.read().map((entry) => [entry.id, entry]));
      const moved = rows.filter((row) => !row.failed && (row.action === 'add' || picked.includes(row)));
      const same = rows.filter((row) => !row.failed && row.action === 'identical');

      for (const row of moved) {
        const entry = byId.get(row.localId);

        if (entry) {
          entry.place = 'team';
        }
      }
      store.write(store.read().filter((entry) => !same.some((row) => row.localId === entry.id)));

      return {
        counts: { ...countsOf(rows), overwritten: picked.length },
        deleted: moved.length + same.length,
        kept: rows.length - moved.length - same.length,
        leftover: rows.filter((row) => !row.failed && !moved.includes(row) && !same.includes(row)).map((row) => row.localId),
        failed: rows.filter((row) => row.failed).map((row) => ({ localId: row.localId, title: row.title, message: row.failure ?? '' })),
      };
    },
  };
}

/**
 * The team side of the notes in the mock backend: hiding a team note on this computer, deleting the notes left on it, and copying them to the
 * team database. The store is added to `stores`, which the scripts handlers share.
 */
export function teamNotesHandlers(store: Omit<TeamStore<NoteContent>, 'label'>, stores: CopyStores = {}): TeamNotesHandlers {
  const notes: TeamStore<NoteContent> = { ...store, label: (note) => note.title };
  stores.notes = notes;

  return { memory_note_hide: (args) => hideEntry(notes, 'note', args), memory_notes_discard_local: (args) => discardEntries(notes, 'note', args), ...copyHandlers(stores) };
}

/** The team side of the scripts in the mock backend: hiding a team script on this computer and deleting the scripts left on it. */
export function teamScriptsHandlers(store: Omit<TeamStore<ScriptDetail>, 'label'>, stores: CopyStores = {}): TeamScriptsHandlers {
  const scripts: TeamStore<ScriptDetail> = { ...store, label: (script) => script.name };
  stores.scripts = scripts;

  return { memory_script_hide: (args) => hideEntry(scripts, 'script', args), memory_scripts_discard_local: (args) => discardEntries(scripts, 'script', args) };
}
