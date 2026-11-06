import { describe, expect, it } from 'vitest';

import { approvalCalls, asApproval } from './approval';
import type { UiRequest } from '$lib/ipc/bindings/UiRequest';
import type { ChatState, ToolCallState } from './types';

const call = (id: string, status: ToolCallState['status'], name = 'bash') => ({ id, name, status }) as ToolCallState;
const chat = (...calls: ToolCallState[]) =>
  ({ toolCalls: Object.fromEntries(calls.map((one) => [one.id, one])) }) as unknown as ChatState;
const prompt = (over: Partial<{ toolCallId: string | null; marked: boolean; title: string }> = {}) => ({
  toolCallId: null,
  marked: false,
  title: 'Allow tool: bash',
  ...over,
});
const ids = (calls: ToolCallState[]) => calls.map((one) => one.id);

describe('approvalCalls', () => {
  it('takes the only open call when the agent names none', () => {
    expect(ids(approvalCalls(chat(call('a', 'ok'), call('b', 'running')), prompt()))).toEqual(['b']);
  });

  it('lists parallel calls of the tool the title names and never picks one', () => {
    const state = chat(call('a', 'running'), call('b', 'pending'), call('c', 'running', 'read'));

    expect(ids(approvalCalls(state, prompt()))).toEqual(['a', 'b']);
  });

  it('settles on one call when the title names its tool', () => {
    const state = chat(call('a', 'running', 'github'), call('b', 'running'));

    expect(ids(approvalCalls(state, prompt({ title: 'Allow tool: github' })))).toEqual(['a']);
  });

  it('shows no call of another tool as the subject', () => {
    expect(approvalCalls(chat(call('a', 'running', 'read')), prompt())).toEqual([]);
  });

  it('does not tie a prompt that names no tool to one of several calls', () => {
    expect(approvalCalls(chat(call('a', 'running'), call('b', 'running')), prompt({ title: 'Reset saved work?' }))).toEqual([]);
  });

  it('takes the call the agent names, also among parallel ones', () => {
    const state = chat(call('a', 'running'), call('b', 'running'));

    expect(ids(approvalCalls(state, prompt({ toolCallId: 'a' })))).toEqual(['a']);
    expect(approvalCalls(state, prompt({ toolCallId: 'gone' }))).toEqual([]);
  });

  it('ties a subagent approval to no call of the main chat', () => {
    expect(approvalCalls(chat(call('a', 'running')), prompt({ marked: true }))).toEqual([]);
  });
});

describe('asApproval', () => {
  const select = (options: string[]): UiRequest => ({
    id: 'r',
    kind: { type: 'select', title: 'Allow tool: bash', options, details: options.map(() => null), timeoutMs: null },
  });

  it('makes this call only the primary answer beside a grant that remembers more', () => {
    const prompt = asApproval(select(['Approve File', 'Always Approve File', 'Approve Once', 'Deny']));

    expect(prompt?.approve).toBe('Approve Once');
  });

  it('keeps an approval with many grants a card', () => {
    const options = ['Approve File', 'Approve Dir', 'Always File', 'Always Dir', 'Approve Once', 'Deny'];

    expect(asApproval(select(options))?.approve).toBe('Approve Once');
  });
});
