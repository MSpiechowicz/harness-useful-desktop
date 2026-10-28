import { afterEach, describe, expect, it, vi } from 'vitest';

import type { BranchPage } from '$lib/ipc/bindings/BranchPage';
import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
import type { GitActionResult } from '$lib/ipc/bindings/GitActionResult';
import type { GitStatus } from '$lib/ipc/bindings/GitStatus';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { IpcFailure } from '$lib/ipc/transport';

import { BRANCH_PAGE_SIZE } from './branch-sync.svelte';
import { build, deferred, PROJECT, rows, sync } from './branch-sync-support';
import { BRANCH_FETCH_GAP_MS } from './live';

const OTHER = '/home/demo/other';

function status(overrides: Partial<GitStatus> = {}): GitStatus {
  return { ...structuredClone(fixtures.git.status), ...overrides };
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

  it('searches again when a fetch lands after the list was shown, so new remote branches appear', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    let fetched = false;
    const search = vi.fn(
      (): BranchPage => ({
        branches: rows(0, fetched ? 2 : 1),
        total: fetched ? 2 : 1,
      }),
    );
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: search,
      git_fetch: () => {
        fetched = true;

        return sync();
      },
    });
    store.follow(PROJECT, null);
    store.search('feat', true);
    await vi.waitFor(() => expect(store.page?.total).toBe(1));

    await store.fetchRemote();

    await vi.waitFor(() => expect(store.page?.branches.map((row) => row.name)).toEqual(['feat/0', 'feat/1']));
    expect(search).toHaveBeenCalledTimes(2);
    expect(search).toHaveBeenLastCalledWith({ project: PROJECT, query: 'feat', offset: 0, limit: BRANCH_PAGE_SIZE });
  });

  it('keeps a list scrolled past its first page when a fetch lands', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    const search = vi.fn(({ offset }: { offset: number }): BranchPage => ({ branches: rows(offset, BRANCH_PAGE_SIZE), total: 500 }));
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: search,
      git_fetch: () => sync(),
    });
    store.follow(PROJECT, null);
    store.search('feat', true);
    await vi.waitFor(() => expect(store.page?.branches).toHaveLength(BRANCH_PAGE_SIZE));
    await store.loadMore();
    expect(store.page?.branches).toHaveLength(2 * BRANCH_PAGE_SIZE);

    await store.fetchRemote();

    expect(search).toHaveBeenCalledTimes(2);
    expect(store.page?.branches).toHaveLength(2 * BRANCH_PAGE_SIZE);
  });

  it('searches again when a fetch lands while the first search is still on its way, and shows that answer', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    const first = deferred<BranchPage>();
    let calls = 0;
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: () => {
        calls += 1;

        return calls === 1 ? first.promise : { branches: rows(0, 2), total: 2 };
      },
      git_fetch: () => sync(),
    });
    store.follow(PROJECT, null);
    store.search('feat', true);
    expect(store.page).toBeNull();

    await store.fetchRemote();
    await vi.waitFor(() => expect(store.page?.total).toBe(2));

    first.settle({ branches: rows(0, 1), total: 1 });
    await Promise.resolve();
    expect(calls).toBe(2);
    expect(store.page?.total).toBe(2);
  });

  it('leaves a shown list unmarked when the search after a fetch fails', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    let fetched = false;
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: (): BranchPage => {
        if (fetched) {
          throw IpcFailure.from({ code: 'process', message: 'git failed' });
        }

        return { branches: rows(0, 1), total: 1 };
      },
      git_fetch: () => {
        fetched = true;

        return sync();
      },
    });
    store.follow(PROJECT, null);
    store.search('feat', true);
    await vi.waitFor(() => expect(store.page?.total).toBe(1));

    await store.fetchRemote();
    await vi.waitFor(() => expect(store.searching).toBe(false));

    expect(store.searchFailed).toBe(false);
    expect(store.page?.total).toBe(1);
  });

  it('reports a newer search that a fetch took over and that failed, since the rows answer the older text', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    const typed = deferred<BranchPage>();
    let asked = 0;
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: ({ query }: { query: string }) => {
        if (query === 'ab') {
          asked += 1;
          if (asked > 1) {
            throw IpcFailure.from({ code: 'process', message: 'git failed' });
          }

          return typed.promise;
        }

        return { branches: rows(0, 1), total: 1 };
      },
      git_fetch: () => sync(),
    });
    store.follow(PROJECT, null);
    store.search('a', true);
    await vi.waitFor(() => expect(store.page?.total).toBe(1));
    store.search('ab', true);

    await store.fetchRemote();
    await vi.waitFor(() => expect(store.searching).toBe(false));

    expect(asked).toBe(2);
    expect(store.searchFailed).toBe(true);
    expect(store.page?.total).toBe(1);
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
