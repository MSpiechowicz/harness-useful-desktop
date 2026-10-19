import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, vi } from 'vitest';

import type { EcosystemStatus } from '$lib/ipc/bindings/EcosystemStatus';
import type { EcosystemRelease } from '$lib/ipc/bindings/EcosystemRelease';
import type { MemoryScriptsAvailability } from '$lib/ipc/bindings/MemoryScriptsAvailability';
import type { NoteContent } from '$lib/ipc/bindings/NoteContent';
import type { NoteSaved } from '$lib/ipc/bindings/NoteSaved';
import type { NotesImportPreview } from '$lib/ipc/bindings/NotesImportPreview';
import type { NotesImportResult } from '$lib/ipc/bindings/NotesImportResult';
import type { NotesImportRow } from '$lib/ipc/bindings/NotesImportRow';
import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { sampleScripts, summaryOf } from '$lib/ipc/mock/memory-scripts-fixtures';
import { detailOf, sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { pageOfNotes } from '$lib/ipc/mock/memory-notes-handlers';
import { pageOfScripts } from '$lib/ipc/mock/memory-scripts-handlers';
import { IpcFailure } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { noteDetail, noteList } from '$lib/stores/memoryNotes.svelte';
import { scriptDetail, scriptList } from '$lib/stores/memoryScripts.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { startMockApp } from '../../../tests/mock-app';
import { noteEditor, scriptEditor } from './editor-state.svelte';
import NotesView from './NotesView.svelte';
import ScriptsView from './ScriptsView.svelte';

export const PROJECT = fixtures.recentProjects[0].path;

/** What GitHub says about the latest Harness Memory release next to the installed one. */
export type MemoryRelease = Pick<EcosystemRelease, 'latestVersion' | 'updateAvailable'>;

export const NEWER_RELEASE: MemoryRelease = { latestVersion: '1.1.0', updateAvailable: true };

/** The installed build is a source build, or already the latest release: nothing newer exists. */
export const NO_NEWER_RELEASE: MemoryRelease = { latestVersion: '1.1.0', updateAvailable: false };

/** Whether two titles are the same to the server: any case, the outer spaces dropped. */
const sameTitle = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/** The repository the fake keeps repository notes and scripts of. */
const REPO = 'github.com/useful/harness-desktop';

/** The file the picker of a fake Memory hands over: its review, or null for a picker that was closed. `hold` keeps the import of the first row from finishing until it resolves. */
export interface ImportFile {
  preview: NotesImportPreview | null;
  hold?: Promise<void>;
}

export const importRow = (index: number, title: string, rest: Partial<NotesImportRow> = {}): NotesImportRow => ({
  index,
  title,
  kind: 'note',
  tags: [],
  scope: null,
  bodyPreview: `About ${title}`,
  problems: [],
  duplicateOf: null,
  ...rest,
});

/** The title of the sample file's row that a repository note of the fake Memory already has. */
export const IMPORT_TAKEN_TITLE = 'Notes are read through the Memory CLI, never its database';

/** A review of a file with a new note, one with the title of a repository note, one with a problem, a repeat of the first row and a global note. */
export const SAMPLE_IMPORT: NotesImportPreview = {
  kind: 'plan',
  plan: {
    planId: 'plan-1',
    fileName: 'team-notes.csv',
    format: 'csv',
    truncated: false,
    rows: [
      importRow(0, 'Retry a flaky test once', { kind: 'decision', tags: ['ci'] }),
      importRow(1, IMPORT_TAKEN_TITLE),
      importRow(2, '', { problems: [{ code: 'missing', field: 'title' }] }),
      importRow(3, 'Retry a flaky test once', { duplicateOf: 0 }),
      importRow(4, 'Prefer the release profile for benchmarks', { scope: 'global' }),
    ],
  },
};

/**
 * A Harness Memory that keeps its scripts in this array and answers the three commands like the real one. A function as `availability` is asked at every read, so a test can change what the probe finds.
 * `combined: false` is an older one, which cannot combine the scope with the kind or the status (a function is asked at every read, so a test can update it).
 */
export function fakeMemory(availability: MemoryScriptsAvailability | (() => MemoryScriptsAvailability) = { kind: 'available' }, initial = sampleScripts(), release: MemoryRelease = NEWER_RELEASE, initialNotes = sampleNotes(), combined: boolean | (() => boolean) = true, importFile: ImportFile = { preview: SAMPLE_IMPORT }) {
  let scripts = initial;
  let notes = initialNotes;
  const probe = () => (typeof availability === 'function' ? availability() : availability);
  const combines = () => (typeof combined === 'function' ? combined() : combined);
  const passthrough = ipc.invoke.bind(ipc);

  /** Like the real server: a title taken in the scope is refused (`exists`), a save by id rewrites that note and keeps its scope. */
  const saveNote = ({ draft, replaceId }: CommandArgs<'memory_note_save'>): NoteSaved => {
    const now = new Date().toISOString();
    const rewrite = (found: NoteContent): NoteContent => ({ ...found, title: draft.title, body: draft.body, kind: draft.kind, tags: draft.tags, revision: found.revision + 1, updatedAt: now });

    if (replaceId !== undefined) {
      const found = notes.find((note) => note.id === replaceId);
      if (!found) {
        throw IpcFailure.from({ code: 'notFound', message: 'gone' });
      }

      const other = notes.find((note) => note.id !== found.id && note.scope === found.scope && sameTitle(note.title, draft.title));
      if (other) {
        return { outcome: 'exists', id: other.id };
      }

      notes = notes.map((note) => (note.id === found.id ? rewrite(note) : note));

      return { outcome: 'updated', id: found.id };
    }

    const same = notes.find((note) => note.scope === draft.scope && sameTitle(note.title, draft.title));
    if (same) {
      return { outcome: 'exists', id: same.id };
    }

    const id = Math.max(...notes.map((note) => note.id)) + 1;
    const created = rewrite({ ...notes[0], id, scope: draft.scope, repo: draft.scope === 'repo' ? REPO : null, place: 'local', entities: draft.entities, createdAt: now, revision: 0, hits: 0 });
    notes = [...notes, created];

    return { outcome: 'created', id };
  };

  /**
   * The import of the rows ticked in the review, each saved like a note of the editor. A repository title that is taken skips the row, or replaces the note when asked to. A global
   * title that is taken is rewritten, as the server does. The review holds up the first row until `hold` resolves.
   */
  const importNotes = async ({ planId, rows, defaultScope, replace, project, onProgress }: CommandArgs<'memory_notes_import_apply'>): Promise<NotesImportResult> => {
    const plan = importFile.preview?.kind === 'plan' ? importFile.preview.plan : null;
    if (!plan || plan.planId !== planId) {
      throw IpcFailure.from({ code: 'invalidInput', message: 'this review has expired, so review the file again' });
    }

    const result: NotesImportResult = { created: 0, updated: 0, replaced: 0, exists: [], failed: [] };

    for (const [position, index] of rows.entries()) {
      const row = plan.rows.find((candidate) => candidate.index === index) as NotesImportRow;
      const draft = { title: row.title, body: row.bodyPreview, kind: row.kind, tags: row.tags, entities: [], scope: row.scope ?? defaultScope };
      const saved = saveNote({ project, draft });
      const taken = saved.outcome === 'exists' ? notes.find((note) => note.id === saved.id) : undefined;

      if (taken && (replace || taken.scope === 'global')) {
        saveNote({ project, draft, replaceId: taken.id });
        result[taken.scope === 'global' ? 'updated' : 'replaced'] += 1;
      } else if (taken) {
        result.exists.push({ index, id: taken.id });
      } else if (saved.outcome === 'created') {
        result.created += 1;
      }

      onProgress({ index, done: position + 1, total: rows.length });

      if (position === 0) {
        await importFile.hold;
      }
    }

    return result;
  };

  return vi.spyOn(ipc, 'invoke').mockImplementation(async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (name === 'memory_scripts_list') {
      const asked = args as CommandArgs<'memory_scripts_list'>;
      const visible = scripts.filter((script) => script.scope === 'global' || asked.project !== null);
      const found = probe();
      const page = found.kind === 'available' ? pageOfScripts(visible.map(summaryOf), asked, combines()) : { total: 0, groups: { needsReview: 0, repo: 0, global: 0 }, scopes: null, review: null, combinedFilters: combines(), scripts: [] };

      return { availability: found, ...page } as CommandResult<K>;
    }

    if (name === 'memory_notes_list') {
      const asked = args as CommandArgs<'memory_notes_list'>;
      const found = probe();
      const page = found.kind === 'available' ? pageOfNotes(notes.filter((note) => note.scope === 'global' || asked.project !== null), asked, combines()) : { total: 0, kinds: [], scopes: null, combinedFilters: combines(), notes: [] };

      return { availability: found, ...page } as CommandResult<K>;
    }

    if (name === 'memory_note_get') {
      const found = notes.find((note) => note.id === (args as CommandArgs<'memory_note_get'>).id);
      if (!found) {
        throw IpcFailure.from({ code: 'notFound', message: 'gone' });
      }

      // A copy, as the real server's answer is: a test that edits its note must not edit what the view already holds.
      return structuredClone(detailOf(found, notes)) as CommandResult<K>;
    }

    if (name === 'memory_note_save') {
      return saveNote(args as CommandArgs<'memory_note_save'>) as CommandResult<K>;
    }

    if (name === 'memory_notes_import_preview') {
      return importFile.preview as CommandResult<K>;
    }

    if (name === 'memory_notes_import_apply') {
      return (await importNotes(args as CommandArgs<'memory_notes_import_apply'>)) as CommandResult<K>;
    }

    if (name === 'memory_note_delete') {
      notes = notes.filter((note) => note.id !== (args as CommandArgs<'memory_note_delete'>).id);

      return null as CommandResult<K>;
    }

    // Deleting everything takes what the project sees: its repository's and the global ones.
    if (name === 'memory_notes_delete_all') {
      const gone = notes.filter((note) => note.scope === 'global' || (args as CommandArgs<'memory_notes_delete_all'>).project !== undefined);
      notes = notes.filter((note) => !gone.includes(note));

      return { deleted: gone.length, team: gone.filter((note) => note.place === 'team').length } as CommandResult<K>;
    }

    if (name === 'memory_scripts_delete_all') {
      const gone = scripts.filter((script) => script.scope === 'global' || (args as CommandArgs<'memory_scripts_delete_all'>).project !== undefined);
      scripts = scripts.filter((script) => !gone.includes(script));

      return { deleted: gone.length, team: gone.filter((script) => script.place === 'team').length } as CommandResult<K>;
    }

    if (name === 'memory_script_get') {
      const found = scripts.find((script) => script.id === (args as CommandArgs<'memory_script_get'>).id);
      if (!found) {
        throw IpcFailure.from({ code: 'notFound', message: 'gone' });
      }

      return structuredClone(found) as CommandResult<K>;
    }

    if (name === 'memory_script_delete') {
      scripts = scripts.filter((script) => script.id !== (args as CommandArgs<'memory_script_delete'>).id);

      return null as CommandResult<K>;
    }

    // Like the real server: a name taken in the scope is refused (`exists`) unless replacing, and a script saved here is trusted.
    if (name === 'memory_script_save') {
      const { draft, replace } = args as CommandArgs<'memory_script_save'>;
      const same = scripts.find((script) => script.name === draft.name && script.scope === draft.scope);
      if (same && !replace) {
        return { outcome: 'exists', id: same.id, script: null } as CommandResult<K>;
      }

      const now = new Date().toISOString();
      const id = same?.id ?? Math.max(...scripts.map((script) => script.id)) + 1;
      const written: ScriptDetail = {
        ...(same ?? scripts[0]),
        ...draft,
        id,
        repo: draft.scope === 'repo' ? REPO : null,
        sha256: `${id}`.padStart(64, 'a'),
        place: 'local',
        trusted: true,
        approval: 'local',
        updatedAt: now,
        createdAt: same?.createdAt ?? now,
        runCount: same?.runCount ?? 0,
      };
      scripts = same ? scripts.map((script) => (script === same ? written : script)) : [...scripts, written];

      return { outcome: same ? 'replaced' : 'created', id, script: summaryOf(written) } as CommandResult<K>;
    }

    if (name === 'memory_scripts_trust') {
      const { items } = args as CommandArgs<'memory_scripts_trust'>;
      const reviewed = (script: ScriptDetail) => items.some((item) => item.id === script.id && item.sha256 === script.sha256);
      scripts = scripts.map((script) => (reviewed(script) ? { ...script, trusted: true, approval: 'approved' } : script));

      // Like the real server: a script changed since it was reviewed is refused, the others are approved.
      if (items.some((item) => !scripts.some((script) => script.id === item.id && script.sha256 === item.sha256))) {
        throw IpcFailure.from({ code: 'conflict', message: 'a script changed since it was reviewed' });
      }

      return null as CommandResult<K>;
    }

    // Harness Memory is installed (it answers, if too old), next to whatever the Ecosystem page finds.
    if (name === 'ecosystem_status') {
      const status = (await passthrough(name, args)) as EcosystemStatus;

      return { ...status, items: status.items.map((item) => (item.id === 'memory' ? { ...item, installed: true, version: '1.0.1', updateAvailable: false } : item)) } as CommandResult<K>;
    }

    // The release lookup of the Ecosystem page, with the latest Harness Memory release the test chose.
    if (name === 'ecosystem_releases') {
      const others = (await passthrough(name, args) as EcosystemRelease[]).filter((entry) => entry.id !== 'memory');

      return [...others, { id: 'memory', ...release }] as CommandResult<K>;
    }

    return passthrough(name, args);
  });
}

