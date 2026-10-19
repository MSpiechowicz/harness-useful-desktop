import type { MemoryDeleteAll } from '../bindings/MemoryDeleteAll';
import type { MemoryNotesPage } from '../bindings/MemoryNotesPage';
import type { MemoryScriptsAvailability } from '../bindings/MemoryScriptsAvailability';
import type { NoteContent } from '../bindings/NoteContent';
import type { NoteDetail } from '../bindings/NoteDetail';
import type { NoteKind } from '../bindings/NoteKind';
import type { NoteSaved } from '../bindings/NoteSaved';
import type { NoteSort } from '../bindings/NoteSort';
import type { NotesImportPreview } from '../bindings/NotesImportPreview';
import type { NotesImportResult } from '../bindings/NotesImportResult';
import type { NotesImportRow } from '../bindings/NotesImportRow';
import type { CommandArgs } from '../commands';
import { IpcFailure } from '../transport';
import { deletedAll, requireServer } from './memory-delete-all';
import { detailOf, noteSummaryOf, sampleNotes } from './memory-notes-fixtures';
import { type CopyStores, type TeamNotesHandlers, teamNotesHandlers } from './memory-notes-team-handlers';

/** Browser-preview switch `?notes=`: `empty` keeps no notes at all, `readonly` makes the team notes read-only (a scope that saves on this computer), `blocked` hides a team note that has the title of a note on this computer, so a copy cannot take that one. Anything else shows the sample notes. */
export const NOTE_SCENARIOS = ['ok', 'empty', 'readonly', 'blocked'] as const;
export type NoteScenario = (typeof NOTE_SCENARIOS)[number];

export function noteScenarioFromLocation(search: string = globalThis.location?.search ?? ''): NoteScenario {
  const name = new URLSearchParams(search).get('notes');

  return NOTE_SCENARIOS.find((scenario) => scenario === name) ?? 'ok';
}

/** Browser-preview switch `?filters=old`: a Harness Memory that cannot combine filters (no `scope`, no scope counts), which the view then answers with the one kind filter and a hint to update. */
export function combinesFilters(search: string = globalThis.location?.search ?? ''): boolean {
  return new URLSearchParams(search).get('filters') !== 'old';
}

/** The notes of Harness Memory kept in memory: `memory_notes_list/note_get/note_delete/notes_delete_all/note_save` like the real backend, without a server. */
export interface NoteHandlers extends TeamNotesHandlers {
  memory_notes_list: (args: CommandArgs<'memory_notes_list'>) => MemoryNotesPage;
  memory_note_get: (args: CommandArgs<'memory_note_get'>) => NoteDetail;
  memory_note_delete: (args: CommandArgs<'memory_note_delete'>) => null;
  memory_notes_delete_all: (args: CommandArgs<'memory_notes_delete_all'>) => MemoryDeleteAll;
  memory_note_save: (args: CommandArgs<'memory_note_save'>) => NoteSaved;
  memory_notes_import_preview: (args: CommandArgs<'memory_notes_import_preview'>) => Promise<NotesImportPreview | null>;
  memory_notes_import_apply: (args: CommandArgs<'memory_notes_import_apply'>) => Promise<NotesImportResult>;
  memory_notes_import_template: (args: CommandArgs<'memory_notes_import_template'>) => string | null;
}

/**
 * Browser-preview switch `?notesImport=`: `cancelled` closes the file picker, `refused` picks a CSV file without a title column,
 * `invalidCsv` one with a broken record on line 14, `tooLarge` a file over 2 MiB. Anything else picks the sample file of six notes.
 */
export const NOTES_IMPORT_SCENARIOS = ['ok', 'cancelled', 'refused', 'invalidCsv', 'tooLarge'] as const;
export type NotesImportScenario = (typeof NOTES_IMPORT_SCENARIOS)[number];

export function notesImportScenarioFromLocation(search: string = globalThis.location?.search ?? ''): NotesImportScenario {
  const name = new URLSearchParams(search).get('notesImport');

  return NOTES_IMPORT_SCENARIOS.find((scenario) => scenario === name) ?? 'ok';
}

/** The refused previews of the `?notesImport=` scenarios. */
const REFUSED_IMPORTS: Partial<Record<NotesImportScenario, NotesImportPreview>> = {
  refused: { kind: 'refused', fileName: 'notes-export.csv', reason: 'missingColumns', line: null },
  invalidCsv: { kind: 'refused', fileName: 'broken.csv', reason: 'invalidCsv', line: 14 },
  tooLarge: { kind: 'refused', fileName: 'all-notes.json', reason: 'tooLarge', line: null },
};

