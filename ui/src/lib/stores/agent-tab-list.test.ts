import { describe, expect, it } from 'vitest';

import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';

import { chatSession, STREAMING } from '../../../tests/chat-helpers';
import { ChatTabs } from './agent-tab-list.svelte';

/** A tab list holding one chat whose user has sent `text` as the first message. */
function tabsWith(text: string) {
  const message = { id: 'u1', role: 'user', blocks: [{ type: 'text', text }] } as unknown as ChatMessage;
  const { session } = chatSession([...STREAMING, { type: 'messageEnd', messageId: 'u1', message }]);
  const tabs = new ChatTabs();
  tabs.sessions = [session];

  return { tabs, session };
}

describe('the name a chat was given', () => {
  it('is none while the title is only the opening words of the first message', () => {
    const { tabs, session } = tabsWith('Fix the flaky login test');

    expect(tabs.title(session.key)).toBe('Fix the flaky login test');
    expect(tabs.givenName(session.key)).toBeNull();
  });

  it('is none for a chat that is not open', () => {
    expect(new ChatTabs().givenName('gone')).toBeNull();
  });

  it('is the helper\'s name once it answers, and that is also the title', () => {
    const { tabs, session } = tabsWith('Fix the flaky login test');

    session.aiLabel = { name: 'Login test fix', description: '' };

    expect(tabs.givenName(session.key)).toBe('Login test fix');
    expect(tabs.title(session.key)).toBe('Login test fix');
  });

  it('is the name the user typed, which beats the helper\'s', () => {
    const { tabs, session } = tabsWith('Fix the flaky login test');
    session.aiLabel = { name: 'Login test fix', description: '' };

    session.userNamed = true;
    session.chat.session = { ...(session.chat.session ?? ({} as never)), sessionName: 'My login chat' };

    expect(tabs.givenName(session.key)).toBe('My login chat');
    expect(tabs.title(session.key)).toBe('My login chat');
  });
});
