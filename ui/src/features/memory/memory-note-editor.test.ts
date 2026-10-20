import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { IpcFailure } from '$lib/ipc/transport';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { projectChoice } from '$lib/components/scope/scope-options';
import { noteList } from '$lib/stores/memoryNotes.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { project } from '$lib/stores/project.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { noteEditor } from './editor-state.svelte';
import { appliesTo, calls, EMPTY_PROJECT, fakeMemory, noteRow, pickTarget, PROJECT, registerMemoryHooks, showNotes } from './memory-test-support';
import { chipsOf, fill, openEdit, openEditor, TAKEN_TITLE } from './note-editor-support';
import NotesView from './NotesView.svelte';

registerMemoryHooks();

/** Opens the Notes view where there is no table to show, only words, and waits for them. */
async function showWithoutTable(words: string): Promise<{ unmount: () => void }> {
  nav.go('notes');
  const view = render(NotesView);
  await screen.findByText(words);

  return view;
}

const toastTitles = () => toasts.items.map((toast) => toast.title);

describe('adding a note', () => {
  it('asks for the title first and does not add anything until the note is complete', async () => {
    fakeMemory();
    const dialog = await openEditor();

    expect(within(dialog).getByLabelText('Title')).toHaveFocus();
    expect(within(dialog).getByRole('button', { name: 'Add note' })).toBeDisabled();

    await fill(dialog, { title: 'Release checklist' });
    expect(within(dialog).getByRole('button', { name: 'Add note' })).toBeDisabled();

    await fill(dialog, { body: 'Tag the release, then publish the notes.' });
    expect(within(dialog).getByRole('button', { name: 'Add note' })).toBeEnabled();
  });

  it('saves the note in the open repository with its tags lowercased, says so, and opens its panel', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { title: '  Release checklist ', body: 'Tag the release, then publish the notes.', tags: 'Release, linux release' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a note' })).not.toBeInTheDocument());
    expect(calls(invoke, 'memory_note_save')).toEqual([{ project: PROJECT, draft: { title: 'Release checklist', body: 'Tag the release, then publish the notes.', kind: 'note', tags: ['release', 'linux'], entities: [], scope: 'repo' } }]);
    expect(toasts.items.map((toast) => [toast.title, toast.description])).toContainEqual(['Note added', 'Release checklist']);

    const panel = await screen.findByRole('region', { name: 'Release checklist' });
    expect(within(panel).getByText('Note · github.com/useful/harness-desktop')).toBeInTheDocument();
    expect(await within(panel).findByText('Tag the release, then publish the notes.')).toBeInTheDocument();
    expect(viewState.noteId).not.toBeNull();
  });

  it('keeps a note global when it is chosen, and offers Global with every project on this computer', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    expect(within(dialog).getByText('Applies to')).toBeInTheDocument();
    expect(appliesTo(dialog)).toHaveTextContent('demo');

    await fireEvent.click(appliesTo(dialog));
    const menu = await screen.findByRole('menu', { name: 'Applies to' });
    expect(within(menu).getAllByRole('menuitemradio').map((item) => item.textContent?.replace(/\s+/g, ' ').trim())).toEqual(['Global', 'demo useful/harness-desktop', 'legacy-api', 'research']);
    await fireEvent.click(within(menu).getByRole('menuitemradio', { name: 'Global' }));

    expect(appliesTo(dialog)).toHaveTextContent('Global');
    expect(within(dialog).getByText('For every project.')).toBeInTheDocument();

    await fill(dialog, { title: 'Release checklist', body: 'Tag the release.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(calls(invoke, 'memory_note_save')).toHaveLength(1));
    expect(calls(invoke, 'memory_note_save')[0]).toMatchObject({ project: null, draft: { scope: 'global' } });
  });

  it('saves the note in a recent project that is not the open one, and names that project in the help', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await pickTarget(dialog, /^research/);

    expect(appliesTo(dialog)).toHaveTextContent('research');
    expect(within(dialog).getByText('Only for research.')).toBeInTheDocument();

    await fill(dialog, { title: 'Release checklist', body: 'Tag the release.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(calls(invoke, 'memory_note_save')).toHaveLength(1));
    expect(calls(invoke, 'memory_note_save')[0]).toMatchObject({ project: EMPTY_PROJECT, draft: { scope: 'repo' } });
  });

  it('starts on what the Scope filter looks at', async () => {
    viewState.memoryReach = projectChoice({ path: EMPTY_PROJECT, key: `path:${EMPTY_PROJECT}` });
    const invoke = fakeMemory();
    // The project has no notes, so there is no table to wait for: the view says so and the Add button is there.
    nav.go('notes');
    render(NotesView);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Add note' })).toBeEnabled());
    await fireEvent.click(screen.getByRole('button', { name: 'Add note' }));
    const dialog = await screen.findByRole('dialog', { name: 'Add a note' });

    expect(appliesTo(dialog)).toHaveTextContent('research');
    expect(screen.getByRole('button', { name: 'Scope: research' })).toBeInTheDocument();

    await fill(dialog, { title: 'Release checklist', body: 'Tag the release.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(calls(invoke, 'memory_note_save')).toHaveLength(1));
    expect(calls(invoke, 'memory_note_save')[0]).toMatchObject({ project: EMPTY_PROJECT, draft: { scope: 'repo' } });
  });

  it('starts on Global when the Scope filter looks at the global notes', async () => {
    viewState.memoryReach = 'global';
    fakeMemory();
    const dialog = await openEditor();

    expect(appliesTo(dialog)).toHaveTextContent('Global');
  });

  it('starts on the open project when the Scope filter looks at a repository that is not on this computer', async () => {
    viewState.memoryReach = 'repo:git:github.com/acme/widget';
    let moved = 0;
    fakeMemory({ kind: 'available' }, undefined, undefined, sampleNotes().map((note) => (note.scope === 'repo' && moved++ < 5 ? { ...note, repo: 'github.com/acme/widget' } : note)));
    const dialog = await openEditor();

    expect(appliesTo(dialog)).toHaveTextContent('demo');
  });

  it('says what is wrong with a field once the user has been at it, and keeps the note from being added', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();
    const title = within(dialog).getByLabelText('Title');

    expect(within(dialog).queryByText('Give the note a title.')).not.toBeInTheDocument();
    await fireEvent.blur(title);
    expect(within(dialog).getByText('Give the note a title.')).toBeInTheDocument();

    await fill(dialog, { title: 'x'.repeat(121), body: 'y'.repeat(4001), tags: 'ok, not ok!, a1, a2, a3, a4, a5, a6, a7,' });

    expect(within(dialog).getByText('The title is longer than 120 characters.')).toBeInTheDocument();
    expect(within(dialog).getByText('The content is longer than 4000 characters.')).toBeInTheDocument();
    expect(within(dialog).getByText('Use at most 8 tags.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Add note' })).toBeDisabled();

    for (const tag of ['a1', 'a2', 'a3', 'a4', 'a5', 'a6', 'a7']) {
      await fireEvent.click(within(dialog).getByRole('button', { name: `Remove tag ${tag}` }));
    }

    await fill(dialog, { title: 'Fine', body: 'Fine' });
    expect(within(dialog).getByText('Tags use lowercase letters, digits, - and _ only.')).toBeInTheDocument();

    await fireEvent.submit(title.closest('form') as HTMLFormElement);
    expect(calls(invoke, 'memory_note_save')).toHaveLength(0);
  });

  it('offers to open or to replace a repository note that has the title, and writes nothing until one is chosen', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { title: TAKEN_TITLE, body: 'A newer answer.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    expect(await within(dialog).findByText('This title is taken')).toBeInTheDocument();
    expect(toastTitles()).not.toContain('Note added');

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Replace it' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a note' })).not.toBeInTheDocument());
    expect(calls(invoke, 'memory_note_save').at(-1)).toMatchObject({ replaceId: 3, draft: { title: TAKEN_TITLE, body: 'A newer answer.' } });
    expect(toastTitles()).toContain('Note saved');
    expect(viewState.noteId).toBe(3);
    expect(await screen.findByText('A newer answer.')).toBeInTheDocument();
  });

  it('opens the note that has the title instead of saving', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { title: TAKEN_TITLE.toLowerCase(), body: 'A newer answer.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));
    await fireEvent.click(await within(dialog).findByRole('button', { name: 'Open it' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a note' })).not.toBeInTheDocument());
    expect(await screen.findByRole('region', { name: TAKEN_TITLE })).toBeInTheDocument();
    expect(viewState.noteId).toBe(3);
    expect(calls(invoke, 'memory_note_save')).toHaveLength(1);
  });

  it('drops the finding once the title is changed', async () => {
    fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { title: TAKEN_TITLE, body: 'A newer answer.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));
    await within(dialog).findByText('This title is taken');

    await fill(dialog, { title: `${TAKEN_TITLE} (2)` });

    expect(within(dialog).queryByText('This title is taken')).not.toBeInTheDocument();
  });

  it('drops the finding once Applies to moves to another project, so Replace never reaches a note of the first one', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { title: TAKEN_TITLE, body: 'A newer answer.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));
    await within(dialog).findByText('This title is taken');

    await pickTarget(dialog, /^research/);

    expect(within(dialog).queryByText('This title is taken')).not.toBeInTheDocument();
    expect(within(dialog).queryByRole('button', { name: 'Replace it' })).not.toBeInTheDocument();
    expect(calls(invoke, 'memory_note_save').filter((args) => (args as { replaceId?: number }).replaceId !== undefined)).toHaveLength(0);

    // Back on the project it was found for, the finding stands again.
    await pickTarget(dialog, /^demo/);
    expect(within(dialog).getByText('This title is taken')).toBeInTheDocument();
  });

  it('keeps the dialog open and says why when the note could not be saved', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();
    const base = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<unknown>;
    invoke.mockImplementation((async (name: CommandName, args: unknown) => {
      if (name === 'memory_note_save') {
        throw IpcFailure.from({ code: 'process', message: 'harness-memory stopped' });
      }

      return base(name, args);
    }) as typeof ipc.invoke);

    await fill(dialog, { title: 'Release checklist', body: 'Tag the release.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(toasts.items.map((toast) => [toast.title, toast.description])).toContainEqual(['The note was not saved', 'harness-memory stopped']));
    expect(screen.getByRole('dialog', { name: 'Add a note' })).toBeInTheDocument();
  });

  it('says to update Harness Memory when the installed one is too old to save', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();
    const base = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<unknown>;
    invoke.mockImplementation((async (name: CommandName, args: unknown) => {
      if (name === 'memory_note_save') {
        throw IpcFailure.from({ code: 'unsupported', message: 'this harness-memory predates saving notes and scripts from the app; update Harness Memory' });
      }

      return base(name, args);
    }) as typeof ipc.invoke);

    await fill(dialog, { title: 'Release checklist', body: 'Tag the release.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(toasts.items.map((toast) => toast.description)).toContain('A newer Harness Memory is needed to save from the app. Update it on the Ecosystem page.'));
  });
});

describe('editing a note', () => {
  const TITLE = 'Tauri window stayed blank after a hot reload on Wayland';

  it('opens on the note as it is, with its scope kept', async () => {
    fakeMemory();
    const dialog = await openEdit(4, TITLE);
    const note = sampleNotes().find((candidate) => candidate.id === 4);

    expect(within(dialog).getByLabelText('Title')).toHaveValue(TITLE);
    expect(within(dialog).getByLabelText('Content')).toHaveValue(note?.body);
    expect(chipsOf(dialog, 'tag')).toEqual(note?.tags);
    expect(appliesTo(dialog)).toHaveTextContent('demo');
    expect(appliesTo(dialog)).toBeDisabled();
    expect(within(dialog).getByText('A note keeps its scope.')).toBeInTheDocument();
  });

  it('shows the project of the note once the projects are known, when they were not as the editor opened', async () => {
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<unknown>;
    let release = () => {};
    const known = new Promise<void>((resolve) => (release = resolve));
    invoke.mockImplementation((async (name: CommandName, args: unknown) => {
      if (name === 'project_scopes') {
        await known;
      }

      return real(name, args);
    }) as typeof ipc.invoke);

    const dialog = await openEdit(4, TITLE);
    expect(appliesTo(dialog)).not.toHaveTextContent('demo');

    release();

    await waitFor(() => expect(appliesTo(dialog)).toHaveTextContent('demo'));
    expect(appliesTo(dialog)).toBeDisabled();
  });

  it('shows the repository of a note that is not on this computer, and saves it from the open project', async () => {
    let moved = 0;
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, sampleNotes().map((note) => (note.scope === 'repo' && moved++ < 5 ? { ...note, repo: 'github.com/acme/widget' } : note)));
    await showNotes();
    await fireEvent.click(noteRow(3) as HTMLElement);
    const panel = await screen.findByRole('region', { name: TAKEN_TITLE });
    await fireEvent.click(await within(panel).findByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit note' });

    expect(appliesTo(dialog)).toHaveTextContent('widget');
    expect(appliesTo(dialog)).toBeDisabled();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_note_save')).toHaveLength(1));
    expect(calls(invoke, 'memory_note_save')[0]).toMatchObject({ project: PROJECT, replaceId: 3, draft: { scope: 'repo' } });
  });

  it('saves the changes to that note by id, says so, and shows them in its panel', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit(4, TITLE);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Remove tag webkit' }));
    await fill(dialog, { title: 'Blank window after a Wayland hot reload', tags: 'linux, wayland' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Edit note' })).not.toBeInTheDocument());
    expect(calls(invoke, 'memory_note_save')).toEqual([{ project: PROJECT, replaceId: 4, draft: expect.objectContaining({ title: 'Blank window after a Wayland hot reload', tags: ['linux', 'wayland'], scope: 'repo', kind: 'fix' }) }]);
    expect(toastTitles()).toContain('Note saved');

    const panel = await screen.findByRole('region', { name: 'Blank window after a Wayland hot reload' });
    expect(within(panel).getByText('wayland')).toBeInTheDocument();
    expect(noteEditor.open).toBe(false);
  });

  it('offers only to open the note that has the title it was renamed to, since replacing it would leave this note behind', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit(4, TITLE);

    await fill(dialog, { title: TAKEN_TITLE });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
    await within(dialog).findByText('This title is taken');

    expect(within(dialog).queryByRole('button', { name: 'Replace it' })).not.toBeInTheDocument();
    expect(within(dialog).getByText(/pick a different title/)).toBeInTheDocument();
    expect(calls(invoke, 'memory_note_save')).toHaveLength(1);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Open it' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Edit note' })).not.toBeInTheDocument());
    expect(viewState.noteId).toBe(3);
    expect(calls(invoke, 'memory_note_save')).toHaveLength(1);
  });
});

