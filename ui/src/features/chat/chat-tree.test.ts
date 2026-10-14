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
    expect(group.querySelectorAll('button[aria-label="Close chat"]')).toHaveLength(2);
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

    await fireEvent.click(screen.getByText(agentSessions.title(second.key)).closest('button')!);

    expect(agentSessions.activeKey).toBe(second.key);
    expect(nav.current).toBe('chat');
  });

  it('renames a chat on double-click and keeps the name the user typed over every other', async () => {
    const [first] = open();
    const rename = vi.spyOn(first, 'rename').mockResolvedValue();

    await fireEvent.dblClick(screen.getByText(agentSessions.title(first.key)).closest('button')!);
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

describe('chat tooltip', () => {
  it('is the app\'s own tooltip with the full name, the harness and what the chat is about, and no rename hint', async () => {
    const [first] = open(1, false);
    first.chat = {
      ...first.chat,
      items: [{ kind: 'message', key: 'u1', order: 0, role: 'user', blocks: [{ type: 'text', text: 'Please refactor the settings page so the toggles share one component and add tests for the new switch' }], streaming: false, slots: {}, meta: null }],
    };

    const row = (await screen.findByText(agentSessions.title(first.key))).closest('span[role="presentation"]')!;
    await fireEvent.pointerEnter(row);
    const tip = await screen.findByRole('tooltip');

    // The message once, in full: not its cut-short name with the rest of the sentence under it.
    expect(tip).toHaveTextContent('Please refactor the settings page so the toggles share one component and add tests for the new switch');
    expect(tip.textContent?.match(/Please refactor/g)).toHaveLength(1);
    expect(tip).toHaveTextContent('Oh My Pi');
    expect(tip).not.toHaveTextContent(/rename/i);
    expect(screen.getAllByText(agentSessions.title(first.key))[0].closest('button')?.hasAttribute('title')).toBe(false);
  });

  it('keeps a helper-written name and its description apart', () => {
    const [first] = open(1, false);
    first.aiLabel = { name: 'Dependency check', description: 'Which packages are outdated.' };

    expect(agentSessions.tip(first.key)).toEqual({ text: 'Dependency check', detail: 'Which packages are outdated.' });
  });
});
