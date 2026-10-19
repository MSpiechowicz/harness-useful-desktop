import { render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { routines } from '../routines/routines-data.svelte';
import Shell from './Shell.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp', onboardingCompleted: true });
  await harness.load();
  await project.pick();
});

afterEach(async () => {
  for (const session of [...agentSessions.sessions]) {
    await agentSessions.stop(session.key);
  }

  nav.go('chat');
});

describe('a click on a "needs you" notification', () => {
  it('opens the chat of the run as a tab, wherever the user is, and answers the request once', async () => {
    render(Shell);
    nav.go('hooks');
    await waitFor(() => expect(routines.running.size).toBeGreaterThan(0));
    const run = [...routines.running.values()].find((candidate) => candidate.needsYou)!;

    routines.openRequest = run.id;

    await waitFor(() => expect(agentSessions.sessions.map((session) => session.routine?.runId)).toEqual([run.id]));
    expect(nav.current).toBe('chat');
    expect(agentSessions.active?.routine?.name).toBe(run.routineName);
    expect(routines.openRequest).toBeNull();
    // The chat's header wears the routine.
    expect(await screen.findByRole('button', { name: `Routine · ${run.routineName}` })).toBeInTheDocument();
  });
});
