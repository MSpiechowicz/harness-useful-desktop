import { render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

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
  it('has no tab strip: the open chats live in the rail, and the page is named after the open one', () => {
    const { session } = chatSession();
    const { session: other } = chatSession([], { key: 'second-chat' });
    agentSessions.sessions = [session, other];
    agentSessions.activeKey = session.key;

    render(ChatBar);

    const header = screen.getByRole('banner');
    expect(within(header).getByRole('heading', { level: 1, name: agentSessions.title(session.key) })).toBeInTheDocument();
    expect(within(header).queryByRole('tablist')).toBeNull();
    expect(within(header).getByRole('button', { name: /New chat/ })).toBeInTheDocument();
  });

  it('names the page "Chat" before the first chat, and keeps the actions', () => {
    render(ChatBar);

    expect(screen.getByRole('heading', { level: 1, name: 'Chat' })).toBeInTheDocument();
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
