import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { AuthorRef } from '$lib/ipc/bindings/AuthorRef';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { sampleScripts } from '$lib/ipc/mock/memory-scripts-fixtures';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, fakeMemory, filterChoices, pickFilter, REPO_REACH, registerMemoryHooks, showMemory, showNotes } from './memory-test-support';
import NotesView from './NotesView.svelte';
import ScriptsView from './ScriptsView.svelte';

registerMemoryHooks();

const ADA: AuthorRef = { name: 'Ada Lovelace', via: null };
const GRACE: AuthorRef = { name: 'Grace Hopper', via: null };
/** A person whose name is the word of a menu choice. */
const ALL: AuthorRef = { name: 'all', via: null };

/** Who added each of the first five entries: two Ada, one Grace, one called "all" and one unknown. */
const WHO: (AuthorRef | null)[] = [ADA, GRACE, ALL, null, ADA];

const authored = <T extends { addedBy?: AuthorRef | null }>(entries: T[], who: (AuthorRef | null)[] = WHO): T[] => entries.slice(0, who.length).map((entry, index) => ({ ...entry, addedBy: who[index] }));

/** Sets what the server's feature lines say. */
function withRemote(invoke: ReturnType<typeof fakeMemory>, patch: Partial<MemoryRemote>): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation((async (name: CommandName, args: never) => {
    if (name !== 'memory_remote_get') {
      return real(name as never, args);
    }

    return { ...(await real('memory_remote_get', undefined)), ...patch };
  }) as typeof ipc.invoke);
}

const rowOf = (id: number) => document.querySelector<HTMLElement>(`[data-row="${id}"]`);

/** The two lists the filter is in, driven the same way: what to open, what to ask the server, which rows are whose. */
interface Surface {
  name: string;
  show: () => Promise<void>;
  command: 'memory_notes_list' | 'memory_scripts_list';
  start: (who?: (AuthorRef | null)[]) => ReturnType<typeof fakeMemory>;
  ids: number[];
  author: () => unknown;
  setAuthor: (next: null | { kind: 'name'; name: string } | { kind: 'unknown' }) => void;
  sort: () => unknown;
  setSort: (key: string) => void;
  searchLabel: string;
  noFilter: string;
  /** What finds exactly the entry at position `index` in a search. */
  finder: (index: number) => string;
}

const noteList = () => authored(sampleNotes());
const scriptList = () => authored(sampleScripts());

const SURFACES: Surface[] = [
  {
    name: 'notes',
    show: showNotes,
    command: 'memory_notes_list',
    start: (who) => fakeMemory({ kind: 'available' }, undefined, undefined, who ? authored(sampleNotes(), who) : noteList()),
    ids: noteList().map((note) => note.id),
    author: () => viewState.notesAuthor,
    setAuthor: (next) => (viewState.notesAuthor = next),
    sort: () => viewState.notesSort,
    setSort: (key) => (viewState.notesSort = { key, desc: false }),
    searchLabel: 'Search notes',
    noFilter: 'No notes match these filters.',
    finder: (index) => noteList()[index].title,
  },
  {
    name: 'scripts',
    show: showMemory,
    command: 'memory_scripts_list',
    start: (who) => fakeMemory({ kind: 'available' }, who ? authored(sampleScripts(), who) : scriptList()),
    ids: scriptList().map((script) => script.id),
    author: () => viewState.scriptsAuthor,
    setAuthor: (next) => (viewState.scriptsAuthor = next),
    sort: () => viewState.scriptsSort,
    setSort: (key) => (viewState.scriptsSort = { key, desc: false }),
    searchLabel: 'Search scripts',
    noFilter: 'No scripts match these filters.',
    finder: (index) => scriptList()[index].name,
  },
];

