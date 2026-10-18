import { render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';

import { chatSession } from '../../../tests/chat-helpers';
import ChatBar from './ChatBar.svelte';

beforeEach(() => {
  harness.summaries = fixtures.harnesses;
});

afterEach(() => {
  agentSessions.sessions = [];
  agentSessions.activeKey = null;
});

describe('chat header bar', () => {
  it('shows the open chat\'s harness, its name and its controls, with no tab strip and no new-chat button', () => {
    const { session } = chatSession([], { configuredApprovalMode: 'yolo', approvalOverride: null });
    const { session: other } = chatSession([], { key: 'second-chat' });
    agentSessions.sessions = [session, other];
    agentSessions.activeKey = session.key;

    render(ChatBar);

    expect(screen.getByRole('heading', { level: 1, name: agentSessions.title(session.key) })).toBeInTheDocument();
    expect(screen.getByText('OMP')).toBeInTheDocument();
    expect(screen.getByText('Approvals: yolo')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /thought process/i })).toBeInTheDocument();
    expect(screen.getByText('Stats')).toBeInTheDocument();
    expect(screen.queryByRole('tablist')).toBeNull();
    expect(screen.queryByRole('button', { name: /New chat/ })).toBeNull();
  });

  it('names the page "Chat" before the first chat, with no controls', () => {
    render(ChatBar);

    expect(screen.getByRole('heading', { level: 1, name: 'Chat' })).toBeInTheDocument();
    expect(screen.queryByText(/Approvals:/)).toBeNull();
  });
});
