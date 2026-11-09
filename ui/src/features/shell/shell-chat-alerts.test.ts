import { render, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
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

afterEach(async () => {
  for (const session of [...agentSessions.sessions]) {
    await agentSessions.stop(session.key);
  }

  nav.go('chat');
});

describe('the shell and chat alerts', () => {
  it('listens once however often the shell is mounted, and tells the backend which chat is on screen', async () => {
    const calls: { name: string; args: unknown }[] = [];
    const invoke = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) => {
      calls.push({ name, args });

      return invoke(name as never, args as never);
    }) as never);
    const views = () => calls.filter((call) => call.name === 'chat_alerts_view').map((call) => (call.args as { view: { shown: string | null } }).view);

    await agentSessions.create('omp', project.active!.path);
    const key = agentSessions.activeKey!;

    const first = render(Shell);
    await waitFor(() => expect(views().at(-1)?.shown).toBe(key));
    first.unmount();
    render(Shell);

    nav.go('settings');
    await waitFor(() => expect(views().at(-1)?.shown).toBeNull());
    nav.go('chat');
    await waitFor(() => expect(views().at(-1)?.shown).toBe(key));

    expect(calls.filter((call) => call.name === 'chat_alerts_listen')).toHaveLength(1);
    expect(views().map((view) => view.shown)).toEqual([key, null, key]);
  });
});
