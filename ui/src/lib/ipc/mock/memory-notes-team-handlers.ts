import type { LocalNoteIds } from '../bindings/LocalNoteIds';
import type { MemoryCopyKind } from '../bindings/MemoryCopyKind';
import type { MemoryReposPage } from '../bindings/MemoryReposPage';
import type { MemoryScriptsAvailability } from '../bindings/MemoryScriptsAvailability';
import type { NoteContent } from '../bindings/NoteContent';
import type { NotesCopyCounts } from '../bindings/NotesCopyCounts';
import type { NotesCopyFilter } from '../bindings/NotesCopyFilter';
import type { NotesCopyMode } from '../bindings/NotesCopyMode';
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

/** What a copy to this computer does with a remote entry, by its place in the plan: most are added, some are already here, some differ. */
const PULL_FATES = ['add', 'add', 'identical', 'add', 'localDiffers'] as const;

const DAY_MS = 86_400_000;

/** Why a copy cannot take a local entry: the remote has one of the same name that this computer hides, so it would collide with it. Nothing when it can. */
function blockOf<T extends TeamEntry>(store: TeamStore<T>, entry: T): string | null {
  const label = store.label(entry);
  const hidden = store.read().find((other) => other.place === 'team' && other.hidden && store.label(other) === label);

  return hidden ? `a remote entry named “${label}” is hidden on this computer, so unhide it to copy this one` : null;
}

/** Whether a copy to the remote takes a row that does not fail when it is ticked: it adds, updates or replaces something. */
const copiesSomething = (fate: (typeof FATES)[number]): boolean => fate === 'add' || fate === 'conflict';

