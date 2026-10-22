import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { sessionList } from '$lib/stores/sessions.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { freshBackend, type Backend } from '../../../tests/ecosystem-backend';
import { samplePrefs, withoutChat } from '../../../tests/helpers';
import { newChat, resumeLastSession, switchProject } from '../shell/actions';
import ChatView from './ChatView.svelte';

let backend: Backend;

/** The harnesses as a machine shows them when only the listed ones are installed. */
function installOnly(...ids: HarnessId[]): void {
  harness.summaries = harness.summaries.map((summary) => ({ ...summary, detection: { ...summary.detection, installed: ids.includes(summary.info.id) } }));
}

beforeEach(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  backend = freshBackend();
  prefs.value = samplePrefs({ motion: 'off', onboardingCompleted: true, defaultHarness: 'omp' });
  toasts.items = [];

  await harness.load();
  // OpenCode stands in for a harness the app cannot chat with.
  withoutChat(harness, 'opencode');
  await project.pick();
});

afterEach(() => {
  agentSessions.sessions = [];
  agentSessions.starting = [];
  agentSessions.activeKey = null;
  localStorage.clear();
  nav.go('chat');
});

describe('chat on a machine with no harness', () => {
  beforeEach(() => installOnly());

  it('names the harnesses it can chat with, as the harness list says', async () => {
    render(ChatView);

    expect(await screen.findByText('Chat needs Oh My Pi, Pi, or Claude Code. Install one, then set it up in Settings.')).toBeInTheDocument();
  });

  it('names a harness as soon as its chat is on', async () => {
    await harness.load();
    installOnly();
    render(ChatView);

    expect(await screen.findByText('Chat needs Oh My Pi, Pi, OpenCode, or Claude Code. Install one, then set it up in Settings.')).toBeInTheDocument();
  });

  it('names no harness while the harness list has none to name', async () => {
    harness.summaries = [];
    harness.status = 'error';
    render(ChatView);

    expect(await screen.findByRole('heading', { name: 'No harness found' })).toBeInTheDocument();
    expect(screen.queryByText(/Chat needs/)).toBeNull();
  });
});

describe('chat on a machine with OpenCode only', () => {
  beforeEach(() => {
    prefs.value = samplePrefs({ motion: 'off', onboardingCompleted: true, defaultHarness: 'opencode' });
    installOnly('opencode');
  });

  it('says chat is coming soon and leads to what works, without starting an agent or showing an error', async () => {
    render(ChatView);

    expect(await screen.findByRole('heading', { name: 'Chat with OpenCode is coming soon' })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: /Sessions/ }));

    expect(nav.current).toBe('sessions');
    expect(backend.calls('agent_start')).toHaveLength(0);
    expect(toasts.items).toHaveLength(0);
  });

  it('starts nothing for a new chat and leaves the explaining to the chat page', async () => {
    await newChat();

    expect(nav.current).toBe('chat');
    expect(backend.calls('agent_start')).toHaveLength(0);
    expect(toasts.items).toHaveLength(0);
  });

  it('starts nothing when the project changes', async () => {
    const other = project.recents.find((recent) => recent.path !== project.active?.path);

    await switchProject(other?.path ?? '/home/demo/Projects/other');

    expect(backend.calls('agent_start')).toHaveLength(0);
  });

  it('keeps the chat page as it was when a harness that can chat is installed again', async () => {
    installOnly('opencode', 'omp');
    render(ChatView);

    await waitFor(() => expect(backend.calls('agent_start')).toHaveLength(1));
    expect(screen.queryByRole('heading', { name: 'Chat with OpenCode is coming soon' })).toBeNull();
  });
});

describe('chats go to a harness that can chat', () => {
  it('starts a new chat with the first harness that can chat when the daily driver cannot', async () => {
    prefs.value = samplePrefs({ motion: 'off', onboardingCompleted: true, defaultHarness: 'opencode' });

    await newChat();

    expect(harness.active?.info.id).toBe('opencode');
    expect(backend.calls('agent_start')).toMatchObject([{ request: { harness: 'omp' } }]);
  });

  it('refuses to launch a harness without chat even when a caller asks, and says why', async () => {
    const started = await agentSessions.start({ harness: 'opencode', project: project.active?.path ?? '/', resume: null, approvalOverride: null, trustOnce: false, utility: false, memoryChat: null });

    expect(started).toBeNull();
    expect(backend.calls('agent_start')).toHaveLength(0);
    expect(toasts.items.find((item) => item.title === 'Chat with OpenCode is coming soon')).toMatchObject({ tone: 'info' });
  });
});

describe('resuming the newest session', () => {
  it('says so instead of starting a chat when that session belongs to a harness without chat', async () => {
    const path = project.active?.path ?? '/';
    await sessionList.refresh();
    sessionList.sync('opencode', path);
    await waitFor(() => expect(sessionList.data?.length).toBeGreaterThan(0));

    await resumeLastSession();

    expect(backend.calls('agent_start')).toHaveLength(0);
    expect(toasts.items[0]).toMatchObject({ title: 'Sessions of OpenCode cannot be resumed yet' });
  });
});
