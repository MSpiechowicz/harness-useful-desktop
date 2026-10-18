import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { restoreClientWidth, stubClientWidth } from '../../../tests/helpers';
import { sessionTitle } from './session-format';
import SessionsView from './SessionsView.svelte';

const PROJECT = fixtures.recentProjects[0].path;
const SESSIONS = fixtures.sessions.list.omp.filter((summary) => !summary.emptyCopy);

const WIDE = 1700;
const MIDDLE = 1400;
const NARROW = 1000;

const DETAILS_KEY = 'harness.workspace.sessions.details';
const LIST_KEY = 'harness.workspace.sessions.list';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  // happy-dom lays nothing out: give every element a 640px viewport so virtual lists render a window of rows.
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 640 });

  await prefs.load();
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  await project.open(PROJECT);
});

beforeEach(() => {
  window.localStorage.clear();
  viewState.sessionId = null;
});

afterEach(async () => {
  for (const session of [...agentSessions.sessions]) {
    await agentSessions.stop(session.key);
  }

  git.activate(null);
  nav.go('chat');
  vi.restoreAllMocks();
  restoreClientWidth();
});

async function showSessions(width: number) {
  stubClientWidth(width);
  nav.go('sessions');
  const view = render(SessionsView);
  await waitFor(() => expect(screen.getByRole('list', { name: 'Session transcript' })).toBeInTheDocument());
  // The toggles show once the view is laid out.
  await screen.findByRole('button', { name: 'Details' });

  return view;
}

const listToggle = () => screen.getByRole('button', { name: 'Sessions' });
const detailsToggle = () => screen.getByRole('button', { name: 'Details' });
const sessionButton = (id: string) => document.querySelector<HTMLElement>(`[data-session="${id}"]`) as HTMLElement;

describe('a wide sessions view', () => {
  it('lays out the list, the transcript and the details, with both toggles pressed after Resume and before the menu', async () => {
    await showSessions(WIDE);

    expect(sessionButton(SESSIONS[0].ref.id)).toBeInTheDocument();
    expect(screen.getByRole('list', { name: 'Session transcript' })).toBeInTheDocument();
    expect(within(screen.getByRole('region', { name: 'Details' })).getByText('Overview')).toBeInTheDocument();
    expect(listToggle()).toHaveAttribute('aria-pressed', 'true');
    expect(detailsToggle()).toHaveAttribute('aria-pressed', 'true');

    const order = [screen.getByRole('button', { name: 'Resume in chat' }), listToggle(), detailsToggle(), screen.getByRole('button', { name: 'More actions' })];
    for (const [index, button] of order.slice(1).entries()) {
      expect(order[index].compareDocumentPosition(button) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    }
  });

  it('names the shown session under the Details heading and has no Details button in the session header', async () => {
    await showSessions(WIDE);

    expect(within(screen.getByRole('region', { name: 'Details' })).getByText(sessionTitle(SESSIONS[0]))).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Details' })).toHaveLength(1);
  });

  it('hides the details from their toggle, remembers it, and keeps the transcript in place', async () => {
    await showSessions(WIDE);
    const transcript = screen.getByRole('list', { name: 'Session transcript' });

    await fireEvent.click(detailsToggle());

    expect(detailsToggle()).toHaveAttribute('aria-pressed', 'false');
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
    expect(window.localStorage.getItem(DETAILS_KEY)).toBe('false');
    expect(screen.getByRole('list', { name: 'Session transcript' })).toBe(transcript);
  });

  it('hides the list from its toggle and comes back hidden when the view is shown again', async () => {
    const view = await showSessions(WIDE);

    await fireEvent.click(listToggle());

    expect(listToggle()).toHaveAttribute('aria-pressed', 'false');
    expect(sessionButton(SESSIONS[0].ref.id).closest('[inert]')).not.toBeNull();
    expect(window.localStorage.getItem(LIST_KEY)).toBe('false');

    view.unmount();
    await showSessions(WIDE);

    expect(listToggle()).toHaveAttribute('aria-pressed', 'false');
    expect(sessionButton(SESSIONS[0].ref.id).closest('[inert]')).not.toBeNull();
  });
});

describe('a middle-width sessions view', () => {
  it('slides the details over the transcript from the toggle, and Escape puts them away with focus on the toggle', async () => {
    await showSessions(MIDDLE);
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
    expect(detailsToggle()).toHaveAttribute('aria-pressed', 'false');
    expect(sessionButton(SESSIONS[0].ref.id)).toBeInTheDocument();

    detailsToggle().focus();
    await fireEvent.click(detailsToggle());

    const details = await screen.findByRole('region', { name: 'Details' });
    expect(within(details).getByText('Overview')).toBeInTheDocument();
    expect(detailsToggle()).toHaveAttribute('aria-pressed', 'true');

    within(details).getByRole('button', { name: 'Close' }).focus();
    await fireEvent.keyDown(details, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('region', { name: 'Details' })).toBeNull());
    expect(document.activeElement).toBe(detailsToggle());
    expect(window.localStorage.getItem(DETAILS_KEY)).toBeNull();
  });
});

describe('a narrow sessions view', () => {
  it('keeps the list in a drawer: its toggle opens it, choosing a session closes it, and Escape puts it away with focus on the toggle', async () => {
    await showSessions(NARROW);
    expect(listToggle()).toHaveAttribute('aria-pressed', 'false');
    expect(sessionButton(SESSIONS[0].ref.id)).toBeNull();

    listToggle().focus();
    await fireEvent.click(listToggle());

    const drawer = await screen.findByRole('dialog', { name: 'Sessions' });
    expect(listToggle()).toHaveAttribute('aria-pressed', 'true');

    await fireEvent.keyDown(drawer, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Sessions' })).toBeNull());
    expect(document.activeElement).toBe(listToggle());

    await fireEvent.click(listToggle());
    await screen.findByRole('dialog', { name: 'Sessions' });
    await fireEvent.click(sessionButton(SESSIONS[1].ref.id));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Sessions' })).toBeNull());
    expect(await screen.findByRole('heading', { level: 2, name: sessionTitle(SESSIONS[1]) })).toBeInTheDocument();
    expect(window.localStorage.getItem(LIST_KEY)).toBeNull();
  });
});
