import { afterEach, describe, expect, it, vi } from 'vitest';

import type { BranchPage } from '$lib/ipc/bindings/BranchPage';
import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
import type { GitActionResult } from '$lib/ipc/bindings/GitActionResult';
import type { GitStatus } from '$lib/ipc/bindings/GitStatus';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { IpcFailure } from '$lib/ipc/transport';

import { fakeTransport } from '../../../tests/helpers';
import { BranchSyncStore } from './branch-sync.svelte';
import { BRANCH_FETCH_GAP_MS } from './live';
import { ToastsStore } from './toasts.svelte';

const PROJECT = '/home/demo/project';
const OTHER = '/home/demo/other';

function sync(overrides: Partial<BranchSync> = {}): BranchSync {
  return { ...structuredClone(fixtures.git.sync), ...overrides };
}

function status(overrides: Partial<GitStatus> = {}): GitStatus {
  return { ...structuredClone(fixtures.git.status), ...overrides };
}

type Handlers = Parameters<typeof fakeTransport>[0];

function build(handlers: Handlers, { allowed = true }: { allowed?: boolean } = {}) {
  const notifications = new ToastsStore();
  const repository = { refresh: vi.fn(async () => {}) };
  const store = new BranchSyncStore(fakeTransport(handlers), notifications, repository, () => allowed);

  return { store, notifications, repository };
}

/** A reply the test settles when it wants to, to put answers in a chosen order. */
function deferred<T>() {
  let settle: (value: T) => void = () => {};
  const promise = new Promise<T>((resolve) => (settle = resolve));

  return { promise, settle };
}

const result = (state: BranchSync, outcome: GitActionResult['outcome'] = 'done'): GitActionResult => ({ outcome, state });

afterEach(() => {
  vi.useRealTimers();
});

describe('following the open project', () => {
  it('reads the state when a project opens, and again only when the status shows the branch moved', async () => {
    const read = vi.fn(() => sync());
    const { store } = build({ git_sync_state: read });

    store.follow(PROJECT, null);
    await vi.waitFor(() => expect(store.state?.branch).toBe('feat/empty-input'));
    expect(read).toHaveBeenCalledTimes(1);

    // The first status only says what the first read covered. Edits to a file do not move the branch either.
    store.follow(PROJECT, status());
    store.follow(PROJECT, status());
    await Promise.resolve();
    expect(read).toHaveBeenCalledTimes(1);

    store.follow(PROJECT, status({ head: 'a1b2c3d' }));
    await vi.waitFor(() => expect(read).toHaveBeenCalledTimes(2));

    store.follow(PROJECT, status({ head: 'a1b2c3d', ahead: 0 }));
    await vi.waitFor(() => expect(read).toHaveBeenCalledTimes(3));
  });

  it('forgets the old project at once and ignores its late answer', async () => {
    const late = deferred<BranchSync>();
    const { store } = build({
      git_sync_state: ({ project }: { project: string }) => (project === PROJECT ? late.promise : sync({ branch: 'other-branch' })),
    });

    store.follow(PROJECT, null);
    store.follow(OTHER, null);
    await vi.waitFor(() => expect(store.state?.branch).toBe('other-branch'));

    late.settle(sync({ branch: 'stale' }));
    await Promise.resolve();
    await Promise.resolve();

    expect(store.state?.branch).toBe('other-branch');
  });
});