/** Pause between two rows of an import; tests do not wait. */
const IMPORT_STEP_MS = import.meta.env.MODE === 'test' ? 0 : 250;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** A row of the sample import file and the body Rust would keep for it. */
interface ImportSample {
  row: Omit<NotesImportRow, 'index' | 'bodyPreview'>;
  body: string;
}

/**
 * The notes of the sample import file: new notes, one with the title of a sample repository note (skipped unless replaced), a later
 * row with an earlier row's title, and rows with problems the dialog explains.
 */
const IMPORT_SAMPLES: ImportSample[] = [
  { row: { title: 'Retry a flaky end-to-end test once before failing the run', kind: 'decision', tags: ['ci', 'tests'], scope: null, problems: [], duplicateOf: null }, body: 'One retry hides no real failure in practice and saves a rerun of the whole pipeline.' },
  { row: { title: 'Tauri window stayed blank after a hot reload on Wayland', kind: 'problem', tags: ['tauri', 'wayland'], scope: 'repo', problems: [], duplicateOf: null }, body: 'Seen again with the newer WebKitGTK build. Restarting the dev server brings the window back.' },
  { row: { title: 'Prefer the release profile for benchmarks', kind: 'note', tags: [], scope: 'global', problems: [], duplicateOf: null }, body: 'Debug builds are ten times slower and skew every comparison.' },
  { row: { title: '', kind: 'note', tags: ['draft'], scope: null, problems: [{ code: 'missing', field: 'title' }], duplicateOf: null }, body: 'A row without a title cannot be imported.' },
  { row: { title: 'Cache the tools probe between sessions', kind: 'note', tags: [], scope: null, problems: [{ code: 'unknownKind', field: 'kind' }, { code: 'unknownScope', field: 'scope' }], duplicateOf: null }, body: 'The probe takes a second on a cold start.' },
  { row: { title: 'Retry a flaky end-to-end test once before failing the run', kind: 'decision', tags: ['ci'], scope: null, problems: [], duplicateOf: 0 }, body: 'The same decision, written twice in the file.' },
];

/** The repository key of new repository notes when no note has one yet. */
const MOCK_REPO = 'github.com/useful/harness-desktop';

/** Control and invisible format characters, which Harness Memory drops from a title. */
const INVISIBLE = /^[\p{Cc}\p{Cf}\u034F\u115F\u1160\u17B4\u17B5\u180B-\u180F\u2800\u3164\uFE00-\uFE0F\uFFA0]$/u;

/** A title as the app compares titles: invisible characters dropped, inner spaces collapsed, outer ones dropped, any case. */
function titleKey(title: string): string {
  const visible = Array.from(title, (character) => {
    if (/\s/u.test(character) && character !== '\uFEFF') {
      return ' ';
    }

    return INVISIBLE.test(character) ? '' : character;
  });

  return visible.join('').trim().split(/\s+/).join(' ').toLowerCase();
}

/** Whether two titles are the same (see `titleKey`). */
function sameTitle(a: string, b: string): boolean {
  return titleKey(a) === titleKey(b);
}

/** `draft` written over `found`, one revision later. The entities stay: only a new note takes them. */
function rewrite(found: NoteContent, draft: CommandArgs<'memory_note_save'>['draft']): void {
  found.title = draft.title.trim();
  found.body = draft.body;
  found.kind = draft.kind;
  found.tags = [...new Set(draft.tags.map((tag) => tag.trim().toLowerCase()))];
  found.revision += 1;
  found.updatedAt = new Date().toISOString();
}

const DEFAULT_LIMIT = 50;

const KIND_ORDER: NoteKind[] = ['decision', 'fix', 'problem', 'outcome', 'optimisation', 'location', 'note'];

function matches(found: NoteContent, query: string): boolean {
  const needle = query.trim().toLowerCase();

  return needle === '' || [found.title, found.body, ...found.tags, ...found.entities].some((field) => field.toLowerCase().includes(needle));
}

type NoteOrder = (a: NoteContent, b: NoteContent) => number;

/** How each sort orders two notes, ascending (`scope`: global first, `place`: this computer first); ties go by id. */
const NOTE_ORDERS: Record<NoteSort, NoteOrder> = {
  updated: (a, b) => Date.parse(a.updatedAt) - Date.parse(b.updatedAt),
  title: (a, b) => a.title.localeCompare(b.title),
  kind: (a, b) => a.kind.localeCompare(b.kind),
  scope: (a, b) => a.scope.localeCompare(b.scope),
  place: (a, b) => a.place.localeCompare(b.place),
};

/**
 * One page of notes as the server answers: filtered by the query, the scope and the kind, sorted and cut. The kinds are counted under the
 * query and the scope, the scopes under the query and the kind. `combined: false` is an older server: it drops the scope and counts no scopes.
 * A hidden note is listed only when `hidden` asks for them. `everything` is every note of the store, which the count of notes on this computer takes in.
 */
