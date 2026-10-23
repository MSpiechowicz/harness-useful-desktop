import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { CommandName } from '$lib/ipc/commands';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import NotesView from './NotesView.svelte';
import { calls, fakeMemory, noteRow, registerMemoryHooks, showNotes } from './memory-test-support';

registerMemoryHooks();

/** The notes of a Harness Memory that does not say what language they are in. */
const withoutLang = () => sampleNotes().map((note) => ({ ...note, lang: null }));

/** Makes the server's feature line `notes-lang` listed or not (`undefined`: the settings cannot be read at all). */
function withNotesLang(invoke: ReturnType<typeof fakeMemory>, listed: boolean | undefined): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation((async (name: CommandName, args: never) => {
    if (name !== 'memory_remote_get') {
      return real(name as never, args);
    }

    if (listed === undefined) {
      throw IpcFailure.from({ code: 'process', message: 'unreadable' });
    }

    return { ...(await real('memory_remote_get', undefined)), notesLangSupported: listed };
  }) as typeof ipc.invoke);
}

describe('notes the server flagged as not in English', () => {
  it('marks them in the list, and only them', async () => {
    fakeMemory();
    await showNotes();

    expect(within(noteRow(4) as HTMLElement).getByTitle('Not in English')).toBeInTheDocument();
    expect(within(noteRow(7) as HTMLElement).getByTitle('Not in English')).toBeInTheDocument();
    expect(within(noteRow(3) as HTMLElement).queryByTitle('Not in English')).toBeNull();
  });

  it('narrows the list to them with the Not in English filter, asking the server for it', async () => {
    const invoke = fakeMemory();
    await showNotes();

    const chip = await screen.findByRole('button', { name: 'Not in English' });
    expect(chip).toHaveAttribute('aria-pressed', 'false');
    expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ lang: undefined });

    await fireEvent.click(chip);

    await waitFor(() => expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ lang: 'other', offset: 0 }));
    await waitFor(() => expect(noteRow(3)).toBeNull());
    expect(noteRow(4)).not.toBeNull();
    expect(noteRow(7)).not.toBeNull();
    expect(screen.getByRole('button', { name: 'Not in English' })).toHaveAttribute('aria-pressed', 'true');

    await fireEvent.click(screen.getByRole('button', { name: 'Not in English' }));

    await waitFor(() => expect(noteRow(3)).not.toBeNull());
    expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ lang: undefined });
  });

  it('keeps the filter on screen when it leaves nothing, so it can be turned off', async () => {
    fakeMemory({ kind: 'available' }, undefined, undefined, sampleNotes().map((note) => ({ ...note, lang: 'en' })));
    viewState.notesNotEnglish = true;
    nav.go('notes');
    render(NotesView);

    const chip = await screen.findByRole('button', { name: 'Not in English' });
    expect(chip).toHaveAttribute('aria-pressed', 'true');
    await waitFor(() => expect(noteRow(3)).toBeNull());

    await fireEvent.click(chip);

    await waitFor(() => expect(noteRow(3)).not.toBeNull());
  });

  it('offers neither the filter nor the badge when the server does not list notes-lang', async () => {
    const invoke = fakeMemory({ kind: 'available' }, undefined, undefined, withoutLang());
    withNotesLang(invoke, false);
    await showNotes();
    await waitFor(() => expect(memoryRemote.value?.notesLangSupported).toBe(false));

    expect(noteRow(4)).not.toBeNull();
    expect(screen.queryByRole('button', { name: 'Not in English' })).toBeNull();
    expect(screen.queryByTitle('Not in English')).toBeNull();
  });

  it('follows the server\'s feature line over what a row happens to carry', async () => {
    const invoke = fakeMemory();
    withNotesLang(invoke, false);
    await showNotes();
    await waitFor(() => expect(memoryRemote.value?.notesLangSupported).toBe(false));

    expect(noteRow(4)).not.toBeNull();
    expect(screen.queryByRole('button', { name: 'Not in English' })).toBeNull();
    expect(screen.queryByTitle('Not in English')).toBeNull();
  });

  it('drops a chosen Not in English filter on a server that does not list notes-lang', async () => {
    const invoke = fakeMemory();
    withNotesLang(invoke, false);
    viewState.notesNotEnglish = true;
    await showNotes();
    await waitFor(() => expect(memoryRemote.value?.notesLangSupported).toBe(false));

    await waitFor(() => expect(viewState.notesNotEnglish).toBe(false));
    await waitFor(() => expect(calls(invoke, 'memory_notes_list').at(-1)).toMatchObject({ lang: undefined }));
    expect(screen.queryByRole('button', { name: 'Not in English' })).toBeNull();
    expect(noteRow(3)).not.toBeNull();
    expect(noteRow(4)).not.toBeNull();
  });

  it('reads the rows when the feature line could not be read', async () => {
    const invoke = fakeMemory();
    withNotesLang(invoke, undefined);
    await showNotes();

    expect(await screen.findByRole('button', { name: 'Not in English' })).toBeInTheDocument();
    expect(memoryRemote.value).toBeNull();
    expect(within(noteRow(4) as HTMLElement).getByTitle('Not in English')).toBeInTheDocument();
  });

  it('forgets the filter when the notes view is reset', () => {
    viewState.notesNotEnglish = true;

    viewState.reset('notes');

    expect(viewState.notesNotEnglish).toBe(false);
  });
});