function rowOf(entry: TeamEntry, title: string, fate: (typeof FATES)[number], failure: string | null, mode: NotesCopyMode | undefined): NotesCopyRow {
  const team = fate === 'add' ? null : new Date(Date.parse(entry.updatedAt) + (fate === 'teamNewer' ? DAY_MS : -DAY_MS)).toISOString();

  return {
    localId: entry.id,
    teamId: fate === 'add' ? null : entry.id,
    pickable: !failure && (mode === 'toRemote' ? copiesSomething(fate) : fate === 'conflict'),
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

/** A remote entry in a copy to this computer: only one to add can be ticked, the others say why not by their action. */
function pullRowOf(entry: TeamEntry, title: string, fate: (typeof PULL_FATES)[number]): NotesCopyRow {
  return {
    localId: fate === 'add' ? null : entry.id,
    teamId: entry.id,
    pickable: fate === 'add',
    scope: entry.scope,
    repo: entry.repo,
    title,
    action: fate,
    conflict: false,
    failed: false,
    failure: null,
    localUpdatedAt: fate === 'add' ? null : entry.updatedAt,
    teamUpdatedAt: entry.updatedAt,
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
    localDiffers: count('localDiffers'),
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
    case 'pickable':
      return row.pickable;
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

/** The mock backend keeps one plan per kind and mode, like the real commands. */
const planKey = (kind: MemoryCopyKind, mode: NotesCopyMode | undefined): string => `${kind}:${mode ?? 'stranded'}`;

/**
 * The copy commands of the mock backend: a plan of the entries saved on this computer (`stranded`, `toRemote`) or of the remote entries missing here
 * (`toLocal`), one at a time per kind and mode and used up by its copy, like the real commands. A copy to this computer reports what it added and
 * changes nothing in the stores. The mock does not read the Settings switches: it never answers `forbidden`. The stores are read when a command runs,
 * so the notes and the scripts handlers may be made in any order.
 */
export function copyHandlers(stores: CopyStores): CopyHandlers {
  const plans: Partial<Record<string, { id: string; rows: NotesCopyRow[] }>> = {};
  let counter = 0;

  const storeOf = (kind: MemoryCopyKind): TeamStore<TeamEntry> => {
    const store = stores[kind];

    if (!store) {
      throw IpcFailure.from({ code: 'unsupported', message: `the mock backend keeps no ${kind}` });
    }

    return store;
  };

  const current = (kind: MemoryCopyKind, mode: NotesCopyMode | undefined, id: string): NotesCopyRow[] => {
    const plan = plans[planKey(kind, mode)];

    if (!plan || plan.id !== id) {
      throw IpcFailure.from({ code: 'invalidInput', message: 'this review has expired, so compare again' });
    }

    return plan.rows;
  };

  /** The rows of a copy to this computer: the remote entries this computer does not hide. */
  const pullRows = (store: TeamStore<TeamEntry>): NotesCopyRow[] =>
    store
      .read()
      .filter((entry) => entry.place === 'team' && !entry.hidden)
      .map((entry, index) => pullRowOf(entry, store.label(entry), PULL_FATES[index % PULL_FATES.length]))
      .sort(inPlanOrder);

  /** The rows of a copy to the remote: the entries saved on this computer. */
  const pushRows = (store: TeamStore<TeamEntry>, mode: NotesCopyMode | undefined): NotesCopyRow[] =>
    store
      .read()
      .filter((entry) => entry.place === 'local')
      .map((entry, index) => rowOf(entry, store.label(entry), FATES[index % FATES.length], blockOf(store, entry), mode))
      .sort(inPlanOrder);

  /** A copy to this computer: the ticked remote entries are added here (reported, not stored), nothing is deleted. */
  const pull = (rows: NotesCopyRow[], overwrite: LocalNoteIds | undefined, pick: LocalNoteIds | undefined): NotesCopyResult => {
    const pickable = rows.filter((row) => row.pickable);
    const picked = pick === 'all' ? pickable : pick === undefined || pick === 'none' ? [] : pickable.filter((row) => pick.includes(row.teamId ?? -1));

    if (overwrite !== undefined && overwrite !== 'none') {
      throw IpcFailure.from({ code: 'invalidInput', message: 'a copy to this computer overwrites nothing' });
    }

    if (picked.length === 0 || (Array.isArray(pick) && pick.some((id) => !pickable.some((row) => row.teamId === id)))) {
      throw IpcFailure.from({ code: 'invalidInput', message: 'pick entries to add from this review' });
    }

    return { counts: { ...countsOf([]), added: picked.length }, deleted: 0, kept: 0, leftover: [], failed: [] };
  };

  /**
   * A copy to the remote of the ticked rows only: each is the remote's from now on and gone from this computer, a ticked conflict replaces the
   * remote text, and a row not ticked is neither copied nor deleted.
   */
  const pushPicked = (store: TeamStore<TeamEntry>, rows: NotesCopyRow[], overwrite: LocalNoteIds | undefined, pick: LocalNoteIds | undefined): NotesCopyResult => {
    const pickable = rows.filter((row) => row.pickable);
    const picked = pick === 'all' ? pickable : pick === undefined || pick === 'none' ? [] : pickable.filter((row) => pick.includes(row.localId ?? -1));

    if (overwrite !== undefined && overwrite !== 'none') {
      throw IpcFailure.from({ code: 'invalidInput', message: 'a copy to the remote overwrites what is ticked, so tick the conflicts' });
    }

    if (picked.length === 0 || (Array.isArray(pick) && pick.some((id) => !pickable.some((row) => row.localId === id)))) {
      throw IpcFailure.from({ code: 'invalidInput', message: 'pick entries to copy from this review' });
    }

    store.write(store.read().filter((entry) => !picked.some((row) => row.localId === entry.id)));

    return {
      counts: { ...countsOf(picked), overwritten: picked.filter((row) => row.conflict).length },
      deleted: picked.length,
      kept: 0,
      leftover: [],
      failed: [],
    };
  };

  return {
    memory_repos_list: ({ kind }) => {
      const store = storeOf(kind);
      const availability = store.availability();

      return availability.kind === 'available' ? reposPage(store.read()) : { availability, repos: [], global: 0 };
    },
    memory_notes_copy_preview: ({ kind, mode }) => {
      const store = storeOf(kind);
      requireServer(store.availability());

      counter += 1;
      const rows = mode === 'toLocal' ? pullRows(store) : pushRows(store, mode);
      const plan = { id: `copy-${counter}`, rows };
      plans[planKey(kind, mode)] = plan;

      const nothing = mode === 'toLocal' ? `no ${kind} of the remote database are missing on this computer` : `no ${kind} are saved on this computer`;

      return { previewId: plan.id, nothing: rows.length === 0 ? nothing : null, counts: countsOf(rows), total: rows.length, rows: rows.slice(0, 50) };
    },
    memory_notes_copy_rows: ({ kind, mode, previewId, filter, offset, limit }) => {
      const rows = current(kind, mode, previewId).filter((row) => inFilter(row, filter));

      return { total: rows.length, rows: rows.slice(offset, offset + limit) };
    },
    memory_notes_copy_apply: ({ kind, mode, previewId, overwrite = 'none', pick }) => {
      const store = storeOf(kind);
      requireServer(store.availability());

      const rows = current(kind, mode, previewId);

      if (mode === 'toLocal') {
        const result = pull(rows, overwrite, pick);
        plans[planKey(kind, mode)] = undefined;

        return result;
      }

      if (mode === 'toRemote') {
        const result = pushPicked(store, rows, overwrite, pick);
        plans[planKey(kind, mode)] = undefined;

        return result;
      }

      if (pick !== undefined && pick !== 'none') {
        throw IpcFailure.from({ code: 'invalidInput', message: 'a copy of what stayed here picks conflicts to overwrite, not entries' });
      }

      const conflicts = rows.filter((row) => row.conflict);
      const picked = overwrite === 'all' ? conflicts : overwrite === 'none' ? [] : conflicts.filter((row) => overwrite.includes(row.localId ?? -1));

      if (Array.isArray(overwrite) && overwrite.some((id) => !conflicts.some((row) => row.localId === id))) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'an entry picked to overwrite is not a conflict of this review' });
      }

      plans[planKey(kind, mode)] = undefined;

      // A copied entry is the team's from now on. One the team has the same is dropped from this computer. The others stay.
      const byId = new Map(store.read().map((entry) => [entry.id, entry]));
      const moved = rows.filter((row) => !row.failed && (row.action === 'add' || picked.includes(row)));
      const same = rows.filter((row) => !row.failed && row.action === 'identical');

      for (const row of moved) {
        const entry = byId.get(row.localId ?? -1);

        if (entry) {
          entry.place = 'team';
        }
      }
      store.write(store.read().filter((entry) => !same.some((row) => row.localId === entry.id)));

      return {
        counts: { ...countsOf(rows), overwritten: picked.length },
        deleted: moved.length + same.length,
        kept: rows.length - moved.length - same.length,
        leftover: rows.filter((row) => !row.failed && !moved.includes(row) && !same.includes(row)).flatMap((row) => (row.localId === null ? [] : [row.localId])),
        failed: rows.filter((row) => row.failed).map((row) => ({ localId: row.localId, teamId: row.teamId, title: row.title, message: row.failure ?? '' })),
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
