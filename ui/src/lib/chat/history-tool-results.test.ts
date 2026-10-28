import { describe, expect, it } from 'vitest';

import type { ChatBlock } from '$lib/ipc/bindings/ChatBlock';
import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import type { MessageRole } from '$lib/ipc/bindings/MessageRole';

import { prependHistory } from './messages';
import { initialChatState } from './state';

function message(role: MessageRole, blocks: ChatBlock[], extra: Partial<ChatMessage> = {}): ChatMessage {
  return { id: null, role, blocks, model: null, provider: null, stopReason: null, errorMessage: null, usage: null, costUsd: null, timestamp: null, toolCallId: null, toolName: null, isError: null, ...extra };
}

function call(id: string, name: string): ChatBlock {
  return { type: 'toolCall', id, name, args: { query: id }, intent: null };
}

function result(id: string, name: string, text: string): ChatMessage {
  return message('toolResult', [{ type: 'text', text }], { toolCallId: id, toolName: name, isError: false });
}

const SEARCH = 'mcp__harness-memory__note_search';

/** A reply with parallel calls, each answered once, as a reloaded Claude Code session lists them. */
const HISTORY: ChatMessage[] = [
  message('user', [{ type: 'text', text: 'Find the notes' }]),
  message('assistant', [call('b', 'Bash'), call('s1', SEARCH), call('s2', SEARCH)]),
  result('b', 'Bash', 'Already up to date.'),
  result('s1', SEARCH, 'no matching notes'),
  result('s2', SEARCH, '#12 Hero chips'),
  message('assistant', [{ type: 'text', text: 'Done.' }]),
];

function expectAnswered(calls: ReturnType<typeof initialChatState>['toolCalls']): void {
  const answers = Object.values(calls).map((entry) => [entry.id, entry.status, entry.resultText]);

  expect(answers.sort(([a], [b]) => String(a).localeCompare(String(b)))).toEqual([
    ['b', 'ok', 'Already up to date.'],
    ['s1', 'ok', 'no matching notes'],
    ['s2', 'ok', '#12 Hero chips'],
  ]);
}

describe('history tool results', () => {
  it('answers every call of a reloaded transcript', () => {
    const state = prependHistory(initialChatState('k'), HISTORY, null, HISTORY.length);

    expectAnswered(state.toolCalls);
  });

  it('keeps the results when a page boundary separates calls from their results', () => {
    // The newest page comes first: its results arrive before the calls they answer.
    const newest = prependHistory(initialChatState('k'), HISTORY.slice(3), '3', HISTORY.length);
    const state = prependHistory(newest, HISTORY.slice(0, 3), null, HISTORY.length);

    expectAnswered(state.toolCalls);
  });
});
