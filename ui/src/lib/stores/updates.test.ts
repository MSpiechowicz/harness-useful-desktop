import { afterEach, describe, expect, it, vi } from 'vitest';

import type { AppPrefs } from '$lib/ipc/bindings/AppPrefs';
import type { UpdateInfo } from '$lib/ipc/bindings/UpdateInfo';
import type { UpdateProgress } from '$lib/ipc/bindings/UpdateProgress';
import { IpcFailure } from '$lib/ipc/transport';

import { samplePrefs, fakeTransport } from '../../../tests/helpers';
import { toasts, ToastsStore } from './toasts.svelte';
import { UpdatesStore, type UpdateState } from './updates.svelte';

const INFO: UpdateInfo = { version: '0.2.0', currentVersion: '0.1.0', date: '2026-10-08T18:42:11Z', notes: '## 0.2.0' };
const NEWER: UpdateInfo = { ...INFO, version: '0.3.0' };

const DOWNLOAD: UpdateProgress[] = [
  { phase: 'downloading', downloaded: 500, total: 1000 },
  { phase: 'downloading', downloaded: 1000, total: 1000 },
  { phase: 'installing', downloaded: 1000, total: 1000 },
  { phase: 'done', downloaded: 1000, total: 1000 },
];

interface Setup {
  check?: () => UpdateInfo | null | Promise<UpdateInfo | null>;
  install?: (onProgress: (progress: UpdateProgress) => void) => void | Promise<void>;
  policy?: AppPrefs['updatePolicy'];
  windows?: boolean;
  agents?: boolean;
  /** Chats in the middle of a turn. */
  working?: number;
}

function setup({ check = () => INFO, install = () => {}, policy = 'startupAsk', windows = false, agents = false, working = 0 }: Setup = {}) {
  const calls: string[] = [];
  const transport = fakeTransport({
    update_check: () => (calls.push('check'), check()),
    update_install: async ({ onProgress }: { onProgress: (progress: UpdateProgress) => void }) => {
      calls.push('install');
      await install(onProgress);

      return null;
    },
    app_relaunch: () => (calls.push('relaunch'), null),
  });
  const store = new UpdatesStore(transport, { value: samplePrefs({ updatePolicy: policy }) }, new ToastsStore(), {
    installerClosesApp: () => windows,
    agentsRunning: () => agents,
    workingChats: () => working,
  });

  return { store, calls };
}

function failing(code: IpcFailure['code']): () => never {
  return () => {
    throw new IpcFailure({ code, message: 'details that must not reach the screen' });
  };
}

describe('startup policy', () => {
  it('startupAsk checks and offers the update without installing it', async () => {
    const { store, calls } = setup();

    await store.startup();

    expect(calls).toEqual(['check']);
    expect(store.state).toEqual({ kind: 'available', info: INFO });
    expect(store.noticeVisible).toBe(true);
  });

  it('autoInstall checks, installs, then offers the restart', async () => {
    const { store, calls } = setup({ policy: 'autoInstall', install: (onProgress) => DOWNLOAD.forEach(onProgress) });

    await store.startup();

    expect(calls).toEqual(['check', 'install']);
    expect(store.state).toEqual({ kind: 'readyToRelaunch', info: INFO });
    expect(store.noticeVisible).toBe(true);
  });

  it('off does nothing, and the manual check still works', async () => {
    const { store, calls } = setup({ policy: 'off' });

    await store.startup();
    expect(calls).toEqual([]);
    expect(store.state.kind).toBe('idle');

    await store.check();
    expect(store.state.kind).toBe('available');
  });

  it('runs once, however often the shell asks', async () => {
    const { store, calls } = setup({ check: () => null });

    await Promise.all([store.startup(), store.startup()]);
    await store.startup();

    expect(calls).toEqual(['check']);
    expect(store.state.kind).toBe('upToDate');
  });

  it.each([
    ['a build without an update key', 'unsupported', 'notConfigured'],
    ['a network failure', 'timeout', 'failed'],
    ['a broken manifest', 'protocol', 'failed'],
  ] as const)('stays silent about %s', async (_name, code, kind) => {
    const { store, calls } = setup({ check: failing(code), policy: 'autoInstall' });

    await store.startup();

    expect(calls).toEqual(['check']);
    expect(store.state.kind).toBe(kind);
    expect(store.noticeVisible).toBe(false);
  });

  it('asks instead of installing when the installer would close the app under a running agent', async () => {
    const { store, calls } = setup({ policy: 'autoInstall', windows: true, agents: true });

    await store.startup();

    expect(calls).toEqual(['check']);
    expect(store.state.kind).toBe('available');
  });
});