describe.each(SURFACES)('the Added by filter of the $name', (surface) => {
  const lastAsk = (invoke: ReturnType<typeof fakeMemory>) => calls(invoke, surface.command).at(-1) as Record<string, unknown>;
  const ofPerson = (person: AuthorRef | null) => surface.ids.filter((_, index) => (WHO[index]?.name ?? null) === (person?.name ?? null));

  it('lists everyone, each person with the count of what they added, and Unknown with its count', async () => {
    surface.start();
    await surface.show();

    const choices = await filterChoices('Added by: All', 'Added by');

    // The people come in the server's order: the most entries first.
    expect(choices).toEqual(['All 5', 'Ada Lovelace 2', 'all 1', 'Grace Hopper 1', 'Unknown 1']);
  });

  it('has no Unknown choice while every entry has a name', async () => {
    surface.start([ADA, GRACE, ADA, GRACE, GRACE]);
    await surface.show();

    const choices = await filterChoices('Added by: All', 'Added by');

    expect(choices).toEqual(['All 5', 'Grace Hopper 3', 'Ada Lovelace 2']);
  });

  it('narrows the list on the server to one person from the first page, and back with the clear button', async () => {
    const invoke = surface.start();
    await surface.show();

    expect(lastAsk(invoke)).not.toHaveProperty('author', expect.anything());

    await pickFilter('Added by: All', /^Grace Hopper/);

    await waitFor(() => expect(lastAsk(invoke)).toMatchObject({ author: { kind: 'name', name: 'Grace Hopper' }, offset: 0 }));
    await waitFor(() => expect(rowOf(ofPerson(ADA)[0])).toBeNull());
    expect(rowOf(ofPerson(GRACE)[0])).not.toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Added by filter' }));

    await waitFor(() => expect(lastAsk(invoke).author).toBeUndefined());
    await waitFor(() => expect(rowOf(ofPerson(ADA)[0])).not.toBeNull());
    expect(surface.author()).toBeNull();
  });

  it('keeps the other choices while one is chosen, and goes back to everyone with All', async () => {
    const invoke = surface.start();
    await surface.show();

    await pickFilter('Added by: All', /^Ada Lovelace/);
    await waitFor(() => expect(lastAsk(invoke)).toMatchObject({ author: { kind: 'name', name: 'Ada Lovelace' } }));

    expect(await filterChoices('Added by: Ada Lovelace', 'Added by')).toEqual(['All 5', 'Ada Lovelace 2', 'all 1', 'Grace Hopper 1', 'Unknown 1']);

    await pickFilter('Added by: Ada Lovelace', /^All/);

    await waitFor(() => expect(lastAsk(invoke).author).toBeUndefined());
    expect(surface.author()).toBeNull();
  });

  it('asks for the entries nobody is known to have added with Unknown', async () => {
    const invoke = surface.start();
    await surface.show();

    await pickFilter('Added by: All', /^Unknown/);

    await waitFor(() => expect(lastAsk(invoke)).toMatchObject({ author: { kind: 'unknown' } }));
    await waitFor(() => expect(rowOf(ofPerson(ADA)[0])).toBeNull());
    expect(rowOf(ofPerson(null)[0])).not.toBeNull();
  });

  it('tells a person called all from the All choice', async () => {
    const invoke = surface.start();
    await surface.show();

    await pickFilter('Added by: All', /^all/);

    await waitFor(() => expect(lastAsk(invoke)).toMatchObject({ author: { kind: 'name', name: 'all' } }));
    await waitFor(() => expect(rowOf(ofPerson(ADA)[0])).toBeNull());
    expect(rowOf(ofPerson(ALL)[0])).not.toBeNull();
    expect(screen.getByRole('button', { name: 'Added by: all' })).toBeInTheDocument();
  });

  it('keeps a chosen person who has nothing, says no entry matches the filters, and clears from the menu button', async () => {
    const invoke = surface.start();
    await surface.show();
    surface.setAuthor({ kind: 'name', name: 'Nobody Atall' });

    expect(await screen.findByText(surface.noFilter)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Added by: Nobody Atall' })).toBeInTheDocument();
    expect(lastAsk(invoke)).toMatchObject({ author: { kind: 'name', name: 'Nobody Atall' } });

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Added by filter' }));

    await waitFor(() => expect(rowOf(surface.ids[0])).not.toBeNull());
    expect(screen.queryByText(surface.noFilter)).toBeNull();
  });

  it('sorts by the column from A to Z first, then the other way, asking the server', async () => {
    const invoke = surface.start();
    await surface.show();

    const header = screen.getByRole('columnheader', { name: 'Added by' });
    expect(header).toHaveAttribute('aria-sort', 'none');

    await fireEvent.click(within(header).getByRole('button', { name: 'Added by' }));
    await waitFor(() => expect(lastAsk(invoke)).toMatchObject({ sort: 'addedBy', order: 'asc', offset: 0 }));
    expect(header).toHaveAttribute('aria-sort', 'ascending');

    await fireEvent.click(within(header).getByRole('button', { name: 'Added by' }));
    await waitFor(() => expect(lastAsk(invoke)).toMatchObject({ sort: 'addedBy', order: 'desc' }));
  });

  it('has no menu when every entry is unknown and nothing narrows the list', async () => {
    surface.start([null, null, null]);
    await surface.show();

    expect(rowOf(surface.ids[0])).not.toBeNull();
    expect(screen.queryByRole('button', { name: /^Added by:/ })).toBeNull();
  });

  it('keeps the menu when a search leaves only entries nobody added, and offers the way back', async () => {
    const invoke = surface.start();
    await surface.show();

    expect(screen.getByRole('button', { name: 'Added by: All' })).toBeInTheDocument();

    // The entry at position 3 has no known author.
    await fireEvent.input(screen.getByLabelText(surface.searchLabel), { target: { value: surface.finder(3) } });

    await waitFor(() => expect(lastAsk(invoke)).toMatchObject({ query: surface.finder(3) }));
    await waitFor(() => expect(rowOf(surface.ids[0])).toBeNull());
    expect(await filterChoices('Added by: All', 'Added by')).toEqual(['All 1', 'Unknown 1']);
  });

  it('forgets the choice when the view is reset', () => {
    surface.setAuthor({ kind: 'unknown' });

    viewState.reset(surface.name === 'notes' ? 'notes' : 'scripts');

    expect(surface.author()).toBeNull();
  });

  it('drops the choice and the sort for a server that cannot filter by author, and offers neither', async () => {
    surface.setAuthor({ kind: 'name', name: 'Ada Lovelace' });
    surface.setSort('addedBy');
    const invoke = surface.start();
    withRemote(invoke, { notesAuthorFilterSupported: false });
    await surface.show();

    await waitFor(() => expect(memoryRemote.value?.notesAuthorFilterSupported).toBe(false));
    await waitFor(() => expect(surface.author()).toBeNull());
    await waitFor(() => expect(surface.sort()).toEqual({ key: 'updated', desc: true }));

    await waitFor(() => expect(lastAsk(invoke).author).toBeUndefined());
    expect(lastAsk(invoke)).toMatchObject({ sort: 'updated', order: 'desc' });

    expect(screen.queryByRole('button', { name: /^Added by:/ })).toBeNull();
    expect(screen.getByRole('columnheader', { name: 'Added by' })).not.toHaveAttribute('aria-sort');
  });
});

describe('an empty project', () => {
  it('shows no Added by menu in the notes', async () => {
    fakeMemory({ kind: 'available' }, undefined, undefined, []);
    nav.go('notes');
    render(NotesView);

    expect(await screen.findByText('No notes yet')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Added by:/ })).toBeNull();
  });

  it('shows no Added by menu in the scripts', async () => {
    fakeMemory({ kind: 'available' }, []);
    nav.go('scripts');
    render(ScriptsView);

    expect(await screen.findByText('No scripts yet')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Added by:/ })).toBeNull();
  });
});

describe('turning pages in the notes', () => {
  it('starts again at the first page when a person is chosen', async () => {
    const invoke = fakeMemory();
    await showNotes();

    viewState.notesPage = 1;
    await waitFor(() => expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ offset: 50 }));

    await pickFilter('Added by: All', /^Grace Hopper/);

    await waitFor(() => expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ author: { kind: 'name', name: 'Grace Hopper' }, offset: 0 }));
    expect(viewState.notesPage).toBe(0);
  });

  it('keeps Delete all on while the filter leaves no note, as the list is narrowed', async () => {
    viewState.memoryReach = REPO_REACH;
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    // A server that counts the kinds under this filter too: nothing is left to count.
    invoke.mockImplementation((async (name: CommandName, args: never) => {
      const answer = await real(name as never, args);

      if (name === 'memory_notes_list' && (args as { author?: unknown }).author) {
        return { ...(answer as object), total: 0, notes: [], kinds: [] };
      }

      return answer;
    }) as typeof ipc.invoke);
    await showNotes();

    await pickFilter('Added by: All', /^Ada Lovelace/);
    await screen.findByText('No notes match these filters.');

    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));

    expect(await screen.findByRole('menuitem', { name: 'Delete all notes…' })).toBeEnabled();
  });
});
