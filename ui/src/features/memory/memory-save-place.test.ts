import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import type { MemorySavePlace } from '$lib/ipc/bindings/MemorySavePlace';
import type { ScriptSaved } from '$lib/ipc/bindings/ScriptSaved';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { IpcFailure } from '$lib/ipc/transport';
import { noteList } from '$lib/stores/memoryNotes.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { scriptList } from '$lib/stores/memoryScripts.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { appliesTo, calls, fakeMemory, noteRow, pickTarget, registerMemoryHooks, rowButton, showMemory, showNotes, withTeamDatabase } from './memory-test-support';
import { outageOf, withRemote } from './memory-sync-support';
import { fill as fillNote, openEditor as openNoteEditor } from './note-editor-support';
import NoteEditor from './NoteEditor.svelte';
import { BODY, fill as fillScript, openEditor as openScriptEditor } from './script-editor-support';
import ScriptEditor from './ScriptEditor.svelte';

registerMemoryHooks();

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off' });
});

afterEach(() => {
  cleanup();
});

/** The two editors, which keep the Save to field in the same place and have to behave alike. */
interface Editor {
  name: 'note' | 'script';
  command: 'memory_note_save' | 'memory_script_save';
  addLabel: string;
  failedToast: string;
  /** The settings with the scope the editor opens on kept in `routing`, and the other scopes kept on the remote only. */
  routing: (routing: MemoryNotesRouting) => Pick<MemoryRemote, 'repositoryNotes' | 'globalNotes'> & Partial<Pick<MemoryRemote, 'scripts'>>;
  open: () => Promise<HTMLElement>;
  openEdit: () => Promise<HTMLElement>;
  complete: (dialog: HTMLElement, taken?: boolean) => Promise<void>;
  /** What the toast of a new entry says per place. */
  created: Record<MemorySavePlace, string>;
}

const EDITORS: Editor[] = [
  {
    name: 'note',
    command: 'memory_note_save',
    addLabel: 'Add note',
    failedToast: 'The note was not saved',
    routing: (routing) => ({ repositoryNotes: routing, globalNotes: routing }),
    open: openNoteEditor,
    openEdit: async () => {
      await showNotes();
      await fireEvent.click(noteRow(4) as HTMLElement);
      const panel = await screen.findByRole('region', { name: 'Tauri window stayed blank after a hot reload on Wayland' });
      await within(panel).findByText(/WebKitGTK renders into a DMA buffer/);
      await fireEvent.click(within(panel).getByRole('button', { name: 'Edit' }));

      return screen.findByRole('dialog', { name: 'Edit note' });
    },
    complete: (dialog, taken = false) => fillNote(dialog, { title: taken ? 'Notes are read through the Memory CLI, never its database' : 'Release checklist', body: 'Tag the release.' }),
    created: { remote: 'Note added to the remote', local: 'Note added on this computer' },
  },
  {
    name: 'script',
    command: 'memory_script_save',
    addLabel: 'Add script',
    failedToast: 'The script was not saved',
    routing: (routing) => ({ repositoryNotes: 'local', globalNotes: 'local', scripts: routing }),
    open: openScriptEditor,
    openEdit: async () => {
      await showMemory();
      await fireEvent.click(rowButton('deploy-check') as HTMLElement);
      const panel = await screen.findByRole('region', { name: 'deploy-check' });
      await waitFor(() => expect(within(panel).getByRole('button', { name: 'Edit' })).toBeEnabled());
      await fireEvent.click(within(panel).getByRole('button', { name: 'Edit' }));

      return screen.findByRole('dialog', { name: 'Edit script' });
    },
    complete: (dialog, taken = false) => fillScript(dialog, { name: taken ? 'ci-status' : 'list-todos', description: 'Lists the todos.', body: BODY }),
    created: { remote: 'Script added to the remote', local: 'Script added on this computer' },
  },
];

/** The fake Memory with its settings kept as asked: the scope kept in `routing`, and `patch` over the rest of what the server says. */
async function setUp(editor: Editor, routing: MemoryNotesRouting, patch: Partial<MemoryRemote> = {}, settings = editor.routing(routing)) {
  const invoke = fakeMemory();

  withTeamDatabase(invoke, settings);
  withRemote(invoke, patch);

  const outage = outageOf(invoke);

  await memoryRemote.load();

  return { invoke, outage };
}

