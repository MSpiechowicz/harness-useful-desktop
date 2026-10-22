import { render, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { chatSession } from '../../../tests/chat-helpers';
import { samplePrefs } from '../../../tests/helpers';
import Shell from './Shell.svelte';

const [first, second] = fixtures.recentProjects.map((entry) => entry.path);

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp', onboardingCompleted: true });
  await harness.load();
  await project.pick();
});

afterEach(() => {
  agentSessions.sessions = [];
  agentSessions.activeKey = null;
  nav.go('chat');
});

describe('the open chat decides the project', () => {
  it('brings its project along when the user switches chats, so Changes and its comments follow', async () => {
    const { session: a } = chatSession([], { key: 'chat-a', project: first });
    const { session: b } = chatSession([], { key: 'chat-b', project: second });
    agentSessions.sessions = [a, b];
    agentSessions.activeKey = a.key;

    render(Shell);
    await waitFor(() => expect(project.active?.path).toBe(first));

    agentSessions.select(b.key);
    await waitFor(() => expect(project.active?.path).toBe(second));

    agentSessions.select(a.key);
    await waitFor(() => expect(project.active?.path).toBe(first));
  });

  it('leaves a project chosen by hand alone until the chat changes again', async () => {
    const { session: a } = chatSession([], { key: 'chat-a', project: first });
    agentSessions.sessions = [a];
    agentSessions.activeKey = a.key;

    render(Shell);
    await waitFor(() => expect(project.active?.path).toBe(first));

    await project.open(second);
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(project.active?.path).toBe(second);
  });
});

describe('a toast about a folder to trust', () => {
  it('goes away when another project becomes the active one', async () => {
    const { session: a } = chatSession([], { key: 'chat-a', project: first });
    agentSessions.sessions = [a];
    agentSessions.activeKey = a.key;

    render(Shell);
    await waitFor(() => expect(project.active?.path).toBe(first));
    toasts.push({ title: 'Claude Code does not trust this folder yet', project: first, duration: 0 });
    toasts.push({ title: 'Something else', duration: 0 });

    await project.open(second);

    await waitFor(() => expect(toasts.items.map((item) => item.title)).toEqual(['Something else']));
    toasts.items = [];
  });
});
