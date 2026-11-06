import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { MemoryChatGuards } from '$lib/ipc/bindings/MemoryChatGuards';
import type { MemoryGuards } from '$lib/ipc/bindings/MemoryGuards';
import { sampleHeld } from '$lib/ipc/mock/memory-guards-fixtures';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { defaultGuards, NO_GLOBAL_GUARDS, withKind } from '$lib/stores/memory-chat';
import { memoryChat } from '$lib/stores/memoryChat.svelte';
import { memoryHeld } from '$lib/stores/memoryHeld.svelte';

import { PROJECT } from '../../../tests/chat-helpers';
import { startMockApp } from '../../../tests/mock-app';
import MemoryChatRow from './MemoryChatRow.svelte';

const KEY = '11111111-2222-4333-8444-555555555555';
/** Saving is turned on for every project, so what a chat says is about the chat itself. */
const ALL_ON: MemoryGuards = {
  notes: { paused: false, everyProject: true, onRepos: [], deadRepos: [], changedRepos: [] },
  scripts: { paused: false, everyProject: true, onRepos: [], deadRepos: [], changedRepos: [] },
  migrationPending: false,
};
const SAVING = defaultGuards({ notes: false, scripts: false });
const attached: string[] = [];

/** Renders what the chat says about saving, with these guards and nothing held. */
async function show(guards: MemoryChatGuards): Promise<void> {
  const session = await agentSessions.create('omp', PROJECT);
  attached.push(session!.key);
  memoryChat.attach(session!.key, { key: KEY, guards });
  render(MemoryChatRow, { agentKey: session!.key });
}

/** The lines the chat says about saving, without the saved and skipped ones. */
function lines(): string[] {
  return screen.queryAllByRole('status').map((line) => line.textContent!.replace(/\s+/g, ' ').trim());
}

const off = (guards: MemoryChatGuards, ...kinds: Array<'notes' | 'scripts'>) => kinds.reduce((next, kind) => withKind(next, kind, { on: false }), guards);
const ask = (guards: MemoryChatGuards, ...kinds: Array<'notes' | 'scripts'>) => kinds.reduce((next, kind) => withKind(next, kind, { ask: true }), guards);

beforeEach(async () => {
  await startMockApp(PROJECT);
  memoryChat.global = ALL_ON;
});

afterEach(async () => {
  for (const key of attached.splice(0)) {
    memoryChat.release(key);
    await agentSessions.stop(key);
  }

  memoryChat.global = null;
  memoryHeld.items = [];
  vi.restoreAllMocks();
});

describe('the line a chat says about what it saves', () => {
  it('says nothing while everything is saved as the agent writes it', async () => {
    await show(SAVING);

    expect(lines()).toEqual([]);
  });

  it('says nothing for a chat without a memory identity', async () => {
    const session = await agentSessions.create('omp', PROJECT);
    attached.push(session!.key);
    render(MemoryChatRow, { agentKey: session!.key });

    expect(lines()).toEqual([]);
  });

  it('says no notes come from the chat when notes are off and nothing is held', async () => {
    await show(off(SAVING, 'notes'));

    expect(lines()).toEqual(['No notes from this chat.']);
  });

  it('says no scripts come from the chat when scripts are off', async () => {
    await show(off(SAVING, 'scripts'));

    expect(lines()).toEqual(['No scripts from this chat.']);
  });

  it('says nothing is saved when both are off', async () => {
    await show(off(SAVING, 'notes', 'scripts'));

    expect(lines()).toEqual(['Nothing is saved from this chat. Agents still read notes and scripts.']);
  });

  it('says nothing in a project that was not turned on, which is how every project starts', async () => {
    memoryChat.global = NO_GLOBAL_GUARDS;
    await show(SAVING);
    await vi.waitFor(() => expect(memoryChat.rootOf(PROJECT)).toBe(PROJECT));

    expect(lines()).toEqual([]);
  });

  it('says only what the user did when the other kind is off by default', async () => {
    memoryChat.global = { ...NO_GLOBAL_GUARDS, notes: { paused: false, everyProject: false, onRepos: [PROJECT], deadRepos: [], changedRepos: [] } };
    await show(off(SAVING, 'notes'));

    await waitFor(() => expect(lines()).toEqual(['No notes from this chat.']));
  });

  it('says nothing about the other kind in a project that has only one kind turned on', async () => {
    memoryChat.global = { ...NO_GLOBAL_GUARDS, notes: { paused: false, everyProject: false, onRepos: [PROJECT], deadRepos: [], changedRepos: [] } };
    await show(ask(SAVING, 'notes'));

    await waitFor(() => expect(lines()).toEqual(['Notes wait for your review.']));
  });

  it('says scripts are paused everywhere when they are paused in Settings', async () => {
    memoryChat.global = { ...ALL_ON, scripts: { paused: true, everyProject: true, onRepos: [], deadRepos: [], changedRepos: [] } };
    await show(SAVING);

    expect(lines()).toEqual(['No scripts from this chat. Scripts are paused everywhere.']);
  });

  it('says paused with a way to resume, and nothing about the kinds', async () => {
    vi.stubEnv('VITE_TRANSPORT', 'mock');
    await show({ ...off(SAVING, 'notes'), paused: true });

    expect(lines()).toEqual(['Saving paused for this chat. Agents still read notes and scripts. Resume']);

    await fireEvent.click(screen.getByRole('button', { name: 'Resume' }));

    await waitFor(() => expect(memoryChat.get(attached[0])?.guards.paused).toBe(false));
  });

  it('says notes wait for review when Ask before saving is on for them', async () => {
    await show(ask(SAVING, 'notes'));

    expect(lines()).toEqual(['Notes from this chat wait for your review.']);
  });

  it('says scripts wait for review when Ask before saving is on for them', async () => {
    await show(ask(SAVING, 'scripts'));

    expect(lines()).toEqual(['Scripts from this chat wait for your review.']);
  });

  it('says both wait for review when Ask before saving is on for both', async () => {
    await show(ask(SAVING, 'notes', 'scripts'));

    expect(lines()).toEqual(['Notes and scripts from this chat wait for your review.']);
  });

  it('joins an off kind and a kind that waits into one line', async () => {
    await show(ask(off(SAVING, 'notes'), 'scripts'));

    expect(lines()).toEqual(['No notes from this chat. Scripts wait for your review.']);
  });

  it('does not ask about a kind that is off', async () => {
    await show(ask(off(SAVING, 'notes'), 'notes'));

    expect(lines()).toEqual(['No notes from this chat.']);
  });

  it('is one line above a waiting card, not a second one', async () => {
    memoryHeld.items = sampleHeld(KEY);
    await show(off(SAVING, 'notes'));

    expect(screen.getByRole('region', { name: 'Saves waiting for your review' })).toBeInTheDocument();
    expect(lines()).toEqual(['No notes from this chat.']);
  });
});

