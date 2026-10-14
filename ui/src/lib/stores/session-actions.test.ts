import { waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi, type MockInstance } from 'vitest';

import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
import { ipc } from '$lib/ipc/client';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { IpcFailure } from '$lib/ipc/transport';

import { PROJECT, storeWithMock } from '../../../tests/chat-helpers';
import { cleanUpCopies, deleteSession } from './session-actions';
import { sessionList } from './sessions.svelte';
import { toasts } from './toasts.svelte';
import { viewState } from './view-state.svelte';

const REF = fixtures.sessions.list.omp[1].ref;

describe('deleting sessions', () => {
  const copy = (id: string, parentSession: string | null = null) => ({ ref: { harness: 'omp', id }, emptyCopy: true, parentSession }) as SessionSummary;
  const deletes = (invoke: MockInstance<typeof ipc.invoke>) => invoke.mock.calls.filter(([name]) => name === 'session_delete').map(([, args]) => args);

  afterEach(() => {
    vi.restoreAllMocks();
    toasts.items = [];
    viewState.sessionId = null;
    sessionList.sync(null, null);
  });

  it('deletes a session once the user confirms, and forgets it was selected', async () => {
    const invoke = vi.spyOn(ipc, 'invoke').mockResolvedValue(null as never);
    const { store } = storeWithMock();
    viewState.sessionId = REF.id;

    expect(await deleteSession(store, REF, PROJECT, () => Promise.resolve(true))).toBe(true);

    expect(deletes(invoke)).toEqual([{ session: REF, project: PROJECT, emptyCopyOnly: false }]);
    expect(viewState.sessionId).toBeNull();
  });

  it('keeps the session when the user cancels', async () => {
    const invoke = vi.spyOn(ipc, 'invoke').mockResolvedValue(null as never);
    const { store } = storeWithMock();
    viewState.sessionId = REF.id;

    expect(await deleteSession(store, REF, PROJECT, () => Promise.resolve(false))).toBe(false);

    expect(deletes(invoke)).toHaveLength(0);
    expect(viewState.sessionId).toBe(REF.id);
  });

  it('refuses a session an open chat runs, without asking', async () => {
    const invoke = vi.spyOn(ipc, 'invoke').mockResolvedValue(null as never);
    const { store } = storeWithMock();
    const running = { harness: 'omp', id: fixtures.agent.state.state.sessionId } as const;
    await store.create('omp', PROJECT);
    await waitFor(() => expect(store.sessions[0].chat.session?.sessionId).toBe(running.id));
    const confirm = vi.fn(() => Promise.resolve(true));

    expect(await deleteSession(store, running, PROJECT, confirm)).toBe(false);

    expect(confirm).not.toHaveBeenCalled();
    expect(deletes(invoke)).toHaveLength(0);
    expect(toasts.items.at(-1)?.title).toBe('This session is open in a chat');
  });

  it('explains a refusal of the backend, which knows agents this window does not', async () => {
    vi.spyOn(ipc, 'invoke').mockRejectedValue(IpcFailure.from({ code: 'forbidden', message: 'Something generic.' }));
    const { store } = storeWithMock();

    expect(await deleteSession(store, REF, PROJECT, () => Promise.resolve(true))).toBe(false);

    expect(toasts.items.at(-1)?.title).toBe('This session is open in a chat');
  });

  it('cleans up the empty copies one after the other, leaving out one an open chat runs', async () => {
    const invoke = vi.spyOn(ipc, 'invoke').mockResolvedValue(null as never);
    const { store } = storeWithMock();
    const running = fixtures.agent.state.state.sessionId;
    await store.create('omp', PROJECT);
    await waitFor(() => expect(store.sessions[0].chat.session?.sessionId).toBe(running));

    expect(await cleanUpCopies(store, [copy('copy-a'), copy(running), copy('copy-b')], PROJECT, () => Promise.resolve(true))).toBe(2);

    expect(deletes(invoke)).toEqual([
      { session: { harness: 'omp', id: 'copy-a' }, project: PROJECT, emptyCopyOnly: true },
      { session: { harness: 'omp', id: 'copy-b' }, project: PROJECT, emptyCopyOnly: true },
    ]);
    expect(toasts.items.at(-1)).toMatchObject({ title: 'Deleted 2 empty copies', description: '1 copy kept: it is open in a chat' });
  });

  it('skips copies that are gone or held by an agent, deletes the rest, and says how many an agent kept', async () => {
    const refusals: Record<string, IpcFailure> = {
      'copy-a': IpcFailure.from({ code: 'notFound', message: 'That session no longer exists.' }),
      'copy-b': IpcFailure.from({ code: 'forbidden', message: 'An agent holds it.' }),
    };
    const invoke = vi.spyOn(ipc, 'invoke').mockImplementation((name, args) => {
      const refusal = name === 'session_delete' ? refusals[(args as { session: { id: string } }).session.id] : undefined;

      return refusal ? Promise.reject(refusal) : Promise.resolve(null as never);
    });
    const { store } = storeWithMock();

    expect(await cleanUpCopies(store, [copy('copy-a'), copy('copy-b'), copy('copy-c'), copy('copy-d')], PROJECT, () => Promise.resolve(true))).toBe(2);

    expect(deletes(invoke)).toHaveLength(4);
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0]).toMatchObject({ title: 'Deleted 2 empty copies', tone: 'success', description: '1 copy kept: it is open in a chat' });
  });

  it('says so when every copy that is left is held by an agent', async () => {
    vi.spyOn(ipc, 'invoke').mockRejectedValue(IpcFailure.from({ code: 'forbidden', message: 'An agent holds it.' }));
    const { store } = storeWithMock();

    expect(await cleanUpCopies(store, [copy('copy-a'), copy('copy-b')], PROJECT, () => Promise.resolve(true))).toBe(0);

    expect(toasts.items).toMatchObject([{ title: '2 copies kept: they are open in a chat', tone: 'warning' }]);
  });

  it('stops cleaning up at the first copy that cannot be deleted', async () => {
    const invoke = vi.spyOn(ipc, 'invoke').mockImplementation((name, args) =>
      name === 'session_delete' && (args as { session: { id: string } }).session.id === 'copy-a' ? Promise.reject(IpcFailure.from({ code: 'io', message: 'Busy.' })) : Promise.resolve(null as never),
    );
    const { store } = storeWithMock();

    expect(await cleanUpCopies(store, [copy('copy-a'), copy('copy-b')], PROJECT, () => Promise.resolve(true))).toBe(0);

    expect(deletes(invoke)).toHaveLength(1);
    expect(toasts.items.at(-1)).toMatchObject({ title: 'Could not delete the session', description: 'Busy.' });
  });

  it('deletes a chain of empty copies from the deepest up, so no copy loses its parent before its own check', async () => {
    const gone = new Set<string>();
    const invoke = vi.spyOn(ipc, 'invoke').mockImplementation((name, args) => {
      if (name !== 'session_delete') {
        return Promise.resolve(null as never);
      }

      // The backend re-checks that a copy still has its parent.
      const { session } = args as { session: { id: string } };
      const parent = { 'copy-b': 'copy-a', 'copy-c': 'copy-b' }[session.id];
      if (parent && gone.has(parent)) {
        return Promise.reject(IpcFailure.from({ code: 'invalidInput', message: 'Not an empty copy.' }));
      }

      gone.add(session.id);

      return Promise.resolve(null as never);
    });
    const { store } = storeWithMock();
    const chain = [copy('copy-a', 'root'), copy('copy-b', 'copy-a'), copy('copy-c', 'copy-b')];

    expect(await cleanUpCopies(store, chain, PROJECT, () => Promise.resolve(true))).toBe(3);

    expect(deletes(invoke).map((args) => (args as { session: { id: string } }).session.id)).toEqual(['copy-c', 'copy-b', 'copy-a']);
    expect(toasts.items.map((toast) => toast.tone)).toEqual(['success']);
  });

  it('skips a copy the backend no longer finds empty, without a failure toast, and carries on', async () => {
    const invoke = vi.spyOn(ipc, 'invoke').mockImplementation((name, args) =>
      name === 'session_delete' && (args as { session: { id: string } }).session.id === 'copy-a'
        ? Promise.reject(IpcFailure.from({ code: 'invalidInput', message: 'Not an empty copy.' }))
        : Promise.resolve(null as never),
    );
    const { store } = storeWithMock();

    expect(await cleanUpCopies(store, [copy('copy-a'), copy('copy-b')], PROJECT, () => Promise.resolve(true))).toBe(1);

    expect(deletes(invoke)).toHaveLength(2);
    expect(toasts.items.map((toast) => toast.title)).toEqual(['Deleted 1 empty copy']);
  });

  it('drops the row of a session whose deletion failed after the file was already gone', async () => {
    const [kept, ghost] = fixtures.sessions.list.omp.filter((summary) => !summary.emptyCopy);
    let listed = [kept, ghost];
    vi.spyOn(ipc, 'invoke').mockImplementation((name) => {
      if (name === 'sessions_list') {
        return Promise.resolve(listed as never);
      }

      listed = [kept];

      return Promise.reject(IpcFailure.from({ code: 'io', message: 'Could not remove the sidecar.' }));
    });
    const { store } = storeWithMock();
    sessionList.sync('omp', PROJECT);
    await waitFor(() => expect(sessionList.data).toHaveLength(2));
    viewState.sessionId = ghost.ref.id;

    expect(await deleteSession(store, ghost.ref, PROJECT, () => Promise.resolve(true))).toBe(false);

    expect(toasts.items.at(-1)?.title).toBe('Could not delete the session');
    expect(sessionList.data?.map((summary) => summary.ref.id)).toEqual([kept.ref.id]);
    expect(viewState.sessionId).toBeNull();
  });

  it('keeps the selection of a session whose deletion failed and which is still there', async () => {
    const [target] = fixtures.sessions.list.omp.filter((summary) => !summary.emptyCopy);
    vi.spyOn(ipc, 'invoke').mockImplementation((name) =>
      name === 'sessions_list' ? Promise.resolve([target] as never) : Promise.reject(IpcFailure.from({ code: 'io', message: 'Busy.' })),
    );
    const { store } = storeWithMock();
    sessionList.sync('omp', PROJECT);
    await waitFor(() => expect(sessionList.data).toHaveLength(1));
    viewState.sessionId = target.ref.id;

    expect(await deleteSession(store, target.ref, PROJECT, () => Promise.resolve(true))).toBe(false);

    expect(viewState.sessionId).toBe(target.ref.id);
  });
});