export function pageOfNotes(
  visible: NoteContent[],
  { offset, limit = DEFAULT_LIMIT, query = '', kind, scope, sort = 'updated', order, hidden = false }: CommandArgs<'memory_notes_list'>,
  combined = true,
  everything: NoteContent[] = visible,
): Omit<MemoryNotesPage, 'availability'> {
  const answering = visible.filter((found) => matches(found, query) && found.hidden === hidden);
  const onThisComputer = everything.filter((found) => found.place === 'local');
  const direction = (order ?? (sort === 'updated' ? 'desc' : 'asc')) === 'desc' ? -1 : 1;
  const compare = NOTE_ORDERS[sort];
  const inKind = answering.filter((found) => kind === undefined || found.kind === kind);
  const inScope = answering.filter((found) => !combined || scope === undefined || found.scope === scope);
  const chosen = inKind.filter((found) => inScope.includes(found));

  return {
    total: chosen.length,
    kinds: KIND_ORDER.map((each) => ({ kind: each, count: inScope.filter((found) => found.kind === each).length })),
    scopes: combined ? { repo: inKind.filter((found) => found.scope === 'repo').length, global: inKind.filter((found) => found.scope === 'global').length } : null,
    combinedFilters: combined,
    notes: [...chosen].sort((a, b) => direction * (compare(a, b) || a.id - b.id)).slice(offset, offset + limit).map(noteSummaryOf),
    hiddenCount: visible.filter((found) => found.hidden).length,
    localCounts: { repo: onThisComputer.filter((found) => found.scope === 'repo').length, global: onThisComputer.filter((found) => found.scope === 'global').length },
  };
}

/** With `blocked`, a team note hidden on this computer takes the title of the first note saved on it, so copying that one collides with it. */
function withBlocked(notes: NoteContent[], blocked: boolean): NoteContent[] {
  const local = notes.find((found) => found.place === 'local');

  return blocked && local ? [...notes, { ...local, id: Math.max(...notes.map((found) => found.id)) + 1, place: 'team', hidden: true }] : notes;
}

