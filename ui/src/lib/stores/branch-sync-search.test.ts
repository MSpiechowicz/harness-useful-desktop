import { afterEach, describe, expect, it, vi } from 'vitest';

import type { BranchPage } from '$lib/ipc/bindings/BranchPage';
import { IpcFailure } from '$lib/ipc/transport';

import { BRANCH_PAGE_SIZE } from './branch-sync.svelte';
import { build, deferred, PROJECT, rows, sync } from './branch-sync-support';

afterEach(() => {
  vi.useRealTimers();
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

  it('loads the next page when asked, from where the last one ended, and stops once every match is loaded', async () => {
    const asked: number[] = [];
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: ({ offset, limit }: { offset: number; limit: number }): BranchPage => {
        asked.push(offset);

        return { branches: rows(offset, Math.min(limit, 120 - offset)), total: 120 };
      },
    });
    store.follow(PROJECT, null);

    await store.loadMore();
    expect(asked).toEqual([]);

    store.search('feat', true);
    await vi.waitFor(() => expect(store.page?.branches).toHaveLength(BRANCH_PAGE_SIZE));
    expect(store.hasMore).toBe(true);

    await store.loadMore();
    expect(store.page?.branches).toHaveLength(2 * BRANCH_PAGE_SIZE);
    expect(store.page?.branches[BRANCH_PAGE_SIZE].name).toBe(`feat/${BRANCH_PAGE_SIZE}`);

    await store.loadMore();
    expect(store.page?.branches).toHaveLength(120);
    expect(store.hasMore).toBe(false);

    await store.loadMore();
    expect(asked).toEqual([0, 50, 100]);
    expect(store.loadingMore).toBe(false);
  });

  it('asks for one page at a time', async () => {
    const next = deferred<BranchPage>();
    const asked: number[] = [];
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: ({ offset }: { offset: number }) => {
        asked.push(offset);

        return offset === 0 ? { branches: rows(0, BRANCH_PAGE_SIZE), total: 120 } : next.promise;
      },
    });
    store.follow(PROJECT, null);
    store.search('feat', true);
    await vi.waitFor(() => expect(store.page).not.toBeNull());

    const first = store.loadMore();
    const second = store.loadMore();
    expect(store.loadingMore).toBe(true);

    next.settle({ branches: rows(BRANCH_PAGE_SIZE, BRANCH_PAGE_SIZE), total: 120 });
    await Promise.all([first, second]);

    expect(asked).toEqual([0, 50]);
    expect(store.loadingMore).toBe(false);
  });

  it('continues the search the rows came from when a newer one failed', async () => {
    const asked: { query: string; offset: number }[] = [];
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: ({ query, offset }: { query: string; offset: number }): BranchPage => {
        asked.push({ query, offset });
        if (query === 'abc') {
          throw IpcFailure.from({ code: 'process', message: 'git failed' });
        }

        return { branches: rows(offset, BRANCH_PAGE_SIZE), total: 120 };
      },
    });
    store.follow(PROJECT, null);
    store.search('', true);
    await vi.waitFor(() => expect(store.page).not.toBeNull());

    store.search('abc', true);
    await vi.waitFor(() => expect(store.searchFailed).toBe(true));
    await store.loadMore();

    expect(asked.at(-1)).toEqual({ query: '', offset: BRANCH_PAGE_SIZE });
    // The rows answer the older text, so the failed newer search stays reported.
    expect(store.searchFailed).toBe(true);
  });

  it('starts over at the first page for a new query, replacing what was loaded', async () => {
    const calls: { query: string; offset: number }[] = [];
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: ({ query, offset }: { query: string; offset: number }): BranchPage => {
        calls.push({ query, offset });

        return query === 'feat' ? { branches: rows(offset, BRANCH_PAGE_SIZE), total: 200 } : { branches: [rows(900, 1)[0]], total: 1 };
      },
    });
    store.follow(PROJECT, null);
    store.search('feat', true);
    await vi.waitFor(() => expect(store.page).not.toBeNull());
    await store.loadMore();
    expect(store.page?.branches).toHaveLength(2 * BRANCH_PAGE_SIZE);

    store.search('other', true);
    await vi.waitFor(() => expect(store.page?.total).toBe(1));
    await store.loadMore();

    expect(store.page?.branches.map((row) => row.name)).toEqual(['feat/900']);
    expect(calls).toEqual([
      { query: 'feat', offset: 0 },
      { query: 'feat', offset: 50 },
      { query: 'other', offset: 0 },
    ]);
  });

  it('drops a page that arrives after the query changed', async () => {
    const late = deferred<BranchPage>();
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: ({ query, offset }: { query: string; offset: number }) => {
        if (query === 'feat') {
          return offset === 0 ? { branches: rows(0, BRANCH_PAGE_SIZE), total: 200 } : late.promise;
        }

        return { branches: rows(900, 1), total: 1 };
      },
    });
    store.follow(PROJECT, null);
    store.search('feat', true);
    await vi.waitFor(() => expect(store.page).not.toBeNull());

    const loading = store.loadMore();
    expect(store.loadingMore).toBe(true);
    store.search('other', true);
    await vi.waitFor(() => expect(store.page?.total).toBe(1));
    expect(store.loadingMore).toBe(false);

    late.settle({ branches: rows(BRANCH_PAGE_SIZE, BRANCH_PAGE_SIZE), total: 200 });
    await loading;

    expect(store.page?.branches.map((row) => row.name)).toEqual(['feat/900']);
    expect(store.hasMore).toBe(false);
  });

  it('drops a first page that arrives after a newer search', async () => {
    const slow = deferred<BranchPage>();
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: ({ query }: { query: string }) => (query === 'old' ? slow.promise : { branches: rows(900, 1), total: 1 }),
    });
    store.follow(PROJECT, null);

    store.search('old', true);
    store.search('new', true);
    await vi.waitFor(() => expect(store.page?.total).toBe(1));
    slow.settle({ branches: rows(0, BRANCH_PAGE_SIZE), total: 200 });
    await Promise.resolve();
    await Promise.resolve();

    expect(store.page?.branches.map((row) => row.name)).toEqual(['feat/900']);
  });

  it('shows a failed page and loads it on the next ask', async () => {
    let broken = true;
    const { store } = build({
      git_sync_state: () => sync(),
      git_branch_search: ({ offset }: { offset: number }) => {
        if (offset === 0) {
          return { branches: rows(0, BRANCH_PAGE_SIZE), total: 80 };
        }
        if (broken) {
          throw IpcFailure.from({ code: 'process', message: 'git failed' });
        }

        return { branches: rows(offset, 30), total: 80 };
      },
    });
    store.follow(PROJECT, null);
    store.search('feat', true);
    await vi.waitFor(() => expect(store.page).not.toBeNull());

    await store.loadMore();
    expect(store.searchFailed).toBe(true);
    expect(store.loadingMore).toBe(false);
    expect(store.page?.branches).toHaveLength(BRANCH_PAGE_SIZE);

    broken = false;
    await store.loadMore();
    expect(store.searchFailed).toBe(false);
    expect(store.page?.branches).toHaveLength(80);
  });
});
