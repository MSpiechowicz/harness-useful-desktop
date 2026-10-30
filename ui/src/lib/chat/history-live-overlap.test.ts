import { describe, expect, it } from 'vitest';

import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import type { MessageRole } from '$lib/ipc/bindings/MessageRole';

import { prependHistory } from './messages';
import { applyEvent } from './reducer';
import { initialChatState } from './state';
import type { ChatState } from './types';

function message(role: MessageRole, text: string, timestamp: number | null): ChatMessage {
  return { id: null, role, blocks: [{ type: 'text', text }], model: null, provider: null, stopReason: null, errorMessage: null, usage: null, costUsd: null, timestamp, toolCallId: null, toolName: null, isError: null };
}

function texts(state: ChatState): string[] {
  return state.items.flatMap((item) => (item.kind === 'message' ? item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])) : []));
}

/** A chat that watched the running turn start: its prompt is already on screen. */
function watchingTurn(): ChatState {
  const prompt = message('user', 'Run the tests', 3000);

  return applyEvent(applyEvent(initialChatState('k'), { type: 'messageStart', messageId: 'u', role: 'user' }), { type: 'messageEnd', messageId: 'u', message: prompt });
}

describe('history read while a turn runs', () => {
  it('shows the turn once when the live stream already showed its messages', () => {
    const history = [message('user', 'Earlier', 1000), message('assistant', 'Earlier answer', 2000), message('user', 'Run the tests', 3000)];
    const state = prependHistory(watchingTurn(), history, null, history.length);

    expect(texts(state)).toEqual(['Earlier', 'Earlier answer', 'Run the tests']);
  });

  it('keeps the turn from the history when the chat did not watch it start', () => {
    const history = [message('user', 'Earlier', 1000), message('user', 'Run the tests', 3000)];
    const state = prependHistory(initialChatState('k'), history, null, history.length);

    expect(texts(state)).toEqual(['Earlier', 'Run the tests']);
  });

  it('never merges messages that only share a time or have none', () => {
    const history = [message('assistant', 'Same time other role', 3000), message('user', 'No time', null), message('user', 'No time either', null)];
    const state = prependHistory(watchingTurn(), history, null, history.length);

    expect(texts(state)).toEqual(['Same time other role', 'No time', 'No time either', 'Run the tests']);
  });

  it('shows a message once when the live one carries its history entry id but not its time', () => {
    // Claude Code: the app shows the prompt it sent at once, the transcript stamps it a moment later
    const live = { ...message('user', 'Run the tests', 3000), id: 'entry-1' };
    const state = applyEvent(applyEvent(initialChatState('k'), { type: 'messageStart', messageId: 'entry-1', role: 'user' }), { type: 'messageEnd', messageId: 'entry-1', message: live });
    const history = [message('user', 'Earlier', 1000), { ...message('user', 'Run the tests', 3042), id: 'entry-1' }];

    expect(texts(prependHistory(state, history, null, history.length))).toEqual(['Earlier', 'Run the tests']);
  });
});
