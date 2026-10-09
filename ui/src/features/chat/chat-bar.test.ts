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
  it('puts the chat tabs and both actions in one header that still names the page', () => {
    const { session } = chatSession();
    agentSessions.sessions = [session];
    agentSessions.activeKey = session.key;

    render(ChatBar);

    const header = screen.getByRole('banner');
    expect(within(header).getByRole('heading', { level: 1, name: 'Chat' })).toBeInTheDocument();
    expect(within(header).getByRole('tablist', { name: 'Chats' })).toBeInTheDocument();
    expect(within(header).getByRole('button', { name: /View changes/ })).toBeInTheDocument();
    expect(within(header).getByRole('button', { name: /New chat/ })).toBeInTheDocument();
  });

  it('shows no tab strip before the first chat, but keeps the page name and the actions', () => {
    render(ChatBar);

    expect(screen.getByRole('heading', { level: 1, name: 'Chat' })).toBeInTheDocument();
    expect(screen.queryByRole('tablist')).toBeNull();
    expect(screen.getByRole('button', { name: /New chat/ })).toBeInTheDocument();
  });

  it('cannot start a chat or open changes without a project', () => {
    const open = project.active;
    project.active = null;

    render(ChatBar);

    expect(screen.getByRole('button', { name: /New chat/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /View changes/ })).toBeDisabled();

    project.active = open;
  });
});
