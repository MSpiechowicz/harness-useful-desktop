import { describe, expect, it } from 'vitest';

import { fixtures } from './fixtures';
import { MockHooks } from './hooks-state';
import { HELD_NOTE_APPEND_ID, HELD_NOTE_SAVE_ID, HELD_NOTE_UPDATE_ID, HELD_REVISIONS, HELD_SCRIPT_SAVE_ID, PICKED_FOLDER, SAMPLE_CHAT_KEY } from './memory-guards-fixtures';
import { MockMemoryGuards } from './memory-guards-handlers';
import { defaultScenario } from './scenarios';
import { createMockTransport } from './transport';

const CHAT = '3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b';
const GUARDS = { paused: false, notes: { on: true, ask: true }, scripts: { on: true, ask: false } };

describe('the mock save guards', () => {
  it('gives the sample held saves to the first chat with a memory identity', async () => {
    const guards = new MockMemoryGuards(() => 'ok');
    const handlers = guards.handlers();
    const changes: string[] = [];
    await handlers.memory_held_listen({ onEvent: (event) => changes.push(event.kind) });

    expect(await handlers.memory_held_list({ chatKey: CHAT })).toEqual([]);

    guards.chatStarted('agent-1', '/home/demo/project', { key: CHAT, guards: GUARDS });
    const held = await handlers.memory_held_list({ chatKey: CHAT });

    expect(held.map((item) => item.tool)).toEqual(['script_save', 'note_update', 'note_save', 'note_update']);
    expect(held.find((item) => item.heldId === HELD_NOTE_UPDATE_ID)?.current).toMatchObject({ id: 3 });
    expect(held.every((item) => item.revision === HELD_REVISIONS[item.heldId])).toBe(true);
    expect(changes).toEqual(['changed']);
  });

  it('saves and skips held saves, then they are gone', async () => {
    const handlers = new MockMemoryGuards(() => 'ok').handlers();

    expect(await handlers.memory_held_save({ heldId: HELD_NOTE_UPDATE_ID, edited: null, revision: HELD_REVISIONS[HELD_NOTE_UPDATE_ID] })).toEqual({ kind: 'note', id: 3 });
    expect(await handlers.memory_held_save({ heldId: HELD_SCRIPT_SAVE_ID, edited: null, revision: null })).toEqual({ kind: 'script', name: 'check-bindings' });
    await handlers.memory_held_skip({ heldId: HELD_NOTE_SAVE_ID, revision: HELD_REVISIONS[HELD_NOTE_SAVE_ID] });
    expect(await handlers.memory_held_save({ heldId: HELD_NOTE_APPEND_ID, edited: null, revision: HELD_REVISIONS[HELD_NOTE_APPEND_ID] })).toEqual({ kind: 'note', id: 7 });

    expect(await handlers.memory_held_list({ chatKey: SAMPLE_CHAT_KEY })).toEqual([]);
    expect(() => handlers.memory_held_skip({ heldId: HELD_NOTE_SAVE_ID, revision: null })).toThrow(expect.objectContaining({ code: 'notFound' }));
    expect(() => handlers.memory_held_skip({ heldId: 'not-a-uuid', revision: null })).toThrow(expect.objectContaining({ code: 'invalidInput' }));
    expect(() => handlers.memory_held_skip({ heldId: HELD_SCRIPT_SAVE_ID, revision: '--all' })).toThrow(expect.objectContaining({ code: 'invalidInput' }));
  });

  it('refuses a save the agent held again since the user saw it, and keeps the newer one', async () => {
    const guards = new MockMemoryGuards(() => 'changed');
    const handlers = guards.handlers();
    const seen = HELD_REVISIONS[HELD_NOTE_SAVE_ID];

    expect(() => handlers.memory_held_save({ heldId: HELD_NOTE_SAVE_ID, edited: null, revision: seen })).toThrow(expect.objectContaining({ code: 'changed' }));

    const newer = (await handlers.memory_held_list({ chatKey: SAMPLE_CHAT_KEY })).find((item) => item.heldId === HELD_NOTE_SAVE_ID);
    expect(newer?.revision).not.toBe(seen);
    expect(newer?.arguments).toMatchObject({ body: expect.stringContaining('Changed by the agent.') });

    expect(await handlers.memory_held_save({ heldId: HELD_NOTE_SAVE_ID, edited: null, revision: newer?.revision ?? null })).toEqual({ kind: 'note', id: expect.any(Number) });
  });

  it('refuses a skip the agent held again since the user saw it, and keeps the newer one', async () => {
    const handlers = new MockMemoryGuards(() => 'changed').handlers();
    const seen = HELD_REVISIONS[HELD_NOTE_SAVE_ID];

    expect(() => handlers.memory_held_skip({ heldId: HELD_NOTE_SAVE_ID, revision: seen })).toThrow(expect.objectContaining({ code: 'changed' }));

    const newer = (await handlers.memory_held_list({ chatKey: SAMPLE_CHAT_KEY })).find((item) => item.heldId === HELD_NOTE_SAVE_ID);
    expect(newer?.revision).not.toBe(seen);

    await handlers.memory_held_skip({ heldId: HELD_NOTE_SAVE_ID, revision: newer?.revision ?? null });
    expect((await handlers.memory_held_list({ chatKey: SAMPLE_CHAT_KEY })).some((item) => item.heldId === HELD_NOTE_SAVE_ID)).toBe(false);
  });

  it('applies each change to what is saved now, so changes made at once all apply', async () => {
    const handlers = new MockMemoryGuards(() => 'empty').handlers();

    const [first, second] = await Promise.all([
      handlers.memory_guards_update({ patch: { kind: 'notes', addRepo: '/b/' } }),
      handlers.memory_guards_update({ patch: { kind: 'notes', addRepo: '/a' } }),
    ]);
    expect(first.notes.offRepos).toEqual(['/b']);
    expect(second.notes.offRepos).toEqual(['/a', '/b']);

    const paused = await handlers.memory_guards_update({ patch: { kind: 'scripts', paused: true } });
    expect(paused).toEqual({ notes: { paused: false, offRepos: ['/a', '/b'] }, scripts: { paused: true, offRepos: [] } });

    const removed = await handlers.memory_guards_update({ patch: { kind: 'notes', removeRepo: '/b/' } });
    expect(removed.notes.offRepos).toEqual(['/a']);
    expect(await handlers.memory_guards_get(undefined)).toEqual(removed);
    expect(() => handlers.memory_guards_update({ patch: { kind: 'notes', addRepo: 'relative' } })).toThrow(expect.objectContaining({ code: 'invalidInput' }));
  });

  it('ignores a chat guards write that is not newer than the last one', async () => {
    const mock = createMockTransport(defaultScenario(), 0);
    const request = { harness: 'omp', project: fixtures.recentProjects[0].path, resume: null, approvalOverride: null, trustOnce: false, utility: false } as const;
    const info = await mock.invoke('agent_start', { request: { ...request, memoryChat: { key: CHAT, guards: GUARDS } }, onEvent: () => {} });

    await expect(mock.invoke('memory_chat_guards_set', { key: CHAT, guards: { ...GUARDS, paused: true }, revision: 2 })).resolves.toEqual({ applied: true, revision: 2 });
    await expect(mock.invoke('memory_chat_guards_set', { key: CHAT, guards: GUARDS, revision: 1 })).resolves.toEqual({ applied: false, revision: 2 });
    await expect(mock.invoke('chat_memory_check_again', { agentId: info.key })).rejects.toMatchObject({ code: 'invalidInput' });

    await expect(mock.invoke('memory_chat_guards_set', { key: CHAT, guards: GUARDS, revision: 3 })).resolves.toEqual({ applied: true, revision: 3 });
    await expect(mock.invoke('chat_memory_check_again', { agentId: info.key })).resolves.toBeNull();

    await mock.invoke('agent_stop', { key: info.key });
  });

  it('picks a folder without opening a project', async () => {
    const mock = createMockTransport(defaultScenario(), 0);
    const before = await mock.invoke('project_recent', undefined);

    await expect(mock.invoke('dialog_pick_folder', undefined)).resolves.toBe(PICKED_FOLDER);
    expect(await mock.invoke('project_recent', undefined)).toEqual(before);
  });

  it('checks again with the memory-save hook as the user edited it', () => {
    const hooks = new MockHooks(() => 'ok', () => true);
    const project = fixtures.recentProjects[0].path;

    expect(hooks.memorySaveFollowUp(project)).toMatch(/^<harness-hook-followup name="Save what you learned">Most turns need no note/);

    const view = hooks.list(null).hooks.find((candidate) => candidate.hook?.seed === 'memory-save');
    const hook = view?.hook;
    if (!view || !hook) {
      throw new Error('the sample memory-save hook is missing');
    }

    hooks.save({ ...hook, id: hook.id, expectedSha256: view.sha256, action: { type: 'followUp', text: 'Save only the fixes.' } });

    expect(hooks.memorySaveFollowUp(project)).toBe('<harness-hook-followup name="Save what you learned">Save only the fixes.</harness-hook-followup>');
  });

  it('refuses Check again while the chat saves nothing', async () => {
    const mock = createMockTransport(defaultScenario(), 0);
    const request = { harness: 'omp', project: fixtures.recentProjects[0].path, resume: null, approvalOverride: null, trustOnce: false, utility: false } as const;
    const info = await mock.invoke('agent_start', { request: { ...request, memoryChat: { key: CHAT, guards: GUARDS } }, onEvent: () => {} });

    await expect(mock.invoke('chat_memory_check_again', { agentId: info.key })).resolves.toBeNull();

    await mock.invoke('memory_chat_guards_set', { key: CHAT, guards: { ...GUARDS, paused: true }, revision: 1 });
    await expect(mock.invoke('chat_memory_check_again', { agentId: info.key })).rejects.toMatchObject({ code: 'invalidInput' });

    await mock.invoke('agent_stop', { key: info.key });
  });
});
