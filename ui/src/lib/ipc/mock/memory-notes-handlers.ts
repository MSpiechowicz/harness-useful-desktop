import type { MemoryDeleteAll } from '../bindings/MemoryDeleteAll';
import type { MemoryNotesPage } from '../bindings/MemoryNotesPage';
import type { MemoryScriptsAvailability } from '../bindings/MemoryScriptsAvailability';
import type { NoteContent } from '../bindings/NoteContent';
import type { NoteDetail } from '../bindings/NoteDetail';
import type { NoteKind } from '../bindings/NoteKind';
import type { NoteSort } from '../bindings/NoteSort';
import type { CommandArgs } from '../commands';
import { IpcFailure } from '../transport';
import { deletedAll, requireServer } from './memory-delete-all';
import { detailOf, noteSummaryOf, sampleNotes } from './memory-notes-fixtures';

/** Browser-preview switch `?notes=`: `empty` keeps no notes at all. Anything else shows the sample notes. */
export const NOTE_SCENARIOS = ['ok', 'empty'] as const;
export type NoteScenario = (typeof NOTE_SCENARIOS)[number];

export function noteScenarioFromLocation(search: string = globalThis.location?.search ?? ''): NoteScenario {
  const name = new URLSearchParams(search).get('notes');

  return NOTE_SCENARIOS.find((scenario) => scenario === name) ?? 'ok';
}

/** Browser-preview switch `?filters=old`: a Harness Memory that cannot combine filters (no `scope`, no scope counts), which the view then answers with the one kind filter and a hint to update. */
export function combinesFilters(search: string = globalThis.location?.search ?? ''): boolean {
  return new URLSearchParams(search).get('filters') !== 'old';
}

/** The notes of Harness Memory kept in memory: `memory_notes_list/note_get/note_delete/notes_delete_all` like the real backend, without a server. */
export interface NoteHandlers {
  memory_notes_list: (args: CommandArgs<'memory_notes_list'>) => MemoryNotesPage;
  memory_note_get: (args: CommandArgs<'memory_note_get'>) => NoteDetail;
  memory_note_delete: (args: CommandArgs<'memory_note_delete'>) => null;
  memory_notes_delete_all: (args: CommandArgs<'memory_notes_delete_all'>) => MemoryDeleteAll;
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
 */
export function pageOfNotes(
  visible: NoteContent[],
  { offset, limit = DEFAULT_LIMIT, query = '', kind, scope, sort = 'updated', order }: CommandArgs<'memory_notes_list'>,
  combined = true,
): Omit<MemoryNotesPage, 'availability'> {
  const answering = visible.filter((found) => matches(found, query));
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
  };
}

export function memoryNotesHandlers(availability: () => MemoryScriptsAvailability, scenario: () => NoteScenario = noteScenarioFromLocation, combined: () => boolean = combinesFilters): NoteHandlers {
  let notes: NoteContent[] | null = null;
  const current = (): NoteContent[] => (notes ??= scenario() === 'ok' ? sampleNotes() : []);

  /** A project sees its repository's notes and the global ones, no project only the global ones. */
  const visible = (project: string | null): NoteContent[] => current().filter((found) => found.scope === 'global' || project !== null);

  const find = (id: number, project: string | null): NoteContent => {
    const found = visible(project).find((candidate) => candidate.id === id);
    if (!found) {
      throw IpcFailure.from({ code: 'notFound', message: `note ${id} was not found` });
    }

    return found;
  };

  return {
    memory_notes_list: (args) => {
      const state = availability();
      if (state.kind !== 'available') {
        return { availability: state, total: 0, kinds: [], scopes: null, combinedFilters: combined(), notes: [] };
      }

      return { availability: state, ...pageOfNotes(visible(args.project), args, combined()) };
    },
    memory_note_get: ({ id, project }) => detailOf(find(id, project), visible(project)),
    memory_note_delete: ({ id, project }) => {
      const found = find(id, project);
      notes = current().filter((candidate) => candidate !== found);

      return null;
    },
    memory_notes_delete_all: ({ project = null }) => {
      requireServer(availability());

      const gone = visible(project);
      notes = current().filter((candidate) => !gone.includes(candidate));

      return deletedAll(gone);
    },
  };
}
