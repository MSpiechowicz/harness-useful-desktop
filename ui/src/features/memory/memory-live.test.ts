import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { CommandName } from '$lib/ipc/commands';
import { sampleNotes } from '$lib/ipc/mock/memory-notes-fixtures';
import { sampleScripts } from '$lib/ipc/mock/memory-scripts-fixtures';
import { IpcFailure } from '$lib/ipc/transport';
import { scriptList } from '$lib/stores/memoryScripts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { fakeMemory, noteRow, registerMemoryHooks, rowButton, showMemory, showNotes } from './memory-test-support';

registerMemoryHooks();

/** The window comes back to the front: an open view reads again, like at every tick of its poll. */
const scriptListSha = () => scriptList.data?.scripts.find((script) => script.name === 'ci-status')?.sha256;

const tick = () => window.dispatchEvent(new Event('focus'));

describe('an open script', () => {
  it('shows what an agent changed in it while its panel was open, without clicking another script and back', async () => {
    const scripts = sampleScripts();
    const open = scripts.find((script) => script.name === 'ci-status')!;
    fakeMemory(undefined, scripts);
    await showMemory();
    await fireEvent.click(rowButton('ci-status') as HTMLElement);
    const panel = await screen.findByRole('region', { name: 'ci-status' });
    await within(panel).findByText(/import json/);

    open.description = 'Shows whether the last five runs passed.';
    open.timeoutSecs = 120;
    open.runCount = 9;
    tick();

    expect(await within(panel).findByText('Shows whether the last five runs passed.')).toBeInTheDocument();
    expect(within(panel).getByText('120 s')).toBeInTheDocument();
    expect(within(panel).getByText('9')).toBeInTheDocument();
  });

  it('shows the new code when an agent rewrote it', async () => {
    const scripts = sampleScripts();
    const open = scripts.find((script) => script.name === 'ci-status')!;
    fakeMemory(undefined, scripts);
    await showMemory();
    await fireEvent.click(rowButton('ci-status') as HTMLElement);
    const panel = await screen.findByRole('region', { name: 'ci-status' });
    await within(panel).findByText(/import json/);

    open.body = 'print("rewritten")\n';
    open.sha256 = 'b'.repeat(64);
    tick();

    expect(await within(panel).findByText(/rewritten/)).toBeInTheDocument();
    expect(within(panel).queryByText(/import json/)).toBeNull();
  });

  it('shows the new code even when the first read of it failed', async () => {
    const scripts = sampleScripts();
    const open = scripts.find((script) => script.name === 'ci-status')!;
    const invoke = fakeMemory(undefined, scripts);
    const answer = invoke.getMockImplementation() as (name: CommandName, args: unknown) => Promise<unknown>;
    let failing = false;
    invoke.mockImplementation((async (name: CommandName, args: unknown) => {
      if (failing && name === 'memory_script_get') {
        throw IpcFailure.from({ code: 'internal', message: 'the Memory CLI was busy' });
      }

      return answer(name, args);
    }) as never);
    await showMemory();
    await fireEvent.click(rowButton('ci-status') as HTMLElement);
    const panel = await screen.findByRole('region', { name: 'ci-status' });
    await within(panel).findByText(/import json/);

    failing = true;
    open.body = 'print("rewritten")\n';
    open.sha256 = 'b'.repeat(64);
    tick();
    await waitFor(() => expect(scriptListSha()).toBe(open.sha256));

    failing = false;
    tick();

    expect(await within(panel).findByText(/rewritten/)).toBeInTheDocument();
  });

  it('shows what changed in a script whose row is on another page than the list', async () => {
    const scripts = sampleScripts();
    const open = scripts.find((script) => script.name === 'ci-status')!;
    fakeMemory(undefined, scripts);
    await showMemory();
    await fireEvent.click(rowButton('ci-status') as HTMLElement);
    const panel = await screen.findByRole('region', { name: 'ci-status' });
    await within(panel).findByText(/import json/);

    // A search that leaves the open script out of the page: its row is gone, the panel stays.
    await fireEvent.input(screen.getByRole('searchbox'), { target: { value: 'open-prs' } });
    await waitFor(() => expect(rowButton('ci-status')).toBeNull());
    expect(screen.getByRole('region', { name: 'ci-status' })).toBeInTheDocument();

    open.description = 'Changed while off the page.';
    open.runCount = 11;
    tick();

    expect(await within(panel).findByText('Changed while off the page.')).toBeInTheDocument();
    expect(within(panel).getByText('11')).toBeInTheDocument();
  });
});

describe('an open script that an agent deleted', () => {
  it('closes its panel instead of showing a script that is gone', async () => {
    const scripts = sampleScripts();
    fakeMemory(undefined, scripts);
    await showMemory();
    await fireEvent.click(rowButton('ci-status') as HTMLElement);
    const panel = await screen.findByRole('region', { name: 'ci-status' });
    await within(panel).findByText(/import json/);

    scripts.splice(scripts.findIndex((script) => script.name === 'ci-status'), 1);
    tick();

    await waitFor(() => expect(viewState.scriptId).toBeNull());
    await waitFor(() => expect(screen.queryByRole('region', { name: 'ci-status' })).toBeNull());
  });
});

describe('an open note that an agent deleted', () => {
  it('closes its panel instead of showing a note that is gone', async () => {
    const notes = sampleNotes();
    fakeMemory(undefined, undefined, undefined, notes);
    await showNotes();
    const open = notes[0];
    await fireEvent.click(noteRow(open.id) as HTMLElement);
    await screen.findByRole('region', { name: open.title });

    notes.splice(0, 1);
    tick();

    await waitFor(() => expect(viewState.noteId).toBeNull());
  });
});

describe('an open note', () => {
  it('shows what an agent changed in it while its panel was open', async () => {
    const notes = sampleNotes();
    fakeMemory(undefined, undefined, undefined, notes);
    await showNotes();
    const open = notes[0];
    await fireEvent.click(noteRow(open.id) as HTMLElement);
    const panel = await screen.findByRole('region', { name: open.title });

    open.body = 'An agent rewrote this note.';
    open.title = 'Renamed by an agent';
    tick();

    expect(await screen.findByRole('region', { name: 'Renamed by an agent' })).toBeInTheDocument();
    expect(await within(panel).findByText(/An agent rewrote this note\./)).toBeInTheDocument();
    expect(viewState.noteId).toBe(open.id);
  });
});
