import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { toasts } from '$lib/stores/toasts.svelte';

import { fakeMemory, registerMemoryHooks } from './memory-test-support';
import { fill, openEdit, openEditor } from './note-editor-support';

registerMemoryHooks();

/** Makes the server answer every note save as kept on this computer: the remote cannot be reached, so the change waits there. */
function queueingSaves(invoke: ReturnType<typeof fakeMemory>): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation((async (name: CommandName, args: never) => {
    const answer = await real(name as never, args);

    return name === 'memory_note_save' ? { ...(answer as object), queued: true } : answer;
  }) as typeof ipc.invoke);
}

const toastTitles = () => toasts.items.map((toast) => toast.title);

describe('a note saved while the remote cannot be reached', () => {
  it('says it is kept on this computer and applied when the remote is back, instead of saying it was saved', async () => {
    queueingSaves(fakeMemory());
    const dialog = await openEditor();

    await fill(dialog, { title: 'Release checklist', body: 'Tag the release, then publish the notes.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(toastTitles()).toContain('Saved on this computer'));
    expect(toastTitles()).not.toContain('Note added');
    expect(toasts.items.find((toast) => toast.title === 'Saved on this computer')).toMatchObject({ description: expect.stringContaining('Release checklist'), tone: 'warning' });
  });

  it('says the same for an edit', async () => {
    queueingSaves(fakeMemory());
    const dialog = await openEdit(4, 'Tauri window stayed blank after a hot reload on Wayland');

    await fill(dialog, { body: 'A newer answer.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(toastTitles()).toContain('Saved on this computer'));
    expect(toastTitles()).not.toContain('Note saved');
    expect(screen.queryByRole('dialog', { name: 'Edit note' })).toBeNull();
  });

  it('says a plain save is a save', async () => {
    fakeMemory();
    const dialog = await openEditor();

    await fill(dialog, { title: 'Release checklist', body: 'Tag the release.' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(toastTitles()).toContain('Note added'));
    expect(toastTitles()).not.toContain('Saved on this computer');
  });
});