const picker = (dialog: HTMLElement) => within(dialog).queryByRole('radiogroup', { name: 'Save to' });
const placeRadio = (dialog: HTMLElement, name: 'Remote' | 'This computer') => within(picker(dialog) as HTMLElement).getByRole('radio', { name });
const toastTitles = () => toasts.items.map((toast) => toast.title);

/** Cancels the dialog the way the user does, and opens the add form again. */
async function reopen(editor: Editor, dialog: HTMLElement): Promise<HTMLElement> {
  await fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  await fireEvent.click(await screen.findByRole('button', { name: editor.addLabel }));

  return screen.findByRole('dialog', { name: editor.name === 'note' ? 'Add a note' : 'Add a script' });
}

describe.each(EDITORS)('the Save to field of the $name editor', (editor) => {
  it('shows only where the scope is kept in Remote + this computer, after the Applies to field', async () => {
    await setUp(editor, 'remoteLocal');
    const dialog = await editor.open();

    expect(within(dialog).getByText('Save to')).toBeInTheDocument();
    expect(picker(dialog)).not.toBeNull();
    expect(appliesTo(dialog).compareDocumentPosition(picker(dialog) as HTMLElement) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it.each<MemoryNotesRouting>(['remote', 'local'])('is not there where the scope is kept in %s only', async (routing) => {
    await setUp(editor, routing);
    const dialog = await editor.open();

    expect(picker(dialog)).toBeNull();
    expect(within(dialog).queryByText('Save to')).toBeNull();
  });

  it('is not there while the installed Harness Memory cannot save to a place', async () => {
    const { invoke } = await setUp(editor, 'remoteLocal', { savePlaceSupported: false });
    const dialog = await editor.open();

    expect(picker(dialog)).toBeNull();

    await editor.complete(dialog);
    await fireEvent.click(within(dialog).getByRole('button', { name: editor.addLabel }));

    await waitFor(() => expect(calls(invoke, editor.command)).toHaveLength(1));
    expect(calls(invoke, editor.command)[0]).not.toHaveProperty('place');
  });

  it.each<MemorySavePlace>(['remote', 'local'])('starts on %s when that is the choice of Settings', async (start) => {
    await setUp(editor, 'remoteLocal', { remoteLocalSaves: start });
    const dialog = await editor.open();

    expect(placeRadio(dialog, 'Remote')).toHaveAttribute('aria-checked', String(start === 'remote'));
    expect(placeRadio(dialog, 'This computer')).toHaveAttribute('aria-checked', String(start === 'local'));
  });

  it.each<MemorySavePlace>(['remote', 'local'])('does not remember the last choice: the next editor starts on the Settings choice (%s) again', async (start) => {
    const other = start === 'remote' ? 'local' : 'remote';
    const { invoke } = await setUp(editor, 'remoteLocal', { remoteLocalSaves: start });
    const dialog = await editor.open();

    await fireEvent.click(placeRadio(dialog, other === 'remote' ? 'Remote' : 'This computer'));
    expect(placeRadio(dialog, other === 'remote' ? 'Remote' : 'This computer')).toHaveAttribute('aria-checked', 'true');

    const again = await reopen(editor, dialog);

    expect(placeRadio(again, start === 'remote' ? 'Remote' : 'This computer')).toHaveAttribute('aria-checked', 'true');
    expect(calls(invoke, 'memory_remote_set')).toHaveLength(0);
  });

  it.each<MemorySavePlace>(['remote', 'local'])('sends %s when the user picks it and says where it went', async (chosen) => {
    const { invoke } = await setUp(editor, 'remoteLocal', { remoteLocalSaves: chosen === 'remote' ? 'local' : 'remote' });
    const dialog = await editor.open();

    await fireEvent.click(placeRadio(dialog, chosen === 'remote' ? 'Remote' : 'This computer'));
    await editor.complete(dialog);
    await fireEvent.click(within(dialog).getByRole('button', { name: editor.addLabel }));

    await waitFor(() => expect(calls(invoke, editor.command)).toHaveLength(1));
    expect(calls(invoke, editor.command)[0]).toMatchObject({ place: chosen });
    await waitFor(() => expect(toastTitles()).toContain(editor.created[chosen]));
    expect(toastTitles()).not.toContain(editor.name === 'note' ? 'Note added' : 'Script added');
  });

  it('sends no place while the user keeps the starting choice, so the server follows the setting', async () => {
    const { invoke } = await setUp(editor, 'remoteLocal', { remoteLocalSaves: 'remote' });
    const dialog = await editor.open();

    await editor.complete(dialog);
    await fireEvent.click(within(dialog).getByRole('button', { name: editor.addLabel }));

    await waitFor(() => expect(calls(invoke, editor.command)).toHaveLength(1));
    expect(calls(invoke, editor.command)[0]).not.toHaveProperty('place');
  });

  it('sends no place where the field does not show', async () => {
    const { invoke } = await setUp(editor, 'remote');
    const dialog = await editor.open();

    await editor.complete(dialog);
    await fireEvent.click(within(dialog).getByRole('button', { name: editor.addLabel }));

    await waitFor(() => expect(calls(invoke, editor.command)).toHaveLength(1));
    expect(calls(invoke, editor.command)[0]).not.toHaveProperty('place');
    await waitFor(() => expect(toastTitles()).toContain(editor.name === 'note' ? 'Note added' : 'Script added'));
  });

  it('is not there when an entry is edited, and the edit names no place', async () => {
    const { invoke } = await setUp(editor, 'remoteLocal');
    const dialog = await editor.openEdit();

    expect(picker(dialog)).toBeNull();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, editor.command)).toHaveLength(1));
    expect(calls(invoke, editor.command)[0]).not.toHaveProperty('place');
  });

  it('names no place when the user replaces the entry that has the name', async () => {
    const { invoke } = await setUp(editor, 'remoteLocal');
    const dialog = await editor.open();

    await fireEvent.click(placeRadio(dialog, 'This computer'));
    await editor.complete(dialog, true);
    await fireEvent.click(within(dialog).getByRole('button', { name: editor.addLabel }));
    await fireEvent.click(await within(dialog).findByRole('button', { name: 'Replace it' }));

    await waitFor(() => expect(calls(invoke, editor.command)).toHaveLength(2));
    expect(calls(invoke, editor.command)[0]).toMatchObject({ place: 'local' });
    expect(calls(invoke, editor.command)[1]).not.toHaveProperty('place');
  });

  it('keeps This computer chosen and Remote disabled with the reason while the remote cannot be reached, without changing the Settings choice', async () => {
    const { invoke, outage } = await setUp(editor, 'remoteLocal', { remoteLocalSaves: 'remote' });
    outage.down = true;
    const dialog = await editor.open();

    expect(placeRadio(dialog, 'Remote')).toBeDisabled();
    expect(placeRadio(dialog, 'This computer')).toHaveAttribute('aria-checked', 'true');
    expect(within(dialog).getByText('The remote cannot be reached now, so this is saved on this computer for now.')).toBeInTheDocument();

    await editor.complete(dialog);
    await fireEvent.click(within(dialog).getByRole('button', { name: editor.addLabel }));

    await waitFor(() => expect(calls(invoke, editor.command)).toHaveLength(1));
    expect(calls(invoke, editor.command)[0]).not.toHaveProperty('place');
    expect(calls(invoke, 'memory_remote_set')).toHaveLength(0);
    expect(memoryRemote.value?.remoteLocalSaves).toBe('remote');
  });

  it('offers Remote again once the remote is back', async () => {
    const { outage } = await setUp(editor, 'remoteLocal');
    outage.down = true;
    const dialog = await editor.open();

    expect(placeRadio(dialog, 'Remote')).toBeDisabled();

    outage.down = false;
    await (editor.name === 'note' ? noteList.refresh() : scriptList.refresh());

    await waitFor(() => expect(placeRadio(dialog, 'Remote')).toBeEnabled());
    expect(placeRadio(dialog, 'Remote')).toHaveAttribute('aria-checked', 'true');
  });

  it('keeps the dialog and what was typed when a save to the remote fails, and says why with nothing secret in it', async () => {
    const { invoke } = await setUp(editor, 'remoteLocal');
    const dialog = await editor.open();
    const base = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<unknown>;

    invoke.mockImplementation((async (name: CommandName, args: unknown) => {
      if (name === editor.command) {
        throw IpcFailure.from({ code: 'process', message: 'the remote refused the save (postgres://team@db.example.com)' });
      }

      return base(name, args);
    }) as typeof ipc.invoke);

    await editor.complete(dialog);
    await fireEvent.click(within(dialog).getByRole('button', { name: editor.addLabel }));

    await waitFor(() => expect(toasts.items.map((toast) => [toast.title, toast.description])).toContainEqual([editor.failedToast, 'the remote refused the save (postgres://team@db.example.com)']));
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(within(dialog).getByLabelText(editor.name === 'note' ? 'Content' : 'Code')).toHaveValue(editor.name === 'note' ? 'Tag the release.' : BODY);
    expect(placeRadio(dialog, 'Remote')).toHaveAttribute('aria-checked', 'true');
    expect(within(dialog).getByRole('button', { name: editor.addLabel })).toBeEnabled();
  });

  it('is never in the editor that hands its draft to the caller', async () => {
    await setUp(editor, 'remoteLocal');

    if (editor.name === 'note') {
      render(NoteEditor, { open: true, note: null, links: [], commit: async () => null });
    } else {
      render(ScriptEditor, { open: true, script: null, commit: async () => null });
    }

    const review = await screen.findByRole('dialog');

    expect(within(review).getByText('Applies to')).toBeInTheDocument();
    expect(picker(review)).toBeNull();
  });
});

