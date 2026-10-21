import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, type MockInstance, vi } from 'vitest';

import type { HeldItem } from '$lib/ipc/bindings/HeldItem';
import { HELD_NOTE_APPEND_ID, HELD_NOTE_SAVE_ID, HELD_NOTE_UPDATE_ID, HELD_SCRIPT_SAVE_ID, sampleHeld } from '$lib/ipc/mock/memory-guards-fixtures';
import { defaultGuards } from '$lib/stores/memory-chat';
import { memoryChat } from '$lib/stores/memoryChat.svelte';
import { memoryHeld } from '$lib/stores/memoryHeld.svelte';

import MemoryChatRow from './MemoryChatRow.svelte';

const AGENT = 'agent-waiting';
const KEY = '11111111-2222-4333-8444-555555555555';

function show(items: HeldItem[] = sampleHeld(KEY)): void {
  memoryChat.attach(AGENT, { key: KEY, guards: defaultGuards({ notes: true, scripts: true }) });
  memoryHeld.items = items;
  render(MemoryChatRow, { agentKey: AGENT });
}

/** The article of the waiting item with this title or name. */
function item(name: string): HTMLElement {
  return screen.getByRole('heading', { name }).closest('article') as HTMLElement;
}

let save: MockInstance<typeof memoryHeld.save>;
let skip: MockInstance<typeof memoryHeld.skip>;

beforeEach(() => {
  save = vi.spyOn(memoryHeld, 'save').mockResolvedValue(true);
  skip = vi.spyOn(memoryHeld, 'skip').mockResolvedValue(true);
});

afterEach(() => {
  vi.restoreAllMocks();
  memoryChat.release(AGENT);
  memoryHeld.items = [];
  memoryHeld.failures = {};
});

