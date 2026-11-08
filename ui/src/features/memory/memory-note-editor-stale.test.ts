import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls, fakeMemory, PROJECT, registerMemoryHooks } from './memory-test-support';
import { fill, openEdit, openEditor, TAKEN_TITLE } from './note-editor-support';

registerMemoryHooks();

const TITLE = 'Tauri window stayed blank after a hot reload on Wayland';
const OPENED_REVISION = sampleNotes().find((note) => note.id === 4)?.revision;
const TEAMMATE = 'Changed by a teammate.';

const toastTitles = () => toasts.items.map((toast) => toast.title);

type Sent = { replaceId?: number; expectedRevision?: number };

const revisionsSent = (invoke: ReturnType<typeof fakeMemory>) => calls(invoke, 'memory_note_save').map((call) => (call as Sent).expectedRevision);

/** A teammate rewrites the note while the editor is open on it. */
async function teammateRewrites(): Promise<void> {
  await ipc.invoke('memory_note_save', { project: PROJECT, replaceId: 4, draft: { title: TITLE, body: TEAMMATE, kind: 'fix', tags: ['linux'], entities: [], scope: 'repo' } });
}

/** What is saved for the note now. */
async function storedBody(): Promise<string> {
  return (await ipc.invoke('memory_note_get', { id: 4, project: null })).note.body;
}

describe('saving an edit of a note against the revision it opened', () => {
  it('sends the revision the editor opened', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit(4, TITLE);

    await fill(dialog, { body: 'My edit.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Edit note' })).not.toBeInTheDocument());
    expect(revisionsSent(invoke)).toEqual([OPENED_REVISION]);
    expect(toastTitles()).toContain('Note saved');
  });

  it('sends none for a new note', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { title: 'Release checklist', body: 'Tag the release.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a note' })).not.toBeInTheDocument());

    expect(calls(invoke, 'memory_note_save')).toHaveLength(1);
    expect(calls(invoke, 'memory_note_save')[0]).not.toHaveProperty('expectedRevision');
  });

  it('sends none for a new note that replaces the note of that title', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { title: TAKEN_TITLE, body: 'A newer answer.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));
    await fireEvent.click(await within(dialog).findByRole('button', { name: 'Replace it' }));
    await waitFor(() => expect(calls(invoke, 'memory_note_save')).toHaveLength(2));

    expect(calls(invoke, 'memory_note_save').at(-1)).toMatchObject({ replaceId: 3 });
    expect(calls(invoke, 'memory_note_save').at(-1)).not.toHaveProperty('expectedRevision');
  });
});

describe('a note that changed while it was being edited', () => {
  it('keeps the dialog and the edits, says so inline and not as a failure, and overwrites nothing', async () => {
    fakeMemory();
    const dialog = await openEdit(4, TITLE);

    await teammateRewrites();
    await fill(dialog, { body: 'My edit.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    expect(await within(dialog).findByText('This note changed since you opened it. Reload it to see the latest version.')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('Content')).toHaveValue('My edit.');
    expect(screen.getByRole('dialog', { name: 'Edit note' })).toBeInTheDocument();
    expect(toastTitles()).not.toContain('Note saved');
    expect(toastTitles()).not.toContain('The note was not saved');
    expect(await storedBody()).toBe(TEAMMATE);
  });

  it('fills the form with the latest note on Reload, drops the edits and the notice, and saves the next edit against that revision', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit(4, TITLE);

    await teammateRewrites();
    await fill(dialog, { body: 'My edit.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
    await fireEvent.click(await within(dialog).findByRole('button', { name: 'Reload' }));

    await waitFor(() => expect(within(dialog).getByLabelText('Content')).toHaveValue(TEAMMATE));
    expect(within(dialog).queryByRole('button', { name: 'Reload' })).not.toBeInTheDocument();
    expect(within(dialog).queryByText(/changed since you opened it/)).not.toBeInTheDocument();

    await fill(dialog, { body: `${TEAMMATE} And mine.` });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Edit note' })).not.toBeInTheDocument());
    expect(revisionsSent(invoke).filter((revision) => revision !== undefined)).toEqual([OPENED_REVISION, (OPENED_REVISION ?? 0) + 1]);
    expect(await storedBody()).toBe(`${TEAMMATE} And mine.`);
  });

  it('says the note no longer exists when it was deleted, cannot be saved, and offers Close', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit(4, TITLE);

    await ipc.invoke('memory_note_delete', { id: 4, project: null });
    await fill(dialog, { body: 'My edit.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
    await fireEvent.click(await within(dialog).findByRole('button', { name: 'Reload' }));

    expect(await within(dialog).findByText('This note no longer exists. It was deleted since you opened it.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Save' })).toBeDisabled();
    expect(calls(invoke, 'memory_note_save').filter((call) => (call as Sent).expectedRevision !== undefined)).toHaveLength(1);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Edit note' })).not.toBeInTheDocument());
  });
});
