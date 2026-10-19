import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';

import { calls, fakeMemory, registerMemoryHooks } from './memory-test-support';
import { chipsOf, fill, openEdit, openEditor, TAKEN_TITLE, type } from './note-editor-support';

registerMemoryHooks();

describe('the tags and entities of a new note', () => {
  it('turns what is typed before a comma into a chip, and takes spaces too for tags only', async () => {
    fakeMemory();
    const dialog = await openEditor();

    await type(dialog, 'Tags', 'Release linux');
    await type(dialog, 'Entities', 'src/lib.rs, release pipeline,');

    expect(chipsOf(dialog, 'tag')).toEqual(['release']);
    expect(within(dialog).getByLabelText('Tags')).toHaveValue('linux');
    expect(chipsOf(dialog, 'entity')).toEqual(['src/lib.rs', 'release pipeline']);
    expect(within(dialog).getByLabelText('Entities')).toHaveValue('');
  });

  it('adds a chip with Enter without saving the note, and removes the last one with Backspace on an empty field', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();
    await fill(dialog, { title: 'Release checklist', body: 'Tag the release.' });

    const field = await type(dialog, 'Entities', 'ci.yml');
    await fireEvent.keyDown(field, { key: 'Enter' });
    await type(dialog, 'Entities', 'release.yml');
    await fireEvent.keyDown(field, { key: 'Enter' });

    expect(chipsOf(dialog, 'entity')).toEqual(['ci.yml', 'release.yml']);
    expect(calls(invoke, 'memory_note_save')).toEqual([]);

    await fireEvent.keyDown(field, { key: 'Backspace' });

    expect(chipsOf(dialog, 'entity')).toEqual(['ci.yml']);
  });

  it('removes a chip with its button, and drops a repeat whatever its case or spacing', async () => {
    fakeMemory();
    const dialog = await openEditor();

    await type(dialog, 'Entities', 'Cargo.toml, cargo.TOML,  release   pipeline , Release pipeline,');
    expect(chipsOf(dialog, 'entity')).toEqual(['Cargo.toml', 'release pipeline']);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Remove entity Cargo.toml' }));

    expect(chipsOf(dialog, 'entity')).toEqual(['release pipeline']);
  });

  it('splits a pasted list into chips', async () => {
    fakeMemory();
    const dialog = await openEditor();

    await fireEvent.paste(within(dialog).getByLabelText('Tags'), { clipboardData: { getData: () => 'Perf, build ci' } });
    await fireEvent.paste(within(dialog).getByLabelText('Entities'), { clipboardData: { getData: () => 'a.rs, b c.rs' } });

    expect(chipsOf(dialog, 'tag')).toEqual(['perf', 'build', 'ci']);
    expect(chipsOf(dialog, 'entity')).toEqual(['a.rs', 'b c.rs']);
  });

  it('saves the entities with the note, counting what is typed after the last chip', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { title: 'Release checklist', body: 'Tag the release.' });
    await type(dialog, 'Entities', 'ci.yml, release.yml');
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a note' })).not.toBeInTheDocument());
    expect(calls(invoke, 'memory_note_save')).toMatchObject([{ draft: { entities: ['ci.yml', 'release.yml'] } }]);

    const panel = await screen.findByRole('region', { name: 'Release checklist' });
    expect(within(panel).getByText('release.yml')).toBeInTheDocument();
  });

  it('keeps a note with more than 10 entities or a long one from being added', async () => {
    fakeMemory();
    const dialog = await openEditor();
    await fill(dialog, { title: 'Release checklist', body: 'Tag the release.' });

    await type(dialog, 'Entities', `${Array.from({ length: 11 }, (_, index) => `file-${index}.rs`).join(', ')},`);
    expect(within(dialog).getByText('Use at most 10 entities.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Add note' })).toBeDisabled();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Remove entity file-10.rs' }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Remove entity file-9.rs' }));
    expect(within(dialog).getByRole('button', { name: 'Add note' })).toBeEnabled();

    await type(dialog, 'Entities', `${'x'.repeat(81)},`);
    expect(within(dialog).getByText('An entity is longer than 80 characters.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Add note' })).toBeDisabled();
  });

  it('says that the entities typed are not saved when the note that has the title is replaced', async () => {
    const invoke = fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { title: TAKEN_TITLE, body: 'A newer answer.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));
    await within(dialog).findByText('This title is taken');

    expect(within(dialog).queryByText(/entities you typed are not saved/)).not.toBeInTheDocument();

    await type(dialog, 'Entities', 'ci.yml,');

    expect(within(dialog).getByText(/The entities you typed are not saved when replacing/)).toBeInTheDocument();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Replace it' }));

    await waitFor(() => expect(calls(invoke, 'memory_note_save')).toHaveLength(2));
    expect(calls(invoke, 'memory_note_save').at(-1)).toMatchObject({ replaceId: 3 });
  });
});

describe('the entities of a note that is edited', () => {
  const TITLE = 'Tauri window stayed blank after a hot reload on Wayland';

  it('shows the entities of the note as they are, with no field to change them', async () => {
    const invoke = fakeMemory();
    const dialog = await openEdit(4, TITLE);

    const list = within(dialog).getByRole('list', { name: 'Entities' });
    expect(within(list).getAllByRole('listitem').map((item) => item.textContent)).toEqual(['WEBKIT_DISABLE_DMABUF_RENDERER', 'src-tauri/tauri.conf.json']);
    expect(within(dialog).getByText('Agents set these. Harness Memory keeps them as they are when a note is edited.')).toBeInTheDocument();

    // Its links are shown too, and none of them opens another note from the editor.
    const links = within(dialog).getByRole('list', { name: 'Linked notes' });
    expect(within(links).getAllByRole('listitem').map((item) => item.textContent)).toEqual([expect.stringContaining('related to'), expect.stringContaining('caused by')]);
    expect(within(links).queryByRole('button')).toBeNull();
    expect(within(dialog).queryByRole('textbox', { name: 'Entities' })).toBeNull();
    expect(chipsOf(dialog, 'entity')).toEqual([]);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'memory_note_save')).toHaveLength(1));
  });

  it('spells out characters an agent hid in an entity', async () => {
    fakeMemory(undefined, undefined, undefined, sampleNotes().map((note) => (note.id === 4 ? { ...note, entities: ['ab\u202ecd'] } : note)));
    const dialog = await openEdit(4, TITLE);

    const list = within(dialog).getByRole('list', { name: 'Entities' });
    expect(within(list).getByRole('listitem').textContent).toBe('ab⟨U+202E⟩cd');
  });

  it('says so when the note has no entities', async () => {
    fakeMemory(undefined, undefined, undefined, sampleNotes().map((note) => (note.id === 4 ? { ...note, entities: [] } : note)));
    const dialog = await openEdit(4, TITLE);

    expect(within(dialog).queryByRole('list', { name: 'Entities' })).toBeNull();
    expect(within(dialog).getByText('None')).toBeInTheDocument();
  });
});