describe('the card of saves waiting for review', () => {
  it('says what the agent wants to save and that nothing is saved until the user presses Save', () => {
    show();

    expect(screen.getByRole('heading', { name: 'The agent wants to save 3 notes and 1 script' })).toBeInTheDocument();
    expect(screen.getByText('Nothing is saved until you press Save')).toBeInTheDocument();
  });

  it('shows a new note in full: kind, scope, body, tags and what it is about', () => {
    show();
    const note = item('Bindings come from the export test');

    expect(within(note).getByText('Location')).toBeInTheDocument();
    expect(within(note).getByText('This project')).toBeInTheDocument();
    expect(within(note).getByText('New note')).toBeInTheDocument();
    expect(within(note).getByText(/ts-rs writes ui\/src\/lib\/ipc\/bindings when/)).toBeInTheDocument();
    expect(within(note).getByText('#ts-rs')).toBeInTheDocument();
    expect(within(note).getByText('Tags')).toBeInTheDocument();
    expect(within(note).getByText('About')).toBeInTheDocument();
  });

  it('shows a change to a saved note with a line diff against the note as saved now', () => {
    show();
    const note = item('WebKitGTK caps the page at 60 fps');

    expect(within(note).getByText('Updates note #3')).toBeInTheDocument();
    expect(within(note).getByText('Was: WebKitGTK falls back to 60 fps')).toBeInTheDocument();

    const removed = note.querySelectorAll('[data-diff="remove"]');
    const added = note.querySelectorAll('[data-diff="add"]');
    expect(removed).toHaveLength(1);
    expect(removed[0]).toHaveTextContent('WebKitGTK falls back to a 60 fps timer here.');
    expect(added).toHaveLength(1);
    expect(added[0]).toHaveTextContent('measure frame pacing in WebKitGTK, not in Chrome.');
  });

  it('shows the body a held update with append would leave, in the body and in the diff', () => {
    const [script, update, created] = sampleHeld(KEY);
    const appended = { ...update, arguments: { id: 3, title: 'WebKitGTK falls back to 60 fps', append: 'Measure it in WebKitGTK, not in Chrome.' } } as HeldItem;
    show([script, appended, created]);
    const note = item('WebKitGTK falls back to 60 fps');

    expect(note.querySelector('.md-html')).toHaveTextContent('WebKitGTK falls back to a 60 fps timer here. Measure it in WebKitGTK, not in Chrome.');

    const added = note.querySelectorAll('[data-diff="add"]');
    expect(added).toHaveLength(1);
    expect(added[0]).toHaveTextContent('Measure it in WebKitGTK, not in Chrome.');
    expect(note.querySelectorAll('[data-diff="remove"]')).toHaveLength(0);
  });

  it('shows an update that only appends under the saved title, with the appended text added', () => {
    show();
    const note = item('Source files stay under 500 lines');

    expect(within(note).getByText('Updates note #7')).toBeInTheDocument();
    expect(note.querySelector('.md-html')).toHaveTextContent('Split it into modules. Run bun run guard before a commit. It checks file sizes too.');
    expect(note.querySelectorAll('[data-diff="add"]')).toHaveLength(1);
    expect(note.querySelectorAll('[data-diff="remove"]')).toHaveLength(0);
  });

  it('shows a script in full, with what it needs before it runs', async () => {
    show();
    const script = item('check-bindings');

    expect(within(script).getByText('Script')).toBeInTheDocument();
    expect(within(script).getByText('Bash')).toBeInTheDocument();
    expect(within(script).getByText('New script')).toBeInTheDocument();
    expect(within(script).getByText('Regenerate the TS bindings and fail when they changed.')).toBeInTheDocument();
    expect(await within(script).findByText(/git diff/)).toBeInTheDocument();
    expect(within(script).getByText('Only after you trust it. Saving does not run it.')).toBeInTheDocument();
    expect(within(script).getByText('Times out after 120s')).toBeInTheDocument();
  });

  it('folds a long note and opens it with Show all', async () => {
    const long = sampleHeld(KEY).slice(2).map((held) => ({ ...held, arguments: { ...(held.arguments as object), body: Array.from({ length: 30 }, (_, line) => `Line ${line + 1} of the note`).join('\n\n') } }) as HeldItem);
    show(long);
    const note = item('Bindings come from the export test');

    expect(note.querySelector('[data-folded]')).not.toBeNull();

    await fireEvent.click(within(note).getByRole('button', { name: 'Show all' }));

    expect(note.querySelector('[data-folded]')).toBeNull();
    expect(within(note).getByText('Line 30 of the note')).toBeInTheDocument();
  });

  it('saves one item as the agent sent it', async () => {
    show();

    await fireEvent.click(within(item('Bindings come from the export test')).getByRole('button', { name: 'Save' }));

    expect(save).toHaveBeenCalledTimes(1);
    expect(save.mock.calls[0][0]).toMatchObject({ heldId: HELD_NOTE_SAVE_ID });
    expect(save.mock.calls[0][1]).toBeUndefined();
  });

  it('skips one item without saving anything', async () => {
    show();

    await fireEvent.click(within(item('WebKitGTK caps the page at 60 fps')).getByRole('button', { name: 'Skip' }));

    expect(skip).toHaveBeenCalledTimes(1);
    expect(skip.mock.calls[0][0]).toMatchObject({ heldId: HELD_NOTE_UPDATE_ID });
    expect(save).not.toHaveBeenCalled();
  });

  it('sends the edited save for the version the editor was opened on', async () => {
    const items = sampleHeld(KEY);
    const opened = items.find((held) => held.heldId === HELD_SCRIPT_SAVE_ID)!.revision;
    show(items);

    await fireEvent.click(within(item('check-bindings')).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit the script before saving' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0][2]).toBe(opened);
  });

  it('closes the editor and says the agent changed it when the save is held again meanwhile', async () => {
    const items = sampleHeld(KEY);
    show(items);

    await fireEvent.click(within(item('check-bindings')).getByRole('button', { name: 'Edit' }));
    await screen.findByRole('dialog', { name: 'Edit the script before saving' });

    memoryHeld.items = items.map((held) => (held.heldId === HELD_SCRIPT_SAVE_ID ? { ...held, revision: 'f'.repeat(64) } : held));

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(within(item('check-bindings')).getByRole('alert')).toHaveTextContent('The agent changed it. Review it again.');
    expect(save).not.toHaveBeenCalled();
  });

  it('saves everything that waits, one after the other', async () => {
    show();

    await fireEvent.click(screen.getByRole('button', { name: 'Save all' }));

    await waitFor(() => expect(save).toHaveBeenCalledTimes(4));
    expect(save.mock.calls.map(([held]) => held.heldId)).toEqual([HELD_SCRIPT_SAVE_ID, HELD_NOTE_UPDATE_ID, HELD_NOTE_SAVE_ID, HELD_NOTE_APPEND_ID]);
  });

  it('saves what the user changed in the editor as the edited save, and closes the editor', async () => {
    show();

    await fireEvent.click(within(item('check-bindings')).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit the script before saving' });
    const description = within(dialog).getByLabelText('Description') as HTMLInputElement;
    expect(description.value).toBe('Regenerate the TS bindings and fail when they changed.');

    await fireEvent.input(description, { target: { value: 'Regenerate the bindings.' } });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0][0]).toMatchObject({ heldId: HELD_SCRIPT_SAVE_ID });
    expect(save.mock.calls[0][1]).toMatchObject({ name: 'check-bindings', description: 'Regenerate the bindings.', language: 'bash', timeoutSecs: 120, scope: 'repo' });
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('edits a change to a saved note with only the fields that change takes', async () => {
    show();

    await fireEvent.click(within(item('WebKitGTK caps the page at 60 fps')).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit the note before saving' });
    await fireEvent.input(within(dialog).getByLabelText('Title'), { target: { value: 'WebKitGTK caps pages at 60 fps' } });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save.mock.calls[0][1]).toEqual({ title: 'WebKitGTK caps pages at 60 fps', body: expect.stringContaining('60 fps timer'), kind: 'problem', tags: ['webkit', 'perf'] });
  });

  it('keeps the editor open and says why when the save is refused', async () => {
    show();
    save.mockImplementation(async (held) => {
      memoryHeld.failures = { [held.heldId]: 'The note was deleted in the meantime, so this change cannot be saved. Skip it.' };

      return false;
    });

    await fireEvent.click(within(item('WebKitGTK caps the page at 60 fps')).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit the note before saving' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    expect(await within(dialog).findByText(/deleted in the meantime/)).toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: 'Edit the note before saving' })).toBeInTheDocument();
  });

  it('shows why an item could not be saved inside that item', () => {
    show();
    memoryHeld.failures = { [HELD_NOTE_UPDATE_ID]: 'The note was deleted in the meantime, so this change cannot be saved. Skip it.' };

    return waitFor(() => expect(within(item('WebKitGTK caps the page at 60 fps')).getByRole('alert')).toHaveTextContent('deleted in the meantime'));
  });

  it('has no card while nothing waits', () => {
    show([]);

    expect(screen.queryByRole('region', { name: 'Saves waiting for your review' })).toBeNull();
  });
});

describe('what the chat says when saving is paused', () => {
  it('says so under the conversation and resumes from there', async () => {
    // The guards go to the mock backend, which takes them as Harness Memory does.
    vi.stubEnv('VITE_TRANSPORT', 'mock');
    show([]);
    await memoryChat.setPaused(AGENT, true);

    expect(await screen.findByText('Saving paused for this chat. Agents still read notes and scripts.')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Resume' }));

    await waitFor(() => expect(memoryChat.get(AGENT)?.guards.paused).toBe(false));
  });
});
