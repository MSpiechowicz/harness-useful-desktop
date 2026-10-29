import { describe, expect, it, vi } from 'vitest';

import type { CodeBuildEvent } from '$lib/ipc/bindings/CodeBuildEvent';
import type { CodeIndexState } from '$lib/ipc/bindings/CodeIndexState';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { decide, EMPTY_BUILD_TOASTS, MemoryCodeStore, type BuildToastState } from './memoryCode.svelte';
import { ToastsStore } from './toasts.svelte';

const MINUTE = 60_000;

function build(patch: Partial<CodeBuildEvent> = {}): CodeBuildEvent {
  return { buildId: 1, root: '/work/api', repoName: 'api', kind: 'full', phase: 'building', filesTotal: 240, filesDone: 24, error: null, ...patch };
}

/** Feeds the events through `decide` one after the other, like the store does. */
function run(steps: [CodeBuildEvent, number][]) {
  let state: BuildToastState = EMPTY_BUILD_TOASTS;

  return steps.map(([event, at]) => {
    const decision = decide(event, state, at);

    state = decision.state;

    return decision;
  });
}

describe('decide', () => {
  it('pushes one toast for a build not seen before, not tied to a project and without auto-dismiss', () => {
    const [first] = run([[build(), 0]]);

    expect(first).toMatchObject({
      action: 'push',
      toast: { title: 'Building code graph for api…', description: '24 of 240 files', duration: 0 },
    });
    expect(first.action === 'push' && first.toast.project).toBeUndefined();
  });

  it('leaves the description out while the files are not counted yet', () => {
    const [first] = run([[build({ filesTotal: 0, filesDone: 0 }), 0]]);

    expect(first.action === 'push' && first.toast.description).toBeUndefined();
  });

  it('updates the description as the same build goes on, and says nothing for the same event again', () => {
    const [, more, again] = run([
      [build(), 0],
      [build({ filesDone: 72 }), 1000],
      [build({ filesDone: 72 }), 1500],
    ]);

    expect(more).toMatchObject({ action: 'update', toast: { description: '72 of 240 files' } });
    expect(again.action).toBe('none');
  });

  it('turns the toast into a success that leaves after 4 seconds when the build is ready', () => {
    const [, ready] = run([
      [build(), 0],
      [build({ phase: 'ready', filesDone: 240 }), 1000],
    ]);

    expect(ready).toMatchObject({ action: 'update', toast: { title: 'Code graph ready · 240 files', tone: 'success', duration: 4000, description: undefined } });
  });

  it('says "1 file" for one file', () => {
    const [, ready] = run([
      [build({ filesTotal: 1, filesDone: 0 }), 0],
      [build({ phase: 'ready', filesTotal: 1, filesDone: 1 }), 1000],
    ]);

    expect(ready).toMatchObject({ toast: { title: 'Code graph ready · 1 file' } });
  });

  it('shows a failed build as a danger toast with the reason, that stays until dismissed', () => {
    const [, failed] = run([
      [build(), 0],
      [build({ phase: 'error', error: 'disk full' }), 1000],
    ]);

    expect(failed).toMatchObject({ action: 'update', toast: { title: 'Code graph could not be built', description: 'disk full', tone: 'danger', duration: 0 } });
  });

  it.each(['stale', 'cancelled'] as const)('takes the toast away without a word when the build is %s', (phase) => {
    const [, gone] = run([
      [build(), 0],
      [build({ phase }), 1000],
    ]);

    expect(gone.action).toBe('dismiss');
  });

  it('shows a build that was done before the backend saw it as one short success toast', () => {
    const [lone] = run([[build({ phase: 'ready', filesDone: 240 }), 0]]);

    expect(lone).toMatchObject({ action: 'push', toast: { title: 'Code graph ready · 240 files', tone: 'success', duration: 4000 } });
  });

  it('shows nothing for a stopped build it never saw', () => {
    const [none] = run([[build({ phase: 'cancelled' }), 0]]);

    expect(none.action).toBe('none');
  });

  it('shows no second toast for a build that is done, however many events follow', () => {
    const decisions = run([
      [build(), 0],
      [build({ phase: 'ready', filesDone: 240 }), 1000],
      [build({ phase: 'ready', filesDone: 240 }), 2000],
      [build({ filesDone: 120 }), 3000],
      [build({ phase: 'error', error: 'late' }), 4000],
    ]);

    expect(decisions.map((each) => each.action)).toEqual(['push', 'update', 'none', 'none', 'none']);
  });

  it('ignores a new build of the same repository within 10 minutes of its last toast, ready included', () => {
    const decisions = run([
      [build({ buildId: 1 }), 0],
      [build({ buildId: 1, phase: 'ready', filesDone: 240 }), 5 * MINUTE],
      [build({ buildId: 2 }), 9 * MINUTE],
      [build({ buildId: 2, filesDone: 100 }), 9 * MINUTE + 1000],
      [build({ buildId: 2, phase: 'ready', filesDone: 240 }), 9 * MINUTE + 2000],
    ]);

    expect(decisions.map((each) => each.action)).toEqual(['push', 'update', 'none', 'none', 'none']);
  });

  it('counts the 10 minutes from the last change of the toast, and announces a build after that', () => {
    const decisions = run([
      [build({ buildId: 1 }), 0],
      [build({ buildId: 1, phase: 'ready', filesDone: 240 }), 5 * MINUTE],
      [build({ buildId: 2 }), 15 * MINUTE + 1],
    ]);

    expect(decisions.map((each) => each.action)).toEqual(['push', 'update', 'push']);
  });

  it('reopens one toast when a build that went stale writes again, and not a second one while it is shown', () => {
    const decisions = run([
      [build(), 0],
      [build({ phase: 'stale' }), 1000],
      [build({ filesDone: 48 }), 2000],
      [build({ filesDone: 72 }), 3000],
      [build({ phase: 'ready', filesDone: 240 }), 4000],
    ]);

    expect(decisions.map((each) => each.action)).toEqual(['push', 'dismiss', 'push', 'update', 'update']);
    expect(decisions[2]).toMatchObject({ toast: { description: '48 of 240 files', duration: 0 } });
  });

  it('does not reopen a cancelled build', () => {
    const decisions = run([
      [build(), 0],
      [build({ phase: 'cancelled' }), 1000],
      [build({ filesDone: 48 }), 2000],
    ]);

    expect(decisions.map((each) => each.action)).toEqual(['push', 'dismiss', 'none']);
  });

  it('does not reopen a build cancelled while it was paused', () => {
    const decisions = run([
      [build(), 0],
      [build({ phase: 'stale' }), 1000],
      [build({ phase: 'cancelled' }), 2000],
      [build({ filesDone: 48 }), 3000],
    ]);

    expect(decisions.map((each) => each.action)).toEqual(['push', 'dismiss', 'none', 'none']);
  });

  it('always shows an error, even within 10 minutes of the last toast of the repository, once for the build', () => {
    const decisions = run([
      [build({ buildId: 1 }), 0],
      [build({ buildId: 1, phase: 'ready', filesDone: 240 }), MINUTE],
      [build({ buildId: 2 }), 2 * MINUTE],
      [build({ buildId: 2, phase: 'error', error: 'disk full' }), 3 * MINUTE],
      [build({ buildId: 2, phase: 'error', error: 'disk full' }), 3 * MINUTE + 500],
      [build({ buildId: 3, phase: 'error', error: 'no space' }), 4 * MINUTE],
    ]);

    expect(decisions.map((each) => each.action)).toEqual(['push', 'update', 'none', 'push', 'none', 'push']);
    expect(decisions[3]).toMatchObject({ toast: { title: 'Code graph could not be built', description: 'disk full', tone: 'danger' } });
  });

  it('forgets builds and repositories after 30 minutes, and still keeps the quiet time of 10 minutes', () => {
    let state: BuildToastState = EMPTY_BUILD_TOASTS;

    for (const [event, at] of [
      [build({ buildId: 1 }), 0],
      [build({ buildId: 1, phase: 'ready', filesDone: 240 }), MINUTE],
    ] as [CodeBuildEvent, number][]) {
      state = decide(event, state, at).state;
    }

    expect(Object.keys(state.builds)).toEqual(['1']);

    const quiet = decide(build({ buildId: 2, root: '/work/web' }), state, 20 * MINUTE);
    expect(quiet.state.builds[1]).toBeDefined();

    const later = decide(build({ buildId: 3, root: '/work/web' }), quiet.state, 40 * MINUTE);
    expect(Object.keys(later.state.builds).sort()).toEqual(['2', '3']);
    expect(Object.keys(later.state.shownAt)).toEqual(['/work/web']);
  });

  it('keeps builds of other repositories apart', () => {
    const decisions = run([
      [build({ buildId: 1 }), 0],
      [build({ buildId: 2, root: '/work/web', repoName: 'web' }), 1000],
    ]);

    expect(decisions.map((each) => each.action)).toEqual(['push', 'push']);
  });
});

