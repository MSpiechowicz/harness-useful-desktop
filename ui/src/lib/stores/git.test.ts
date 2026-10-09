import { waitFor } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { FileDiff } from '$lib/ipc/bindings/FileDiff';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { PROJECT, recordingTransport } from '../../../tests/chat-helpers';
import { GitStore } from './git.svelte';
import { nav } from './nav.svelte';

const ENTRIES = fixtures.git.status.entries;

async function loaded(store: GitStore): Promise<void> {
  await waitFor(() => expect(store.loadStatus).toBe('ready'));
  await waitFor(() => expect(store.entries.length === 0 || store.diffs[store.entries[0].path]?.status === 'ready').toBe(true));
}

function open() {
  const backend = recordingTransport();
  const store = new GitStore(backend.transport);

  store.activate(PROJECT);

  return { store, ...backend };
}

describe('changes', () => {
  it('reads status and branches, selects the first file and diffs everything in one request', async () => {
    const { store, of } = open();
    await loaded(store);

    expect(store.status?.branch).toBe(fixtures.git.status.branch);
    expect(store.branches).toHaveLength(fixtures.git.branches.length);
    expect(store.entries).toHaveLength(ENTRIES.length);
    expect(store.selected).toBe(store.entries[0].path);

    const requests = of('git_diff');
    expect(requests).toHaveLength(1);
    expect(requests[0].request).toEqual({ scope: 'all', paths: [] });
    expect(store.entries.every((entry) => store.diffFor(entry.path)?.status === 'ready')).toBe(true);
  });

  it('narrows the files and re-asks for the diffs when the scope changes', async () => {
    const { store, of } = open();
    await loaded(store);

    store.setScope('staged');
    await loaded(store);

    expect(store.entries.map((entry) => entry.path)).toEqual(ENTRIES.filter((entry) => entry.staged !== null).map((entry) => entry.path));
    expect(of('git_diff').at(-1)?.request.scope).toBe('staged');
    expect(store.entries.every((entry) => entry.staged !== null)).toBe(true);

    store.setScope('untracked');
    await loaded(store);

    expect(store.entries.every((entry) => entry.unstaged === 'untracked')).toBe(true);
  });

  it('moves the selection onto a visible file when its file drops out of the scope', async () => {
    const { store } = open();
    await loaded(store);
    const modified = ENTRIES.find((entry) => entry.unstaged === 'modified' && entry.staged === null);
    store.select(modified?.path ?? null);

    store.setScope('staged');

    expect(store.entries.some((entry) => entry.path === store.selected)).toBe(true);
  });

  it('limits the view to the files a session touched and goes to Changes', async () => {
    const { store, of } = open();
    await loaded(store);
    nav.go('chat');
    const [first, second] = ENTRIES.map((entry) => entry.path);

    store.showPaths(PROJECT, [first, `./${second}`], 'this session');

    expect(nav.current).toBe('changes');
    expect(store.filter).toEqual({ label: 'this session', paths: [first, second] });
    expect(store.entries.map((entry) => entry.path)).toEqual([first, second]);
    await waitFor(() => expect(of('git_diff').at(-1)?.request.paths).toEqual(expect.arrayContaining([first, second])));

    store.clearFilter();

    expect(store.entries).toHaveLength(ENTRIES.length);
  });

  it('keeps a renamed file under its old name too, so its diff can pair the rename', async () => {
    const { store, of } = open();
    await loaded(store);
    const renamed = ENTRIES.find((entry) => entry.origPath !== null);
    expect(renamed).toBeDefined();

    store.showPaths(PROJECT, [renamed?.path ?? ''], 'this chat');

    await waitFor(() => expect(of('git_diff').at(-1)?.request.paths).toEqual([renamed?.path, renamed?.origPath]));
  });

  it('reports a folder that is not a repository without an error', async () => {
    const backend = recordingTransport();
    const store = new GitStore(backend.transport);
    const notRepo = fixtures.recentProjects.find((candidate) => !candidate.isGitRepo && candidate.exists);

    store.activate(notRepo?.path ?? null);
    await waitFor(() => expect(store.loadStatus).toBe('ready'));

    expect(store.status?.isRepo).toBe(false);
    expect(store.entries).toEqual([]);
    expect(store.error).toBeNull();
  });

  it('forgets the old project completely when another one opens', async () => {
    const { store } = open();
    await loaded(store);
    store.showPaths(PROJECT, [ENTRIES[0].path], 'this chat');

    store.activate(fixtures.recentProjects[1].path);

    expect(store.filter).toBeNull();
    expect(store.status).toBeNull();
    expect(store.selected).toBeNull();
    await waitFor(() => expect(store.loadStatus).toBe('ready'));
  });

  it('shows a failed diff as an error that can be retried', async () => {
    const backend = recordingTransport();
    let failures = 1;
    const flaky: Transport = {
      invoke: (name, args) => {
        if (name === 'git_diff' && failures > 0) {
          failures -= 1;

          return Promise.reject(IpcFailure.from({ code: 'internal', message: 'git timed out' }));
        }

        return backend.transport.invoke(name, args);
      },
    };
    const store = new GitStore(flaky);

    store.activate(PROJECT);
    await waitFor(() => expect(store.diffFor(ENTRIES[0].path)?.status).toBe('error'));
    store.retryDiff(ENTRIES[0].path);

    await waitFor(() => expect(store.diffFor(ENTRIES[0].path)?.status).toBe('ready'));
    const state = store.diffFor(ENTRIES[0].path);
    expect(state?.status === 'ready' ? state.diffs.every((diff: FileDiff) => diff.path === ENTRIES[0].path) : false).toBe(true);
  });
});