describe('fetching the remote', () => {
  it('fetches no more than once a minute and joins a fetch that is under way', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    const reply = deferred<BranchSync>();
    const fetch = vi.fn(() => reply.promise);
    const { store } = build({ git_sync_state: () => sync(), git_fetch: fetch });
    store.follow(PROJECT, null);

    const first = store.fetchRemote();
    const second = store.fetchRemote();
    await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(1));
    reply.settle(sync({ behind: 5 }));
    await Promise.all([first, second]);
    expect(store.state?.behind).toBe(5);

    await store.fetchRemote();
    expect(fetch).toHaveBeenCalledTimes(1);

    vi.setSystemTime(Date.now() + BRANCH_FETCH_GAP_MS);
    await store.fetchRemote();
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it('does not touch the network when the user switched it off or there is no remote', async () => {
    const fetch = vi.fn(() => sync());
    const off = build({ git_sync_state: () => sync(), git_fetch: fetch }, { allowed: false });
    off.store.follow(PROJECT, null);
    await off.store.fetchRemote();

    const bare = build({
      git_sync_state: () => sync({ remote: null, upstream: null }),
      git_fetch: fetch,
    });
    bare.store.follow(PROJECT, null);
    await bare.store.fetchRemote();

    expect(fetch).not.toHaveBeenCalled();
  });

  it('says a failed fetch quietly, without a toast, and clears it once one works', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    let reachable = false;
    const { store, notifications } = build({
      git_sync_state: () => sync(),
      git_fetch: () =>
        reachable
          ? sync()
          : Promise.reject(
              IpcFailure.from({
                code: 'process',
                message: 'Could not resolve host',
              }),
            ),
    });
    store.follow(PROJECT, null);

    await store.fetchRemote();
    expect(store.fetchFailed).toBe(true);
    expect(notifications.items).toEqual([]);

    reachable = true;
    vi.setSystemTime(Date.now() + BRANCH_FETCH_GAP_MS);
    await store.fetchRemote();
    expect(store.fetchFailed).toBe(false);
  });

  it('keeps what an action just reported over the answer of a fetch that started before it', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    const slow = deferred<BranchSync>();
    const { store } = build({
      git_sync_state: () => sync(),
      git_fetch: () => slow.promise,
      git_sync_action: () => result(sync({ ahead: 0 })),
    });
    store.follow(PROJECT, null);

    const fetching = store.fetchRemote();
    await vi.waitFor(() => expect(store.state).not.toBeNull());
    await store.run('push');
    slow.settle(sync({ ahead: 2 }));
    await fetching;

    expect(store.state?.ahead).toBe(0);
  });
});

describe('running an action', () => {
  it('runs one at a time and reads git again afterwards', async () => {
    const reply = deferred<GitActionResult>();
    const act = vi.fn(() => reply.promise);
    const { store, repository } = build({
      git_sync_state: () => sync(),
      git_sync_action: act,
    });
    store.follow(PROJECT, null);
    await vi.waitFor(() => expect(store.state).not.toBeNull());

    const first = store.run('push');
    expect(store.busy).toBe('push');
    await store.run('pull');
    expect(act).toHaveBeenCalledTimes(1);

    reply.settle(result(sync({ ahead: 0 })));
    await first;

    expect(store.busy).toBeNull();
    expect(store.state?.ahead).toBe(0);
    expect(store.note).toMatchObject({
      tone: 'success',
      text: 'Pushed to origin/feat/empty-input',
    });
    expect(repository.refresh).toHaveBeenCalled();
  });

  it('says so when there was nothing to do, and says nothing for a rebase that stopped', async () => {
    let outcome: GitActionResult['outcome'] = 'upToDate';
    const { store } = build({
      git_sync_state: () => sync(),
      git_sync_action: () => result(sync(), outcome),
    });
    store.follow(PROJECT, null);
    await vi.waitFor(() => expect(store.state).not.toBeNull());

    await store.run('rebase');
    expect(store.note?.text).toBe('Already up to date');

    outcome = 'stopped';
    await store.run('rebase');
    expect(store.note).toBeNull();
  });

  it('shows git’s first error line as a toast, nothing more, and reads the state again', async () => {
    const read = vi.fn(() => sync());
    const { store, notifications } = build({
      git_sync_state: read,
      git_sync_action: () =>
        Promise.reject(
          IpcFailure.from({
            code: 'process',
            message: 'rejected: non-fast-forward\nhint: pull first\nhint: more',
          }),
        ),
    });
    store.follow(PROJECT, null);
    await vi.waitFor(() => expect(read).toHaveBeenCalledTimes(1));

    await store.run('push');

    expect(notifications.items).toHaveLength(1);
    expect(notifications.items[0]).toMatchObject({
      title: 'Couldn’t push',
      description: 'rejected: non-fast-forward',
      tone: 'danger',
    });
    expect(read).toHaveBeenCalledTimes(2);
    expect(store.busy).toBeNull();
  });
});

describe('reading the repository', () => {
  it('says so when the first read fails, and clears it once a read works', async () => {
    let fail = true;
    const { store } = build({
      git_sync_state: () => (fail ? Promise.reject(IpcFailure.from({ code: 'process', message: 'boom' })) : sync()),
    });

    store.follow(PROJECT, null);
    await vi.waitFor(() => expect(store.readFailed).toBe(true));
    expect(store.state).toBeNull();

    fail = false;
    await store.read();
    expect(store.readFailed).toBe(false);
    expect(store.state).not.toBeNull();
  });

  it('keeps the state it has when a later read fails', async () => {
    let fail = false;
    const { store } = build({
      git_sync_state: () => (fail ? Promise.reject(IpcFailure.from({ code: 'process', message: 'boom' })) : sync()),
    });

    store.follow(PROJECT, null);
    await vi.waitFor(() => expect(store.state).not.toBeNull());
    fail = true;
    await store.read();

    expect(store.readFailed).toBe(false);
    expect(store.state).not.toBeNull();
  });
});