export function calls(invoke: ReturnType<typeof fakeMemory>, name: string): unknown[] {
  return invoke.mock.calls.filter(([command]) => command === name).map(([, args]) => args);
}

/** The table row of a script, by name. */
export const rowButton = (name: string) => document.querySelector<HTMLElement>(`[data-row="${sampleScripts().find((script) => script.name === name)?.id}"]`);

/** Registers what every test of the Notes and Scripts views needs: a mock backend with the project open, and a clean view before and after each test. */
export function registerMemoryHooks(): void {
  beforeAll(async () => {
    // happy-dom lays nothing out: give every element a 640px viewport so the virtual list renders a window of rows.
    Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 640 });

    await startMockApp(PROJECT);
  });

  beforeEach(() => {
    viewState.reset('notes');
    viewState.reset('scripts');
    noteList.sync(null, null);
    noteDetail.sync(null, null);
    scriptList.sync(null, null);
    scriptDetail.sync(null, null);
    noteEditor.reset();
    scriptEditor.reset();
    ecosystem.reset();
  });

  afterEach(() => {
    nav.go('chat');
    toasts.items = [];
    vi.restoreAllMocks();
  });
}

/** The table row of a note, by id. */
export const noteRow = (id: number) => document.querySelector<HTMLElement>(`[data-row="${id}"]`);

/** Opens the Notes view and waits for its list. */
export async function showNotes(): Promise<void> {
  nav.go('notes');
  render(NotesView);
  await screen.findByRole('grid', { name: 'Notes' });
}

/** Opens the Scripts view and waits for its list. */
export async function showMemory(): Promise<void> {
  nav.go('scripts');
  render(ScriptsView);
  await screen.findByRole('grid', { name: 'Scripts' });
}

/** Opens a filter menu by its button and picks one of its choices. */
export async function pickFilter(button: string, choice: RegExp): Promise<void> {
  await fireEvent.click(screen.getByRole('button', { name: button }));
  await fireEvent.click(await screen.findByRole('menuitemradio', { name: choice }));
}

/** What a filter menu offers, each choice with its count as the user reads it; the menu is closed again. */
export async function filterChoices(button: string, menu: string): Promise<string[]> {
  await fireEvent.click(screen.getByRole('button', { name: button }));

  const open = await screen.findByRole('menu', { name: menu });
  const choices = within(open).getAllByRole('menuitemradio').map((item) => item.textContent?.trim() ?? '');

  await fireEvent.keyDown(open, { key: 'Escape' });

  return choices;
}
