import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { project } from '$lib/stores/project.svelte';

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
  it('puts the chat tabs and the new-chat action in one header that still names the page', () => {
    const { session } = chatSession();
    const { session: other } = chatSession([], { key: 'second-chat' });
    agentSessions.sessions = [session, other];
    agentSessions.activeKey = session.key;

    render(ChatBar);

    const header = screen.getByRole('banner');
    expect(within(header).getByRole('heading', { level: 1, name: 'Chat' })).toBeInTheDocument();
    expect(within(header).getByRole('tablist', { name: 'Chats' })).toBeInTheDocument();
    expect(within(header).getByRole('button', { name: /New chat/ })).toBeInTheDocument();
  });

  it('renames a chat in its tab on double-click', async () => {
    const { session } = chatSession();
    const { session: other } = chatSession([], { key: 'second-chat' });
    const rename = vi.spyOn(session, 'rename').mockResolvedValue();
    agentSessions.sessions = [session, other];
    agentSessions.activeKey = session.key;

    render(ChatBar);

    await fireEvent.dblClick(screen.getAllByRole('tab')[0]);
    const field = screen.getByRole('textbox', { name: 'Chat name' });
    await fireEvent.input(field, { target: { value: 'Fix the login bug' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    expect(rename).toHaveBeenCalledWith('Fix the login bug');
    expect(screen.queryByRole('textbox', { name: 'Chat name' })).toBeNull();
  });

  it('shows no tab strip for a single chat', () => {
    const { session } = chatSession();
    agentSessions.sessions = [session];
    agentSessions.activeKey = session.key;

    render(ChatBar);

    expect(screen.queryByRole('tablist')).toBeNull();
    expect(screen.getByRole('heading', { level: 1, name: 'Chat' })).toBeInTheDocument();
  });

  it('shows no tab strip before the first chat, but keeps the page name and the actions', () => {
    render(ChatBar);

    expect(screen.getByRole('heading', { level: 1, name: 'Chat' })).toBeInTheDocument();
    expect(screen.queryByRole('tablist')).toBeNull();
    expect(screen.getByRole('button', { name: /New chat/ })).toBeInTheDocument();
  });

  it('cannot start a chat without a project', () => {
    const open = project.active;
    project.active = null;

    render(ChatBar);

    expect(screen.getByRole('button', { name: /New chat/ })).toBeDisabled();

    project.active = open;
  });
});
