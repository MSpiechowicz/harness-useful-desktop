import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { updates } from '$lib/stores/updates.svelte';

import { chatSession, STREAMING } from '../../../tests/chat-helpers';
import RestartConfirm from './RestartConfirm.svelte';
import UpdateNotice from './UpdateNotice.svelte';

const INFO = { version: '0.2.0', currentVersion: '0.1.0', date: null, notes: null };

let invoke: MockInstance<Transport['invoke']>;

beforeEach(() => {
  invoke = vi.spyOn(ipc, 'invoke').mockResolvedValue(null as never);
  updates.state = { kind: 'readyToRelaunch', info: INFO };
  render(UpdateNotice);
  render(RestartConfirm);
});

afterEach(() => {
  vi.restoreAllMocks();
  updates.state = { kind: 'idle' };
  updates.confirmingRestart = false;
  agentSessions.sessions = [];
  nav.go('chat');
});

const relaunched = () => invoke.mock.calls.filter(([name]) => name === 'app_relaunch').length;

describe('restart after an update', () => {
  it('asks before stopping a chat that is working, and restarts only when told to', async () => {
    agentSessions.sessions = [chatSession(STREAMING).session];

    await fireEvent.click(screen.getByRole('button', { name: 'Restart now' }));

    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveTextContent('1 running chat will be stopped');
    expect(relaunched()).toBe(0);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Keep working' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(relaunched()).toBe(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Restart now' }));
    await fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Restart now' }));

    await waitFor(() => expect(relaunched()).toBe(1));
  });

  it('counts every working chat', async () => {
    agentSessions.sessions = [chatSession(STREAMING).session, chatSession(STREAMING).session, chatSession().session];

    await fireEvent.click(screen.getByRole('button', { name: 'Restart now' }));

    expect(await screen.findByRole('dialog')).toHaveTextContent('2 running chats will be stopped');
  });

  it('restarts at once when nothing is working, even with an idle chat open', async () => {
    agentSessions.sessions = [chatSession().session];

    await fireEvent.click(screen.getByRole('button', { name: 'Restart now' }));

    await waitFor(() => expect(relaunched()).toBe(1));
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
