import { render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { MemoryChatGuards } from '$lib/ipc/bindings/MemoryChatGuards';
import type { MemoryGuards } from '$lib/ipc/bindings/MemoryGuards';
import { sampleHeld } from '$lib/ipc/mock/memory-guards-fixtures';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { defaultGuards, withKind } from '$lib/stores/memory-chat';
import { memoryChat } from '$lib/stores/memoryChat.svelte';
import { memoryHeld } from '$lib/stores/memoryHeld.svelte';

import { PROJECT } from '../../../tests/chat-helpers';
import { startMockApp } from '../../../tests/mock-app';
import { hasMemoryRow } from './memory-rows';
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

/** The lines the chat says about saving. */
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

describe('what a chat says about saving', () => {
  it('says nothing for a chat without a memory identity', async () => {
    const session = await agentSessions.create('omp', PROJECT);
    attached.push(session!.key);
    render(MemoryChatRow, { agentKey: session!.key });

    expect(lines()).toEqual([]);
    expect(hasMemoryRow(session!.key)).toBe(false);
  });

  it.each([
    ['everything is saved as the agent writes it', SAVING],
    ['notes are off', off(SAVING, 'notes')],
    ['scripts are off', off(SAVING, 'scripts')],
    ['both are off', off(SAVING, 'notes', 'scripts')],
    ['notes wait for review', ask(SAVING, 'notes')],
    ['scripts wait for review', ask(SAVING, 'scripts')],
    ['both wait for review', ask(SAVING, 'notes', 'scripts')],
    ['saving is paused', { ...SAVING, paused: true }],
  ])('says nothing and adds no row while %s', async (_what, guards) => {
    await show(guards);

    expect(lines()).toEqual([]);
    expect(hasMemoryRow(attached[0])).toBe(false);
  });

  it('shows a waiting card without a line about it', async () => {
    memoryHeld.items = sampleHeld(KEY);
    await show(ask(SAVING, 'notes'));

    expect(screen.getByRole('region', { name: 'Saves waiting for your review' })).toBeInTheDocument();
    expect(lines()).toEqual([]);
    expect(hasMemoryRow(attached[0])).toBe(true);
  });
});