describe('the build toasts of the store', () => {
  function setup() {
    const notifications = new ToastsStore();
    const push = vi.spyOn(notifications, 'push');
    const update = vi.spyOn(notifications, 'update');
    const dismiss = vi.spyOn(notifications, 'dismiss');
    const store = new MemoryCodeStore({ invoke: vi.fn() } as unknown as Transport, notifications, () => 0);

    return { notifications, store, push, update, dismiss };
  }

  it('shows one toast for a whole build: it changes for progress and when ready, and the same event again changes nothing', () => {
    const { notifications, store, push, update } = setup();

    store.handle(build({ filesDone: 0 }));
    store.handle(build({ filesDone: 24 }));
    store.handle(build({ filesDone: 144 }));

    for (let each = 0; each < 20; each += 1) {
      store.handle(build({ filesDone: 144 }));
    }

    expect(push).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenCalledTimes(2);
    expect(notifications.items).toHaveLength(1);
    expect(notifications.items[0]).toMatchObject({ description: '144 of 240 files', duration: 0 });

    store.handle(build({ phase: 'ready', filesDone: 240 }));

    expect(push).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenCalledTimes(3);
    expect(notifications.items).toEqual([expect.objectContaining({ title: 'Code graph ready · 240 files', tone: 'success', duration: 4000, description: undefined })]);
  });

  it('keeps the toast of a build when the user changes project, and turns it ready', () => {
    const { notifications, store } = setup();

    store.handle(build());
    notifications.leaveProjects('/work/other');
    store.handle(build({ filesDone: 120 }));

    expect(notifications.items).toEqual([expect.objectContaining({ description: '120 of 240 files' })]);

    notifications.leaveProjects(null);
    store.handle(build({ phase: 'ready', filesDone: 240 }));

    expect(notifications.items).toEqual([expect.objectContaining({ title: 'Code graph ready · 240 files', tone: 'success' })]);
  });

  it('shows a build that writes again after going stale as one toast', () => {
    const { notifications, store } = setup();

    store.handle(build());
    store.handle(build({ phase: 'stale' }));
    expect(notifications.items).toEqual([]);

    store.handle(build({ filesDone: 96 }));
    store.handle(build({ filesDone: 120 }));

    expect(notifications.items).toEqual([expect.objectContaining({ description: '120 of 240 files' })]);
  });

  it('removes the toast of a build that was cancelled', () => {
    const { notifications, store } = setup();

    store.handle(build());
    store.handle(build({ phase: 'cancelled' }));

    expect(notifications.items).toEqual([]);
  });

  it('does not bring a dismissed toast back with later progress', () => {
    const { notifications, store, push } = setup();

    store.handle(build());
    notifications.dismiss(notifications.items[0].id);
    store.handle(build({ filesDone: 100 }));

    expect(notifications.items).toEqual([]);
    expect(push).toHaveBeenCalledTimes(1);
  });
});