describe('the Save to field of the note editor and where the note applies', () => {
  const editor = EDITORS[0];

  it('follows the Applies to choice: project notes and global notes are routed on their own', async () => {
    await setUp(editor, 'remoteLocal', {}, { repositoryNotes: 'remoteLocal', globalNotes: 'remote' });
    const dialog = await editor.open();

    expect(picker(dialog)).not.toBeNull();

    await pickTarget(dialog, 'Global');
    expect(picker(dialog)).toBeNull();

    await pickTarget(dialog, /^demo/);
    expect(picker(dialog)).not.toBeNull();
  });

  it('shows for global notes alone kept in Remote + this computer, and names the place for them', async () => {
    const { invoke } = await setUp(editor, 'remoteLocal', {}, { repositoryNotes: 'remote', globalNotes: 'remoteLocal' });
    const dialog = await editor.open();

    expect(picker(dialog)).toBeNull();

    await pickTarget(dialog, 'Global');
    expect(picker(dialog)).not.toBeNull();
    expect(appliesTo(dialog)).toHaveTextContent('Global');

    await fireEvent.click(placeRadio(dialog, 'This computer'));
    await editor.complete(dialog);
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(calls(invoke, 'memory_note_save')).toHaveLength(1));
    expect(calls(invoke, 'memory_note_save')[0]).toMatchObject({ project: null, draft: { scope: 'global' }, place: 'local' });
  });

  it('keeps the choice made while the Applies to field changes', async () => {
    await setUp(editor, 'remoteLocal');
    const dialog = await editor.open();

    await fireEvent.click(placeRadio(dialog, 'This computer'));
    await pickTarget(dialog, 'Global');

    expect(placeRadio(dialog, 'This computer')).toHaveAttribute('aria-checked', 'true');
  });
});

