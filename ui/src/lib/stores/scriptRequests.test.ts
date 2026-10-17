import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { ScriptRequestEvent } from '$lib/ipc/bindings/ScriptRequestEvent';
import type { ScriptRunRequest } from '$lib/ipc/bindings/ScriptRunRequest';
import type { CommandArgs, CommandName, CommandResult } from '$lib/ipc/commands';
import { sampleScripts } from '$lib/ipc/mock/memory-scripts-fixtures';
import { sampleRunRequests } from '$lib/ipc/mock/script-request-fixtures';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { APPROVAL_WINDOW_MS, ScriptRequestsStore } from './scriptRequests.svelte';
import { ToastsStore } from './toasts.svelte';

const NOW = Date.parse('2026-10-17T10:00:00Z');

/** A backend that records what it is asked and lets a test decide how `memory_script_decide` ends. */
function setup(decide: () => Promise<null> = () => Promise.resolve(null)) {
  const sent: { name: string; args: unknown }[] = [];
  let sink: ((event: ScriptRequestEvent) => void) | undefined;

  const transport: Transport = {
    invoke: async <K extends CommandName>(name: K, args: CommandArgs<K>) => {
      sent.push({ name, args });

      if (name === 'memory_script_requests_listen') {
        sink = (args as CommandArgs<'memory_script_requests_listen'>).onRequest;
      }

      return (name === 'memory_script_decide' ? await decide() : null) as CommandResult<K>;
    },
  };
  const toasts = new ToastsStore();
  const store = new ScriptRequestsStore(transport, toasts);

  return { store, toasts, sent, emit: (event: ScriptRequestEvent) => sink?.(event) };
}

function requests(): [ScriptRunRequest, ScriptRunRequest] {
  const [first, second] = sampleRunRequests(sampleScripts(), NOW);

  return [first, second];
}

