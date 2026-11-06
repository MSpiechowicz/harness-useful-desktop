import { describe, expect, it } from 'vitest';

import { approvalSubject } from './approval';
import type { ChatState, ToolCallState } from './types';

const call = (id: string, status: ToolCallState['status']) => ({ id, name: 'bash', status }) as ToolCallState;
const chat = (...calls: ToolCallState[]) =>
  ({ toolCalls: Object.fromEntries(calls.map((one) => [one.id, one])) }) as unknown as ChatState;

describe('approvalSubject', () => {
  it('takes the only open call when the agent names none', () => {
    expect(approvalSubject(chat(call('a', 'ok'), call('b', 'running')), { toolCallId: null })?.id).toBe('b');
  });

  it('never guesses between parallel calls when the agent names none', () => {
    expect(approvalSubject(chat(call('a', 'running'), call('b', 'pending')), { toolCallId: null })).toBeNull();
  });

  it('takes the call the agent names, also among parallel ones', () => {
    expect(approvalSubject(chat(call('a', 'running'), call('b', 'running')), { toolCallId: 'a' })?.id).toBe('a');
  });
});