describe('where a note can be added', () => {
  it('is not offered while Harness Memory is missing, outdated or not answering', async () => {
    const cases = [
      [{ kind: 'notInstalled' }, 'Harness Memory is not installed'],
      [{ kind: 'outdated' }, 'Your memory needs an update'],
      [{ kind: 'failed', message: 'it crashed' }, 'Could not reach Harness Memory'],
    ] as const;

    for (const [availability, words] of cases) {
      fakeMemory(availability);
      const { unmount } = await showWithoutTable(words);

      expect(screen.queryByRole('button', { name: 'Add note' })).not.toBeInTheDocument();

      unmount();
    }
  });

  it('is offered in the empty list too', async () => {
    fakeMemory({ kind: 'available' }, undefined, undefined, []);
    await showWithoutTable('No notes yet');

    const buttons = await screen.findAllByRole('button', { name: 'Add note' });
    expect(buttons).toHaveLength(2);

    await fireEvent.click(buttons[1]);

    expect(await screen.findByRole('dialog', { name: 'Add a note' })).toBeInTheDocument();
  });

  it('is opened by the command palette once the list is up', async () => {
    fakeMemory();
    noteEditor.wantsNew = true;
    await showNotes();

    expect(await screen.findByRole('dialog', { name: 'Add a note' })).toBeInTheDocument();
    expect(noteEditor.wantsNew).toBe(false);
  });

  it('is not opened by the command palette while there is no Harness Memory to save to', async () => {
    fakeMemory({ kind: 'notInstalled' });
    noteEditor.wantsNew = true;
    await showWithoutTable('Harness Memory is not installed');

    expect(screen.queryByRole('dialog', { name: 'Add a note' })).not.toBeInTheDocument();
    expect(noteEditor.wantsNew).toBe(false);
  });

  it('is not opened later when Harness Memory comes back after it was not answering', async () => {
    let broken = true;
    fakeMemory(() => {
      if (broken) {
        throw IpcFailure.from({ code: 'process', message: 'harness-memory stopped' });
      }

      return { kind: 'available' };
    });
    noteEditor.wantsNew = true;
    nav.go('notes');
    render(NotesView);
    await screen.findByText('Could not read the notes');

    expect(noteEditor.wantsNew).toBe(false);

    broken = false;
    await noteList.refresh();
    await screen.findByRole('grid', { name: 'Notes' });

    expect(screen.queryByRole('dialog', { name: 'Add a note' })).not.toBeInTheDocument();
  });
});

describe('an editor that outlives nothing', () => {
  afterEach(async () => {
    await project.open(PROJECT);
  });

  it('closes when the view is left', async () => {
    fakeMemory();
    await showNotes();
    await fireEvent.click(await screen.findByRole('button', { name: 'Add note' }));
    await screen.findByRole('dialog', { name: 'Add a note' });

    cleanup();

    expect(noteEditor.open).toBe(false);
    expect(noteEditor.target).toBeNull();
  });

  it('closes when the project changes', async () => {
    fakeMemory();
    await showNotes();
    await fireEvent.click(await screen.findByRole('button', { name: 'Add note' }));
    await screen.findByRole('dialog', { name: 'Add a note' });

    project.active = null;

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a note' })).not.toBeInTheDocument());
    expect(noteEditor.open).toBe(false);
  });
});
