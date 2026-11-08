import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { HeldItem } from '$lib/ipc/bindings/HeldItem';
import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import type { MemorySavePlace } from '$lib/ipc/bindings/MemorySavePlace';
import { ipc } from '$lib/ipc/client';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { HELD_NOTE_SAVE_ID, HELD_NOTE_UPDATE_ID, HELD_SCRIPT_SAVE_ID, sampleHeld } from '$lib/ipc/mock/memory-guards-fixtures';
import { defaultGuards } from '$lib/stores/memory-chat';
import { memoryChat } from '$lib/stores/memoryChat.svelte';
import { memoryHeld } from '$lib/stores/memoryHeld.svelte';
import { noteList } from '$lib/stores/memoryNotes.svelte';
import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
import { scriptList } from '$lib/stores/memoryScripts.svelte';
import { projectScopes } from '$lib/stores/project-scopes.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import MemoryChatRow from './MemoryChatRow.svelte';

const AGENT = 'agent-held-place';
const KEY = '11111111-2222-4333-8444-555555555555';
const NOTE = 'Bindings come from the export test';
const SCRIPT = 'check-bindings';
const UPDATE = 'WebKitGTK caps the page at 60 fps';
const HOME = '/home/dev/projects/harness-desktop';

interface Setup {
  remoteLocalSaves?: MemorySavePlace;
  heldSavePlaceSupported?: boolean;
  routing?: MemoryNotesRouting;
  globalNotes?: MemoryNotesRouting;
  offline?: boolean;
}

/** What Harness Memory answers a held save with, by held id. */
type Answers = Record<string, { kind: 'note'; id: number; queued: false; place: MemorySavePlace | null } | { kind: 'script'; name: string; place: MemorySavePlace | null }>;

let remote: MemoryRemote;
let saves: { heldId: string; place?: MemorySavePlace; edited: unknown }[];
let answers: Answers;
let refuse: string | null;

/** The held saves of the chat on screen, with the settings of Harness Memory kept as asked. */
async function show(setup: Setup = {}, items: HeldItem[] = sampleHeld(KEY)): Promise<void> {
  const { remoteLocalSaves = 'remote', heldSavePlaceSupported = true, routing = 'remoteLocal', globalNotes = routing, offline = false } = setup;

  remote = { ...(await ipc.invoke('memory_remote_get', undefined)), repositoryNotes: routing, globalNotes, scripts: routing, remoteLocalSaves, heldSavePlaceSupported } as MemoryRemote;
  memoryRemote.value = remote;
  noteList.data = { offline } as typeof noteList.data;
  scriptList.data = { offline } as typeof scriptList.data;
  projectScopes.data = [{ path: HOME, name: 'harness-desktop', root: HOME, key: 'path:' + HOME }];

  memoryChat.attach(AGENT, { key: KEY, guards: defaultGuards({ notes: true, scripts: true }) });
  memoryHeld.items = items.map((held) => ({ ...held, projectPath: HOME, repoRoot: HOME }));
  render(MemoryChatRow, { agentKey: AGENT });
}

const article = (name: string): HTMLElement => screen.getByRole('heading', { name }).closest('article') as HTMLElement;
const group = (scope: HTMLElement): HTMLElement | null => within(scope).queryByRole('radiogroup', { name: 'Save to' });
const radio = (scope: HTMLElement, name: 'Remote' | 'This computer'): HTMLElement => within(group(scope) as HTMLElement).getByRole('radio', { name });
const saveOf = (name: string) => within(article(name)).getByRole('button', { name: 'Save' });

beforeEach(() => {
  saves = [];
  answers = {};
  refuse = null;
  toasts.items = [];

  const real = ipc.invoke.bind(ipc);

  vi.spyOn(ipc, 'invoke').mockImplementation((async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
    if (name === 'memory_held_save') {
      const { heldId, place, edited } = args as CommandArgs<'memory_held_save'> & { place?: MemorySavePlace };
      saves.push({ heldId, place, edited });

      if (refuse !== null) {
        throw Object.assign(new Error(refuse), { code: 'process' });
      }

      return (answers[heldId] ?? { kind: 'note', id: 1, queued: false, place: null }) as CommandResult<K>;
    }

    if (name === 'memory_remote_get') {
      return remote as CommandResult<K>;
    }

    return real(name, args);
  }) as typeof ipc.invoke);
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  memoryChat.release(AGENT);
  memoryHeld.items = [];
  memoryHeld.failures = {};
  projectScopes.data = null;
  noteList.data = null;
  scriptList.data = null;
});