describe('an abort or continue that fails', () => {
  it('toasts git’s first line for Abort and for Continue', async () => {
    const read = vi.fn(() => sync());
    const { store, notifications } = build({
      git_sync_state: read,
      git_sync_action: ({ action }: { action: string }) =>
        Promise.reject(
          IpcFailure.from({
            code: 'process',
            message: `${action} refused: hook failed\nhint: more`,
          }),
        ),
    });
    store.follow(PROJECT, null);
    await vi.waitFor(() => expect(read).toHaveBeenCalledTimes(1));

    await store.run('abort');
    await store.run('continue');

    expect(notifications.items.map((item) => [item.title, item.description, item.tone])).toEqual([
      ['Couldn’t abort', 'abort refused: hook failed', 'danger'],
      ['Couldn’t continue', 'continue refused: hook failed', 'danger'],
    ]);
    expect(store.busy).toBeNull();
  });
});

describe('switching branches', () => {
  it('offers to stash and switch when git refuses because of uncommitted files', async () => {
    const asked: boolean[] = [];
    const { store, notifications } = build({
      git_sync_state: () => sync(),
      git_branch_search: (): BranchPage => ({ branches: [], total: 0 }),
      git_switch: ({ request }: { request: { stash: boolean } }) => {
        asked.push(request.stash);

        return result(sync({ branch: request.stash ? 'main' : 'feat/empty-input' }), request.stash ? 'done' : 'blockedByChanges');
      },
    });
    store.follow(PROJECT, null);
    await vi.waitFor(() => expect(store.state).not.toBeNull());

    await store.switchTo({ branch: 'main', create: null, stash: false });
    const toast = notifications.items[0];
    expect(toast).toMatchObject({
      title: 'Couldn’t switch to main',
      tone: 'warning',
    });
    expect(toast.action?.label).toBe('Stash and switch');

    toast.action?.run();
    await vi.waitFor(() => expect(store.state?.branch).toBe('main'));
    expect(asked).toEqual([false, true]);
  });

  it('names what a new branch started from', async () => {
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: (): BranchPage => ({ branches: [], total: 0 }),
      git_switch: () => result(sync({ branch: 'feat/new', upstream: null })),
    });
    store.follow(PROJECT, null);
    await vi.waitFor(() => expect(store.state).not.toBeNull());

    await store.switchTo({ branch: 'feat/new', create: 'main', stash: false });

    expect(store.note).toMatchObject({
      text: 'Created feat/new from main',
      names: ['feat/new', 'main'],
    });
  });
});

describe('searching branches', () => {
  it('waits for the typing to pause, asks once, and ignores an answer that is no longer the latest', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const slow = deferred<BranchPage>();
    const queries: string[] = [];
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: ({ query }: { query: string }) => {
        queries.push(query);

        return query === 'fe'
          ? slow.promise
          : {
              branches: [
                {
                  name: 'feat/x',
                  isCurrent: false,
                  remoteOnly: false,
                  lastCommitDate: null,
                },
              ],
              total: 1,
            };
      },
    });
    store.follow(PROJECT, null);

    store.search('f');
    store.search('fe');
    expect(queries).toEqual([]);
    await vi.advanceTimersByTimeAsync(150);
    expect(queries).toEqual(['fe']);

    store.search('fea', true);
    await vi.waitFor(() => expect(store.page?.total).toBe(1));

    slow.settle({ branches: [], total: 0 });
    await vi.advanceTimersByTimeAsync(0);
    expect(store.page?.total).toBe(1);
  });
});

describe('the main branch', () => {
  it('saves the pick and shows the state it returns', async () => {
    const picked = vi.fn(() =>
      sync({
        main: {
          name: 'develop',
          reference: 'develop',
          source: 'picked',
          newCommits: 3,
          isCurrent: false,
        },
      }),
    );
    const { store } = build({
      git_sync_state: () => sync(),
      git_set_main_branch: picked,
    });
    store.follow(PROJECT, null);
    await vi.waitFor(() => expect(store.state).not.toBeNull());

    await store.pickMain('develop');

    expect(picked).toHaveBeenCalledWith({
      project: PROJECT,
      branch: 'develop',
    });
    expect(store.state?.main?.source).toBe('picked');
    expect(store.busy).toBeNull();
  });
});