describe('a script saved to the remote', () => {
  const editor = EDITORS[1];

  it('is saved and said so when the server answers without the script', async () => {
    const { invoke } = await setUp(editor, 'remoteLocal');
    const dialog = await editor.open();
    const base = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<unknown>;
    const answer: ScriptSaved = { outcome: 'created', id: 4242, script: null, place: 'remote' };

    invoke.mockImplementation((async (name: CommandName, args: unknown) => (name === 'memory_script_save' ? answer : base(name, args))) as typeof ipc.invoke);

    await editor.complete(dialog);
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add script' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Add a script' })).toBeNull());
    expect(toasts.items.map((toast) => [toast.title, toast.description, toast.tone])).toContainEqual(['Script added to the remote', 'list-todos', 'success']);
    expect(viewState.scriptId).toBe(4242);
  });
});

describe('a note saved with a server that does not say where it went', () => {
  const editor = EDITORS[0];

  it('says only that it was added', async () => {
    const { invoke } = await setUp(editor, 'remoteLocal');
    const dialog = await editor.open();
    const base = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<unknown>;

    invoke.mockImplementation((async (name: CommandName, args: unknown) => {
      const answer = await base(name, args);

      return name === 'memory_note_save' ? { ...(answer as object), place: null } : answer;
    }) as typeof ipc.invoke);

    await editor.complete(dialog);
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add note' }));

    await waitFor(() => expect(toastTitles()).toContain('Note added'));
  });
});
