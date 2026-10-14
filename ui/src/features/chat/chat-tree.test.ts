import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';

import { chatSession } from '../../../tests/chat-helpers';
import ChatTree from './ChatTree.svelte';

beforeEach(() => {
  harness.summaries = fixtures.harnesses;
});

afterEach(() => {
  agentSessions.sessions = [];
  agentSessions.activeKey = null;
  nav.go('chat');
});

function open(count = 2, labelled = true) {
  const all = Array.from({ length: count }, (_, index) => chatSession([], { key: `chat-${index}` }).session);
  if (labelled) {
    all.forEach((session, index) => (session.aiLabel = { name: `Chat ${'ab'[index]}`, description: '' }));
  }

  agentSessions.sessions = all;
  agentSessions.activeKey = all[0].key;
  render(ChatTree);

  return all;
}

describe('chat tree', () => {
  it('lists every open chat as a card named with a generated creature, not a number', () => {
    const [first] = open(2, false);

    const group = screen.getByRole('group', { name: 'Chats' });
    expect(group.querySelectorAll('button[aria-current], button[title]').length).toBeGreaterThanOrEqual(2);
    expect(agentSessions.title(first.key)).not.toMatch(/\d/);
  });

  it('shows a chat under the name of its first message, with the rest as its description', () => {
    const [first] = open(1, false);
    const prompt = 'Please refactor the settings page so the toggles share one component and add tests for the new switch';
    first.chat = {
      ...first.chat,
      items: [{ kind: 'message', key: 'u1', order: 0, role: 'user', blocks: [{ type: 'text', text: prompt }], streaming: false, slots: {}, meta: null }],
    };

    expect(agentSessions.title(first.key)).toBe('Please refactor the settings page so…');
    expect(agentSessions.description(first.key)).toContain('add tests for the new switch');
  });

  it('shows the chat on click and goes to the chat page', async () => {
    const [, second] = open();
    nav.go('settings');

    await fireEvent.click(screen.getAllByRole('button').find((button) => button.title.includes(agentSessions.title(second.key)))!);

    expect(agentSessions.activeKey).toBe(second.key);
    expect(nav.current).toBe('chat');
  });

  it('renames a chat on double-click and keeps the name the user typed over every other', async () => {
    const [first] = open();
    const rename = vi.spyOn(first, 'rename').mockResolvedValue();

    await fireEvent.dblClick(screen.getAllByRole('button').find((button) => button.title.includes(agentSessions.title(first.key)))!);
    const field = screen.getByRole('textbox', { name: 'Chat name' });
    await fireEvent.input(field, { target: { value: 'Fix the login bug' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    expect(rename).toHaveBeenCalledWith('Fix the login bug');
    expect(screen.queryByRole('textbox', { name: 'Chat name' })).toBeNull();
  });

  it('closes an idle chat at once', async () => {
    const [first] = open();
    const stop = vi.spyOn(agentSessions, 'stop').mockResolvedValue();

    await fireEvent.click(screen.getAllByRole('button', { name: 'Close chat' })[0]);

    expect(stop).toHaveBeenCalledWith(first.key);
  });
});