describe('the code index switch of the store', () => {
  const ON: CodeIndexState = { enabled: true, source: 'default' };

  function withReplies(replies: { get?: () => CodeIndexState; set?: (on: boolean) => CodeIndexState }) {
    const invoke = vi.fn(async (name: string, args?: { on?: boolean }) => {
      if (name === 'memory_code_index_get') {
        return (replies.get ?? (() => ON))();
      }

      return (replies.set ?? ((on: boolean) => ({ enabled: on, source: 'file' as const })))(args?.on ?? false);
    });

    return { invoke, store: new MemoryCodeStore({ invoke } as unknown as Transport, new ToastsStore()) };
  }

  it('shows nothing until the server answered, and nothing for a server without a code index', async () => {
    const missing = withReplies({
      get: () => {
        throw IpcFailure.from({ code: 'unsupported', message: 'no code index' });
      },
    });

    expect(missing.store.index).toBeNull();
    await missing.store.load();
    expect(missing.store.index).toBeNull();
  });

  it('switches at once and keeps what the server reports', async () => {
    const { store } = withReplies({});

    await store.load();
    const done = store.setEnabled(false);

    expect(store.index?.enabled).toBe(false);

    await done;
    expect(store.index).toEqual({ enabled: false, source: 'file' });
  });

  it('goes back and says why when the server refuses', async () => {
    const { store } = withReplies({
      set: () => {
        throw IpcFailure.from({ code: 'process', message: 'The settings file is read-only' });
      },
    });

    await store.load();
    await store.setEnabled(false);

    expect(store.index).toEqual(ON);
    expect(store.failure).toBe('The settings file is read-only');
  });

  it('does not let a read that began before a change overwrite it', async () => {
    let answer: (state: CodeIndexState) => void = () => undefined;
    const invoke = vi.fn((name: string, args?: { on?: boolean }) => {
      if (name === 'memory_code_index_get') {
        return invoke.mock.calls.length === 1 ? Promise.resolve(ON) : new Promise<CodeIndexState>((resolve) => (answer = resolve));
      }

      return Promise.resolve({ enabled: args?.on ?? false, source: 'file' as const });
    });
    const store = new MemoryCodeStore({ invoke } as unknown as Transport, new ToastsStore());

    await store.load();

    const stale = store.load();
    await store.setEnabled(false);
    answer(ON);
    await stale;

    expect(store.index).toEqual({ enabled: false, source: 'file' });
  });

  it('does not change a switch the environment decides', async () => {
    const { store, invoke } = withReplies({ get: () => ({ enabled: false, source: 'env' }) });

    await store.load();
    await store.setEnabled(true);

    expect(invoke).not.toHaveBeenCalledWith('memory_code_index_set', expect.anything());
    expect(store.index?.enabled).toBe(false);
  });
});