export function memoryNotesHandlers(
  availability: () => MemoryScriptsAvailability,
  scenario: () => NoteScenario = noteScenarioFromLocation,
  combined: () => boolean = combinesFilters,
  importScenario: () => NotesImportScenario = notesImportScenarioFromLocation,
  copyStores: CopyStores = {},
): NoteHandlers {
  let notes: NoteContent[] | null = null;
  const current = (): NoteContent[] =>
    (notes ??= scenario() === 'empty' ? [] : withBlocked(sampleNotes(), scenario() === 'blocked').map((found) => ({ ...found, readOnly: scenario() === 'readonly' && found.place === 'team' })));

  /** A project sees its repository's notes and the global ones, no project only the global ones. */
  const visible = (project: string | null): NoteContent[] => current().filter((found) => found.scope === 'global' || project !== null);

  const find = (id: number, project: string | null, hidden = false): NoteContent => {
    const found = visible(project).find((candidate) => candidate.id === id && candidate.hidden === hidden);
    if (!found) {
      throw IpcFailure.from({ code: 'notFound', message: `note ${id} was not found` });
    }

    return found;
  };

  let importCounter = 0;
  let importPlan: { planId: string; project: string | null; samples: ImportSample[] } | null = null;
  /** Whether an import is being applied: like the real backend, one runs at a time. */
  let importing = false;

  const saving = {
    memory_notes_list: (args: CommandArgs<'memory_notes_list'>): MemoryNotesPage => {
      const state = availability();
      if (state.kind !== 'available') {
        return { availability: state, total: 0, kinds: [], scopes: null, combinedFilters: combined(), notes: [], hiddenCount: null, localCounts: null };
      }

      return { availability: state, ...pageOfNotes(visible(args.project), args, combined(), current()) };
    },
    memory_note_get: ({ id, project, hidden }: CommandArgs<'memory_note_get'>): NoteDetail => detailOf(find(id, project, hidden), visible(project)),
    memory_note_delete: ({ id, project }: CommandArgs<'memory_note_delete'>): null => {
      const found = find(id, project);
      notes = current().filter((candidate) => candidate !== found);

      return null;
    },
    memory_notes_delete_all: ({ project = null }: CommandArgs<'memory_notes_delete_all'>): MemoryDeleteAll => {
      requireServer(availability());

      // A team note that is read-only here stays.
      const all = visible(project);
      const gone = all.filter((candidate) => !candidate.readOnly);
      notes = current().filter((candidate) => !gone.includes(candidate));

      return deletedAll(gone, all.length - gone.length);
    },
    // Like the real backend: an edit (or a replace) goes by id and keeps the scope, and a new note whose title a note of its scope
    // has is refused (`exists`): the server refuses a repository one, the app looks a global one up first.
    memory_note_save: ({ project, draft, replaceId }: CommandArgs<'memory_note_save'>): NoteSaved => {
      requireServer(availability());

      if (replaceId !== undefined) {
        const found = find(replaceId, project);
        const other = visible(project).find((candidate) => candidate !== found && candidate.scope === found.scope && sameTitle(candidate.title, draft.title));
        if (other) {
          return { outcome: 'exists', id: other.id };
        }

        rewrite(found, draft);
        return { outcome: 'updated', id: found.id };
      }

      if (draft.scope === 'repo' && project === null) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'a repository note needs a project' });
      }

      const same = visible(project).find((candidate) => candidate.scope === draft.scope && sameTitle(candidate.title, draft.title));
      if (same) {
        return { outcome: 'exists', id: same.id };
      }

      const now = new Date().toISOString();
      const id = Math.max(0, ...current().map((candidate) => candidate.id)) + 1;
      const repo = draft.scope === 'repo' ? (current().find((candidate) => candidate.repo !== null)?.repo ?? MOCK_REPO) : null;
      const created: NoteContent = {
        id,
        kind: draft.kind,
        title: '',
        body: '',
        scope: draft.scope,
        repo,
        place: 'local',
        tags: [],
        entities: [...new Set(draft.entities)],
        author: { source: 'harness-desktop', root: draft.scope === 'repo' ? project : null },
        createdAt: now,
        updatedAt: now,
        revision: 0,
        hits: 0,
        readOnly: false,
        hidden: false,
      };

      rewrite(created, draft);
      notes = [...current(), created];

      return { outcome: 'created', id };
    },
  };

  return {
    ...saving,
    ...teamNotesHandlers({ read: current, write: (next) => (notes = next), availability }, copyStores),
    // Like the real backend: one review at a time, kept with its project, used up by its import, dropped by a refused file.
    memory_notes_import_preview: async ({ project }) => {
      const picked = importScenario();
      if (picked === 'cancelled') {
        return null;
      }

      const refused = REFUSED_IMPORTS[picked];
      if (refused) {
        importPlan = null;
        return refused;
      }

      importCounter += 1;
      importPlan = { planId: `notes-import-${importCounter}`, project, samples: IMPORT_SAMPLES };

      return {
        kind: 'plan',
        plan: {
          planId: importPlan.planId,
          fileName: 'team-notes.csv',
          format: 'csv',
          truncated: false,
          rows: IMPORT_SAMPLES.map((sample, index) => ({ ...sample.row, index, bodyPreview: sample.body })),
        },
      };
    },
    // The save dialog of the browser preview: the file lands in the home folder, unless the `?notesImport=cancelled` scenario closes the dialog.
    memory_notes_import_template: ({ format }) => (importScenario() === 'cancelled' ? null : `/home/demo/notes-example.${format}`),
    memory_notes_import_apply: async ({ planId, rows, defaultScope, replace, project, onProgress }) => {
      if (importing) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'an import is already running' });
      }

      if (!importPlan || importPlan.planId !== planId || importPlan.project !== project) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'this review has expired, so review the file again' });
      }

      const { samples } = importPlan;
      importPlan = null;

      const selected = [...new Set(rows)].sort((a, b) => a - b);
      if (selected.length === 0 || selected.some((index) => samples[index] === undefined || samples[index].row.problems.length > 0)) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'only notes of the file without problems can be imported' });
      }

      const result: NotesImportResult = { created: 0, updated: 0, replaced: 0, exists: [], failed: [] };
      importing = true;

      try {
        for (const [position, index] of selected.entries()) {
          await delay(IMPORT_STEP_MS);

          const { row, body } = samples[index];
          const draft = { title: row.title, body, kind: row.kind, tags: row.tags, entities: [], scope: row.scope ?? defaultScope };

          try {
            const saved = saving.memory_note_save({ project, draft });

            const again = saved.outcome === 'exists' && replace && saved.id !== null ? saving.memory_note_save({ project, draft, replaceId: saved.id }) : null;

            if (again !== null && again.outcome !== 'exists') {
              result.replaced += 1;
            } else if (saved.outcome === 'exists') {
              result.exists.push({ index, id: (again ?? saved).id });
            } else {
              result[saved.outcome] += 1;
            }
          } catch (error) {
            result.failed.push({ index, message: error instanceof IpcFailure ? error.message : String(error) });
          }

          onProgress({ index, done: position + 1, total: selected.length });
        }
      } finally {
        importing = false;
      }

      return result;
    },
  };
}