describe('watching while the app stays open', () => {
  afterEach(() => vi.useRealTimers());

  it('finds a release made after startup without a restart', async () => {
    vi.useFakeTimers();
    let release: UpdateInfo | null = null;
    const { store, calls } = setup({ check: () => release });
    await store.startup();
    expect(store.state.kind).toBe('upToDate');
    const stop = store.watch();

    release = INFO;
    await vi.advanceTimersByTimeAsync(30 * 60_000);

    expect(calls).toEqual(['check', 'check']);
    expect(store.state).toEqual({ kind: 'available', info: INFO });
    stop();
  });

  it('keeps a found update and stays quiet when a later look fails', async () => {
    vi.useFakeTimers();
    let fail = false;
    const { store } = setup({ check: () => (fail ? failing('timeout')() : null) });
    await store.startup();
    const stop = store.watch();

    fail = true;
    await vi.advanceTimersByTimeAsync(30 * 60_000);

    expect(store.state.kind).toBe('upToDate');
    stop();
  });

  it('does not look again once an update is available', async () => {
    vi.useFakeTimers();
    const { store, calls } = setup();
    await store.startup();
    const stop = store.watch();

    await vi.advanceTimersByTimeAsync(60 * 60_000);

    expect(calls).toEqual(['check']);
    stop();
  });
});

describe('installing', () => {
  it('moves through downloading and installing with the reported progress', async () => {
    const seen: UpdateState[] = [];
    const { store } = setup({
      install: (onProgress) => DOWNLOAD.forEach((progress) => (onProgress(progress), seen.push(store.state))),
    });
    await store.check();

    await store.install();

    expect(seen.map((state) => state.kind)).toEqual(['downloading', 'downloading', 'installing', 'installing']);
    expect(seen[0]).toMatchObject({ progress: { downloaded: 500 } });
    expect(store.state.kind).toBe('readyToRelaunch');
  });

  it('reports a fraction while the size is known and none when it is not', async () => {
    const fractions: (number | null)[] = [];
    const { store } = setup({
      install: (onProgress) => {
        onProgress({ phase: 'downloading', downloaded: 250, total: 1000 });
        fractions.push(store.fraction);
        onProgress({ phase: 'downloading', downloaded: 250, total: null });
        fractions.push(store.fraction);
      },
    });
    await store.check();

    await store.install();

    expect(fractions).toEqual([0.25, null]);
  });

  it('keeps the bar full once the download is done and the install runs', async () => {
    const fractions: (number | null)[] = [];
    const { store } = setup({
      install: (onProgress) => {
        onProgress({ phase: 'downloading', downloaded: 700, total: 1000 });
        onProgress({ phase: 'installing', downloaded: 1000, total: 1000 });
        fractions.push(store.fraction);
      },
    });
    await store.check();

    await store.install();

    expect(fractions).toEqual([1]);
  });

  it('installs only an update a check found, and only once at a time', async () => {
    const { store, calls } = setup();

    await store.install();
    expect(calls).toEqual([]);

    await store.check();
    const first = store.install();
    await store.install();
    await first;

    expect(calls.filter((call) => call === 'install')).toHaveLength(1);
  });

  it('shows a signature failure as a verification failure that cannot be retried', async () => {
    const { store, calls } = setup({ install: failing('forbidden') });
    await store.check();

    await store.install();
    await store.install();

    expect(store.state).toMatchObject({ kind: 'failed', during: 'install', reason: 'verification', info: INFO });
    expect(calls.filter((call) => call === 'install')).toHaveLength(1);
  });

  it('lets a network failure be retried with the same update', async () => {
    let attempts = 0;
    const { store } = setup({
      install: () => {
        attempts += 1;
        if (attempts === 1) {
          failing('io')();
        }
      },
    });
    await store.check();

    await store.install();
    expect(store.state).toMatchObject({ kind: 'failed', reason: 'network' });

    await store.install();
    expect(store.state.kind).toBe('readyToRelaunch');
  });

  it('ignores progress that arrives after the install ended', async () => {
    let late: (progress: UpdateProgress) => void = () => {};
    const { store } = setup({ install: (onProgress) => ((late = onProgress), failing('process')()) });
    await store.check();
    await store.install();

    late({ phase: 'downloading', downloaded: 1, total: 2 });

    expect(store.state).toMatchObject({ kind: 'failed', reason: 'installer' });
  });

  it('asks the backend to restart, and toasts when that fails', async () => {
    const { store, calls } = setup();

    await store.relaunch();
    expect(calls).toEqual(['relaunch']);

    const broken = new UpdatesStore(fakeTransport({ app_relaunch: failing('internal') }), { value: samplePrefs() }, toasts, { installerClosesApp: () => false, agentsRunning: () => false, workingChats: () => 0 });
    await broken.relaunch();
    expect(toasts.items.some((toast) => toast.tone === 'danger')).toBe(true);
  });
});

