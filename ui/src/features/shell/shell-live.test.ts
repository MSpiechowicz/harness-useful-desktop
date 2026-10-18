import { render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { ipc } from '$lib/ipc/client';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { GIT_POLL_MS, HARNESS_FOCUS_GAP_MS, RETRY_BACKOFF_MS } from '$lib/stores/live';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { samplePrefs } from '../../../tests/helpers';
import Shell from './Shell.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp', onboardingCompleted: true });
  await harness.load();
  await project.pick();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  git.activate(null);
  nav.go('chat');
});

/** The backend as it is, except that `answer` may replace a reply: what changes while the app is open. */
function backendWith(answer: (name: string, reply: unknown) => unknown): void {
  const real = ipc.invoke.bind(ipc);

  vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) => real(name as never, args as never).then((reply) => answer(name, reply))) as typeof ipc.invoke);
}

describe('the shell keeps what it shows current', () => {
  const FILES = fixtures.git.status.entries.length;

  it('updates the changed-files count in the rail while Changes is closed', async () => {
    let committed = false;
    backendWith((name, reply) => (name === 'git_status' && committed ? { ...(reply as typeof fixtures.git.status), entries: fixtures.git.status.entries.slice(1) } : reply));
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    render(Shell);
    await waitFor(() => expect(screen.getByRole('button', { name: `Changes, ${FILES} changed files` })).toBeInTheDocument());

    const diffReads = () => vi.mocked(ipc.invoke).mock.calls.filter(([name]) => name === 'git_diff').length;
    const before = diffReads();

    // Committed in a terminal: nobody pressed anything in the app.
    committed = true;
    await vi.advanceTimersByTimeAsync(GIT_POLL_MS);

    await waitFor(() => expect(screen.getByRole('button', { name: `Changes, ${FILES - 1} changed files` })).toBeInTheDocument());
    // The count needs no diffs: they are read when Changes opens.
    expect(diffReads()).toBe(before);
  });

  it('reads git on the retry schedule while it cannot be read, not at the poll pace as well', async () => {
    // The previous test's shell may have started a read on its way out: begin from nothing read.
    git.activate(null);
    let reads = 0;
    const real = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) => {
      if (name !== 'git_status') {
        return real(name as never, args as never);
      }

      reads += 1;

      return Promise.reject({ code: 'process', message: 'not a git repository' });
    }) as typeof ipc.invoke);
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval'] });
    render(Shell);
    await vi.advanceTimersByTimeAsync(1000);
    expect(git.loadStatus).toBe('error');
    const first = reads;

    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[0] + RETRY_BACKOFF_MS[1] + 100);

    // One more read per step of the schedule, though the four-second poll ticked twice meanwhile.
    expect(reads).toBe(first + 2);
  });

  it('finds a harness installed meanwhile when the window gets focus back, but not on every focus', async () => {
    backendWith((name, reply) => {
      const summary = reply as typeof fixtures.harnesses[number];

      return name === 'harness_detect' && summary.info.id === 'pi' ? { ...summary, detection: { ...summary.detection, installed: true } } : reply;
    });
    render(Shell);
    expect(harness.get('pi')?.detection.installed).toBe(false);

    // Right after the last scan nothing is asked again.
    window.dispatchEvent(new Event('focus'));
    await new Promise((resolve) => setTimeout(resolve, 60));
    expect(harness.get('pi')?.detection.installed).toBe(false);

    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(Date.now() + HARNESS_FOCUS_GAP_MS);
    window.dispatchEvent(new Event('focus'));

    await waitFor(() => expect(harness.get('pi')?.detection.installed).toBe(true));
  });
});