describe('the Save to field of a card of a held save', () => {
  it('shows on a new note and a new script of a scope kept in Remote + this computer, and never on a change to a saved note', async () => {
    await show();

    expect(group(article(NOTE))).not.toBeNull();
    expect(group(article(SCRIPT))).not.toBeNull();
    expect(group(article(UPDATE))).toBeNull();
  });

  it.each<MemoryNotesRouting>(['remote', 'local'])('is not there where the scope is kept in %s only', async (routing) => {
    await show({ routing });

    expect(group(article(NOTE))).toBeNull();
    expect(group(article(SCRIPT))).toBeNull();
  });

  it('is not there while the installed Harness Memory cannot take a place for a held save', async () => {
    await show({ heldSavePlaceSupported: false });

    expect(group(article(NOTE))).toBeNull();
    expect(group(article(SCRIPT))).toBeNull();

    await fireEvent.click(saveOf(NOTE));

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0]).not.toHaveProperty('place', expect.anything());
  });

  it('follows the scope of the note: a global note is judged by the routing of the global notes', async () => {
    const items = sampleHeld(KEY).map((held) => (held.heldId === HELD_NOTE_SAVE_ID ? { ...held, arguments: { ...(held.arguments as object), scope: 'global' } } : held)) as HeldItem[];
    await show({ routing: 'remoteLocal', globalNotes: 'remote' }, items);

    expect(group(article(NOTE))).toBeNull();
    expect(group(article(SCRIPT))).not.toBeNull();
  });

  it.each<MemorySavePlace>(['remote', 'local'])('starts on %s when that is the setting of Harness Memory', async (start) => {
    await show({ remoteLocalSaves: start });

    for (const name of [NOTE, SCRIPT]) {
      expect(radio(article(name), 'Remote')).toHaveAttribute('aria-checked', String(start === 'remote'));
      expect(radio(article(name), 'This computer')).toHaveAttribute('aria-checked', String(start === 'local'));
    }
  });

  it('sends the place with Save, and says where the note went when the answer tells', async () => {
    answers[HELD_NOTE_SAVE_ID] = { kind: 'note', id: 9, queued: false, place: 'local' };
    await show();

    await fireEvent.click(radio(article(NOTE), 'This computer'));
    await fireEvent.click(saveOf(NOTE));

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0]).toMatchObject({ heldId: HELD_NOTE_SAVE_ID, place: 'local' });
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Note saved on this computer'));
  });

  it('says where a script went when the answer tells, and just that it was saved when it does not', async () => {
    answers[HELD_SCRIPT_SAVE_ID] = { kind: 'script', name: SCRIPT, place: 'remote' };
    await show();

    await fireEvent.click(saveOf(SCRIPT));
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Script saved to the remote'));

    await fireEvent.click(saveOf(NOTE));
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Note saved'));
  });

  it('sends no place for a change to a saved note', async () => {
    await show();

    await fireEvent.click(saveOf(UPDATE));

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0].heldId).toBe(HELD_NOTE_UPDATE_ID);
    expect(saves[0].place).toBeUndefined();
  });

  it('sends no place where the field does not show', async () => {
    await show({ routing: 'remote' });

    await fireEvent.click(saveOf(NOTE));

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0].place).toBeUndefined();
  });

  it('saves each item of Save all with the choice of its own card', async () => {
    await show({ remoteLocalSaves: 'remote' });

    await fireEvent.click(radio(article(SCRIPT), 'This computer'));
    await fireEvent.click(screen.getByRole('button', { name: 'Save all' }));

    await waitFor(() => expect(saves).toHaveLength(4));
    const placeOf = (id: string) => saves.find((save) => save.heldId === id)?.place;

    expect(placeOf(HELD_SCRIPT_SAVE_ID)).toBe('local');
    expect(placeOf(HELD_NOTE_SAVE_ID)).toBeUndefined();
    expect(placeOf(HELD_NOTE_UPDATE_ID)).toBeUndefined();
  });

  it('keeps the item and shows the reason of the server when the save is refused', async () => {
    refuse = 'The remote cannot be reached';
    await show();

    await fireEvent.click(saveOf(NOTE));

    expect(await within(article(NOTE)).findByRole('alert')).toHaveTextContent('The remote cannot be reached');
    expect(group(article(NOTE))).not.toBeNull();
  });

  it('keeps This computer chosen and Remote disabled with the reason while the remote cannot be reached', async () => {
    await show({ offline: true });

    expect(radio(article(NOTE), 'Remote')).toBeDisabled();
    expect(radio(article(NOTE), 'This computer')).toHaveAttribute('aria-checked', 'true');
    expect(within(article(NOTE)).getByText('The remote cannot be reached now, so this is saved on this computer for now.')).toBeInTheDocument();

    await fireEvent.click(saveOf(NOTE));

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0].place).toBeUndefined();
  });
});

describe('the Save to field in the Edit dialog of a held save', () => {
  async function openEditor(name: string, title: string): Promise<HTMLElement> {
    await fireEvent.click(within(article(name)).getByRole('button', { name: 'Edit' }));

    return screen.findByRole('dialog', { name: title });
  }

  it('shows the same value as the card, and a change in either is seen in both', async () => {
    await show();

    await fireEvent.click(radio(article(NOTE), 'This computer'));
    const dialog = await openEditor(NOTE, 'Edit the note before saving');

    expect(radio(dialog, 'This computer')).toHaveAttribute('aria-checked', 'true');

    await fireEvent.click(radio(dialog, 'Remote'));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());

    expect(radio(article(NOTE), 'Remote')).toHaveAttribute('aria-checked', 'true');
  });

  it('sends the place of the card with the edited save, and for a script as well', async () => {
    await show();

    const dialog = await openEditor(SCRIPT, 'Edit the script before saving');
    await fireEvent.click(radio(dialog, 'This computer'));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0]).toMatchObject({ heldId: HELD_SCRIPT_SAVE_ID, place: 'local' });
    expect(saves[0].edited).toMatchObject({ name: SCRIPT });
  });

  it('follows the scope the editor changes the note to', async () => {
    await show({ routing: 'remoteLocal', globalNotes: 'remote' });
    const dialog = await openEditor(NOTE, 'Edit the note before saving');

    expect(group(dialog)).not.toBeNull();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Applies to' }));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: 'Global' }));

    await waitFor(() => expect(group(dialog)).toBeNull());

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(saves).toHaveLength(1));
    expect(saves[0].place).toBeUndefined();
    expect(saves[0].edited).toMatchObject({ scope: 'global' });
  });

  it('is not in the dialog of a change to a saved note', async () => {
    await show();
    const dialog = await openEditor(UPDATE, 'Edit the note before saving');

    expect(group(dialog)).toBeNull();
  });
});