beforeEach(() => {
  window.sessionStorage.clear();
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('script run requests', () => {
  it('shows the request that waited longest and counts the ones behind it', () => {
    const { store } = setup();
    const [first, second] = requests();

    expect(store.current).toBeNull();

    store.push(first, NOW);
    store.push(second, NOW);

    expect(store.current?.requestId).toBe(first.requestId);
    expect(store.behind).toBe(1);
    expect(store.deadline).toBe(NOW + APPROVAL_WINDOW_MS);
  });

  it('counts the window from when the server wrote the request', () => {
    const { store } = setup();
    const [first] = requests();

    store.push({ ...first, createdAt: new Date(NOW - 30_000).toISOString() }, NOW);

    expect(store.deadline).toBe(NOW + 15_000);
  });

  it('counts from the moment the backend received it when that is earlier than the date the server wrote', () => {
    const { store } = setup();
    const [first] = requests();

    store.push({ ...first, createdAt: new Date(NOW - 5_000).toISOString(), receivedAt: new Date(NOW - 20_000).toISOString() }, NOW);

    expect(store.deadline).toBe(NOW + 25_000);
  });

  it('shows a request that just arrived even when its written date is old by our clock, counted from its arrival', () => {
    const { store } = setup();
    const [first] = requests();

    store.push({ ...first, createdAt: new Date(NOW - 90_000).toISOString(), receivedAt: new Date(NOW - 1_000).toISOString() }, NOW);

    expect(store.current?.requestId).toBe(first.requestId);
    expect(store.deadline).toBe(NOW - 1_000 + 40_000);
  });

  it('still drops a request that the backend received long ago', () => {
    const { store } = setup();
    const [first] = requests();

    store.push({ ...first, createdAt: new Date(NOW - 90_000).toISOString(), receivedAt: new Date(NOW - 60_000).toISOString() }, NOW);

    expect(store.current).toBeNull();
  });

  it('counts a date ahead of our clock from the arrival', () => {
    const { store } = setup();
    const [first] = requests();

    store.push({ ...first, createdAt: new Date(NOW + 60_000).toISOString(), receivedAt: new Date(NOW - 3_000).toISOString() }, NOW);

    expect(store.deadline).toBe(NOW - 3_000 + APPROVAL_WINDOW_MS);
  });

  it('counts from now when the date makes no sense, and ignores a request nobody waits for any more', () => {
    const { store } = setup();
    const [first, second] = requests();

    store.push({ ...first, createdAt: 'not a date' }, NOW);
    expect(store.deadline).toBe(NOW + APPROVAL_WINDOW_MS);

    store.push({ ...second, createdAt: new Date(NOW - 50_000).toISOString(), receivedAt: new Date(NOW - 50_000).toISOString() }, NOW);
    expect(store.behind).toBe(0);
  });

  it('keeps a request reported twice once, as the backend repeats the waiting ones when it starts', () => {
    const { store } = setup();
    const [first] = requests();

    store.push(first, NOW);
    store.push(first, NOW);

    expect(store.behind).toBe(0);
  });

  it('sends the answer, and the next request takes over at once', async () => {
    const { store, sent } = setup();
    const [first, second] = requests();
    store.push(first, NOW);
    store.push(second, NOW);

    const answered = store.decide(true);
    expect(store.current?.requestId).toBe(second.requestId);
    await answered;

    expect(sent).toEqual([{ name: 'memory_script_decide', args: { requestId: first.requestId, allow: true } }]);

    await store.decide(false);

    expect(sent.at(-1)).toEqual({ name: 'memory_script_decide', args: { requestId: second.requestId, allow: false } });
    expect(store.current).toBeNull();
  });

  it('does not ask again about a request it answered, when the backend reports it again after a reload', async () => {
    const { store } = setup();
    const [first, second] = requests();
    store.push(first, NOW);
    store.push(second, NOW);
    await store.decide(false);

    store.push(first, NOW);
    expect(store.current?.requestId).toBe(second.requestId);
    expect(store.behind).toBe(0);

    // A fresh store, as after the page reloaded, hears about the same answer.
    const reloaded = setup().store;
    reloaded.push(first, NOW);

    expect(reloaded.current).toBeNull();
  });

  it('remembers the last 50 answers only', async () => {
    const { store } = setup();
    const [first] = requests();
    const withId = (n: number): ScriptRunRequest => ({ ...first, requestId: n.toString(16).padStart(32, '0') });

    for (let n = 1; n <= 51; n += 1) {
      store.push(withId(n), NOW);
      await store.decide(false);
    }

    store.push(withId(1), NOW);
    expect(store.current?.requestId).toBe(withId(1).requestId);

    store.push(withId(51), NOW);
    expect(store.behind).toBe(0);
  });

  it('answers one request at a time', async () => {
    let finish: () => void = () => {};
    const { store, sent } = setup(() => new Promise((resolve) => (finish = () => resolve(null))));
    const [first, second] = requests();
    store.push(first, NOW);
    store.push(second, NOW);

    const answering = store.decide(true);
    void store.decide(true);
    finish();
    await answering;

    expect(sent).toHaveLength(1);
  });

  it('says only that the request ended when the agent stopped waiting before the answer arrived', async () => {
    const { store, toasts } = setup(() => Promise.reject(IpcFailure.from({ code: 'notFound', message: 'the request is gone' })));
    const [first] = requests();
    store.push(first, NOW);

    await store.decide(true);

    expect(store.current).toBeNull();
    expect(toasts.items).toMatchObject([{ title: 'This request ended', tone: 'neutral' }]);
  });

  it('says so when the answer could not be sent, and still moves on', async () => {
    const { store, toasts } = setup(() => Promise.reject(IpcFailure.from({ code: 'process', message: 'the server did not answer' })));
    const [first] = requests();
    store.push(first, NOW);

    await store.decide(true);

    expect(store.current).toBeNull();
    expect(toasts.items).toMatchObject([{ title: 'Could not send your answer for deploy-check', description: 'the server did not answer', tone: 'danger' }]);
  });

  it('drops a request the agent stopped waiting for, with a quiet note, and ignores one it does not know', () => {
    const { store, toasts } = setup();
    const [first, second] = requests();
    store.push(first, NOW);
    store.push(second, NOW);

    store.ended('f'.repeat(32));
    expect(toasts.items).toEqual([]);

    store.ended(first.requestId);

    expect(store.current?.requestId).toBe(second.requestId);
    expect(toasts.items).toMatchObject([{ title: 'The request to run deploy-check ended', description: 'The agent stopped waiting for an answer.', tone: 'neutral' }]);
  });

  it('gives a request up itself when the server never reports it gone', () => {
    const { store, toasts } = setup();
    const [first] = requests();
    store.push(first, NOW);

    vi.advanceTimersByTime(APPROVAL_WINDOW_MS);
    expect(store.current).not.toBeNull();

    vi.advanceTimersByTime(2000);

    expect(store.current).toBeNull();
    expect(toasts.items).toHaveLength(1);
  });

  it('does not report a request gone after it was answered', async () => {
    const { store, toasts } = setup();
    const [first] = requests();
    store.push(first, NOW);

    await store.decide(false);
    store.ended(first.requestId);
    vi.advanceTimersByTime(APPROVAL_WINDOW_MS + 2000);

    expect(toasts.items).toEqual([]);
  });

  it('listens to the backend, queues what it sends and stops when told to', async () => {
    const { store, sent, emit } = setup();
    const [first, second] = requests();

    const stop = store.start();
    await vi.advanceTimersByTimeAsync(0);
    expect(sent.map((call) => call.name)).toEqual(['memory_script_requests_listen']);

    emit({ type: 'request', request: first });
    emit({ type: 'request', request: second });
    expect(store.behind).toBe(1);

    emit({ type: 'gone', requestId: first.requestId });
    expect(store.current?.requestId).toBe(second.requestId);

    stop();
    emit({ type: 'request', request: first });
    expect(store.current?.requestId).toBe(second.requestId);
    expect(store.behind).toBe(0);
  });
});
