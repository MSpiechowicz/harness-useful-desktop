import { render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { SessionMessagesPage } from '$lib/ipc/bindings/SessionMessagesPage';
import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { RETRY_BACKOFF_MS, SESSIONS_POLL_MS } from '$lib/stores/live';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { SessionDetail, sessionDetail, sessionList } from '$lib/stores/sessions.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { fakeTransport } from '../../../tests/helpers';
import SessionsView from './SessionsView.svelte';

const PROJECT = fixtures.recentProjects[0].path;
const SESSIONS = fixtures.sessions.list.omp;
const SHOWN = SESSIONS.filter((summary) => !summary.emptyCopy);

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  // happy-dom lays nothing out: give every element a 640px viewport so virtual lists render a window of rows.
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 640 });

  await prefs.load();
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  await project.open(PROJECT);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  // The stores outlive a test: the next one starts from nothing loaded.
  sessionList.sync(null, null);
  sessionDetail.open(null, null);
  git.activate(null);
  nav.go('chat');
});

/** A session somebody started in a terminal while the view was open. */
const STARTED_ELSEWHERE: SessionSummary = {
  ...SHOWN[0],
  ref: { ...SHOWN[0].ref, id: 'started-in-a-terminal' },
  firstPrompt: 'Started in a terminal',
  title: null,
  parentSession: null,
  updatedAt: new Date().toISOString(),
};

const sessionButtons = () => [...document.querySelectorAll<HTMLElement>('[data-session]')].map((button) => button.dataset.session);

/** The backend as it is, except for what `answer` makes of the sessions list. */
function listing(answer: (sessions: SessionSummary[]) => SessionSummary[] | Promise<never>): void {
  const real = ipc.invoke.bind(ipc);

  vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) => {
    const reply = real(name as never, args as never);

    return name === 'sessions_list' ? reply.then((sessions) => answer(sessions as SessionSummary[])) : reply;
  }) as typeof ipc.invoke);
}

describe('Sessions keeps itself current', () => {
  it('lists a session started elsewhere without anyone asking, and keeps the selection', async () => {
    let started = false;
    listing((sessions) => (started ? [STARTED_ELSEWHERE, ...sessions] : sessions));
    viewState.sessionId = SHOWN[1].ref.id;
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    nav.go('sessions');
    render(SessionsView);
    await waitFor(() => expect(sessionButtons()).toContain(SHOWN[1].ref.id));
    expect(sessionButtons()).not.toContain(STARTED_ELSEWHERE.ref.id);

    started = true;
    await vi.advanceTimersByTimeAsync(SESSIONS_POLL_MS);

    await waitFor(() => expect(sessionButtons()).toContain(STARTED_ELSEWHERE.ref.id));
    expect(viewState.sessionId).toBe(SHOWN[1].ref.id);
    expect(document.querySelector('[data-session][aria-current="true"]')?.getAttribute('data-session')).toBe(SHOWN[1].ref.id);
  });

  it('does not take the transcript away from the session on screen when a newer one turns up', async () => {
    let started = false;
    listing((sessions) => (started ? [STARTED_ELSEWHERE, ...sessions] : sessions));
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    nav.go('sessions');
    render(SessionsView);
    await waitFor(() => expect(screen.getByRole('list', { name: 'Session transcript' })).toBeInTheDocument());
    const shown = document.querySelector('[data-session][aria-current="true"]')?.getAttribute('data-session');
    expect(shown).toBe(SHOWN[0].ref.id);

    started = true;
    await vi.advanceTimersByTimeAsync(SESSIONS_POLL_MS);

    await waitFor(() => expect(sessionButtons()).toContain(STARTED_ELSEWHERE.ref.id));
    expect(document.querySelector('[data-session][aria-current="true"]')?.getAttribute('data-session')).toBe(shown);
  });

  it('shows no loading state and no message while it looks again', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    nav.go('sessions');
    render(SessionsView);
    await waitFor(() => expect(screen.getByRole('list', { name: 'Session transcript' })).toBeInTheDocument());

    await vi.advanceTimersByTimeAsync(SESSIONS_POLL_MS);

    expect(screen.queryByRole('status', { name: 'Loading sessions' })).toBeNull();
    expect(screen.queryByRole('alert')).toBeNull();
    expect(sessionButtons()).toHaveLength(SHOWN.length);
  });

  it('tries a list that failed to load again by itself, a little later each time', async () => {
    let failing = true;
    listing((sessions) => (failing ? Promise.reject({ code: 'io', message: 'unreadable' }) : sessions));
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    nav.go('sessions');
    render(SessionsView);
    await vi.advanceTimersByTimeAsync(100);
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();

    // Still failing at the first retry: the message stays, with no flash of the loading state.
    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[0]);
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
    expect(screen.queryByRole('status', { name: 'Loading sessions' })).toBeNull();

    failing = false;
    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[1] + 100);

    expect(sessionButtons()).toHaveLength(SHOWN.length);
    expect(screen.queryByRole('button', { name: 'Try again' })).toBeNull();
  });
});

describe('Sessions that cannot be read', () => {
  it('has one rhythm, the retry schedule, not its poll with a retry on top', async () => {
    let reads = 0;
    listing(() => {
      reads += 1;

      return Promise.reject({ code: 'io', message: 'unreadable' });
    });
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval'] });
    nav.go('sessions');
    render(SessionsView);
    await vi.advanceTimersByTimeAsync(100);
    expect(reads).toBe(1);

    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[0] + RETRY_BACKOFF_MS[1] + 100);

    // The first read, then one per step of the schedule. The five-second poll adds none.
    expect(reads).toBe(3);
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });
});

describe('the transcript on screen', () => {
  const page = fixtures.sessions.messages as SessionMessagesPage;
  const ref = SHOWN[0].ref;

  async function follow(pageNow: () => SessionMessagesPage) {
    const detail = new SessionDetail(
      fakeTransport({
        session_messages: () => structuredClone(pageNow()),
        session_files_touched: () => [],
      }),
    );
    detail.open(ref, PROJECT);
    await vi.waitFor(() => expect(detail.transcriptStatus).toBe('ready'));

    return detail;
  }

  it('is not redrawn when a look finds nothing new', async () => {
    const detail = await follow(() => page);
    const shown = detail.messages;

    await detail.follow();

    expect(detail.messages).toBe(shown);
  });

  it('grows when the session was written to meanwhile', async () => {
    let written = page;
    const detail = await follow(() => written);
    const before = detail.messages.length;

    written = { ...page, total: page.total + 1, messages: [...page.messages, structuredClone(page.messages[0])] };
    await detail.follow();

    expect(detail.messages).toHaveLength(before + 1);
    expect(detail.total).toBe(page.total + 1);
  });
});
