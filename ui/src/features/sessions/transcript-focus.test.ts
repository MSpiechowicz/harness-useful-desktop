import { describe, expect, it } from 'vitest';

import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';

import { rowMessage, transcriptRow } from './transcript-focus';

const message = (id: string, role: ChatMessage['role']): ChatMessage => ({ id, role }) as ChatMessage;

const messages = [message('u1', 'user'), message('a1', 'assistant'), message('t1', 'toolResult'), message('a2', 'assistant')];

describe('transcript focus', () => {
  it('maps a message to its row, folding tool results into the row before them', () => {
    expect(transcriptRow(messages, 'u1')).toBe(0);
    expect(transcriptRow(messages, 'a1')).toBe(1);
    expect(transcriptRow(messages, 't1')).toBe(1);
    expect(transcriptRow(messages, 'a2')).toBe(2);
    expect(transcriptRow(messages, 'missing')).toBeNull();
  });

  it('finds the message behind a row, skipping folded tool results', () => {
    expect(rowMessage(messages, 0)?.id).toBe('u1');
    expect(rowMessage(messages, 2)?.id).toBe('a2');
    expect(rowMessage(messages, 3)).toBeNull();
  });
});