describe('the update notice', () => {
  it('"Later" hides the notice for this version only, and the update stays pending', async () => {
    const versions = [INFO, NEWER];
    const { store } = setup({ check: () => versions.shift() ?? null });
    await store.check();

    store.dismiss();
    expect(store.noticeVisible).toBe(false);
    expect(store.pending).toBe(true);

    await store.check();
    expect(store.state).toMatchObject({ info: NEWER });
    expect(store.noticeVisible).toBe(true);
  });

  it('comes back for the restart prompt after the download notice was sent away', async () => {
    const { store } = setup({ install: (onProgress) => (onProgress(DOWNLOAD[0]), store.dismiss()) });
    await store.check();

    await store.install();

    expect(store.state.kind).toBe('readyToRelaunch');
    expect(store.noticeVisible).toBe(true);
  });

  it('shows a failed install, because someone asked for it', async () => {
    const { store } = setup({ install: failing('process') });
    await store.check();

    await store.install();

    expect(store.noticeVisible).toBe(true);
  });

  it('does not check again while an install runs', async () => {
    const release: { done: () => void } = { done: () => {} };
    const { store, calls } = setup({ install: () => new Promise<void>((resolve) => (release.done = resolve)) });
    await store.check();
    const running = store.install();

    await store.check();
    release.done();
    await running;

    expect(calls.filter((call) => call === 'check')).toHaveLength(1);
  });

  it('does not check again once installed, so the old version cannot offer the same update a second time', async () => {
    const { store, calls } = setup();
    await store.check();
    await store.install();

    await store.check();

    expect(store.state).toEqual({ kind: 'readyToRelaunch', info: INFO });
    expect(calls.filter((call) => call === 'check')).toHaveLength(1);
  });
});

describe('restarting', () => {
  it('restarts straight away when no chat is in the middle of a turn', async () => {
    const { store, calls } = setup({ working: 0, agents: true });

    await store.requestRelaunch();

    expect(calls).toEqual(['relaunch']);
    expect(store.confirmingRestart).toBe(false);
  });

  it('waits for confirmation while chats are working, and says how many would stop', async () => {
    const { store, calls } = setup({ working: 2 });

    await store.requestRelaunch();

    expect(calls).toEqual([]);
    expect(store.confirmingRestart).toBe(true);
    expect(store.chatsAtRisk).toBe(2);

    await store.relaunch();

    expect(calls).toEqual(['relaunch']);
  });
});
