import { describe, expect, it } from 'vitest';

import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';
import { fixtures, type StreamName } from '$lib/ipc/mock/fixtures';

import { pendingApproval, pendingDialog } from './approval';
import { prependHistory } from './messages';
import { applyEvent, reduceEnvelope } from './reducer';
import { initialChatState } from './state';
import type { ChatState, MessageItem } from './types';

const KEY = 'session-1';

function envelopes(events: AgentEvent[]): AgentEnvelope[] {
  return events.map((event, index) => ({ sessionKey: KEY, seq: index + 1, event }));
}

function reduceAll(state: ChatState, list: AgentEnvelope[]): ChatState {
  return list.reduce(reduceEnvelope, state);
}

/** A recorded scenario replayed into a fresh session, optionally cut off after `count` events. */
function replay(name: StreamName, count?: number): ChatState {
  const events = fixtures.agent.streams[name].map((envelope) => envelope.event).slice(0, count);

  return reduceAll(initialChatState(KEY), envelopes(events));
}

function messages(state: ChatState): MessageItem[] {
  return state.items.filter((item): item is MessageItem => item.kind === 'message');
}

function textOf(item: MessageItem): string {
  return item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('');
}

describe('reducer over the basic scenario', () => {
  const state = replay('basic');

  it('ends settled and idle, with the model and context from the state event', () => {
    expect(state.phase).toBe('idle');
    expect(state.settledCount).toBe(1);
    expect(state.session?.model?.id).toBeTruthy();
    expect(state.promptError).toBeNull();
  });

  it('keeps the user message and the authoritative assistant message in order', () => {
    const [user, assistant] = messages(state);

    expect(user.role).toBe('user');
    expect(assistant.role).toBe('assistant');
    expect(assistant.streaming).toBe(false);
    expect(textOf(assistant)).toContain('Return `Option<Config>`');
    expect(assistant.blocks.some((block) => block.type === 'thinking')).toBe(true);
    expect(state.lastUserText).toBe(textOf(user));
  });

  it('shows each tool call as one card inside the assistant message that made it', () => {
    const [, assistant] = messages(state);
    const cards = assistant.blocks.filter((block) => block.type === 'tool');
    const calls = Object.values(state.toolCalls);

    expect(cards).toHaveLength(calls.length);
    expect(calls.every((call) => call.status === 'ok')).toBe(true);
    expect(state.items.some((item) => item.kind === 'tool')).toBe(false);
  });

  it('records touched paths and the readable result of tool calls', () => {
    const edit = Object.values(state.toolCalls).find((call) => call.name === 'edit');

    expect(edit?.touchedPaths).toEqual(['src/lib.rs']);
    expect(edit?.resultText).toContain('Updated src/lib.rs');
  });
});

describe('streaming messages', () => {
  it('concatenates deltas per content index in arrival order', () => {
    const events: AgentEvent[] = [
      { type: 'messageStart', messageId: 'a', role: 'assistant' },
      { type: 'thinkingDelta', messageId: 'a', contentIndex: 0, delta: 'think ' },
      { type: 'textDelta', messageId: 'a', contentIndex: 1, delta: 'Hel' },
      { type: 'thinkingDelta', messageId: 'a', contentIndex: 0, delta: 'hard' },
      { type: 'textDelta', messageId: 'a', contentIndex: 1, delta: 'lo ' },
      { type: 'textDelta', messageId: 'a', contentIndex: 1, delta: 'world' },
    ];

    const [message] = messages(reduceAll(initialChatState(KEY), envelopes(events)));

    expect(message.streaming).toBe(true);
    expect(message.blocks).toEqual([
      { type: 'thinking', text: 'think hard', redacted: false },
      { type: 'text', text: 'Hello world' },
    ]);
  });

  it('replaces the streamed buffers with the authoritative message on messageEnd', () => {
    const final = fixtures.agent.streams.basic.find((envelope) => envelope.event.type === 'messageEnd' && envelope.event.message.role === 'assistant');
    if (final?.event.type !== 'messageEnd') {
      throw new Error('fixture has no assistant message');
    }

    const streamed = envelopes([
      { type: 'messageStart', messageId: final.event.messageId, role: 'assistant' },
      { type: 'textDelta', messageId: final.event.messageId, contentIndex: 0, delta: 'partial draft' },
      final.event,
    ]);
    const [message] = messages(reduceAll(initialChatState(KEY), streamed));

    expect(message.streaming).toBe(false);
    expect(textOf(message)).not.toContain('partial draft');
    expect(message.meta?.model).toBe(final.event.message.model);
  });

  it('adds a tool card to the message being streamed, and a free-standing one when none is', () => {
    const during = reduceAll(
      initialChatState(KEY),
      envelopes([
        { type: 'messageStart', messageId: 'a', role: 'assistant' },
        { type: 'toolStart', toolCallId: 't1', name: 'read', args: { path: 'a.rs' }, intent: null },
      ]),
    );
    const after = applyEvent(applyEvent(initialChatState(KEY), { type: 'toolStart', toolCallId: 't2', name: 'read', args: {}, intent: null }), {
      type: 'toolEnd',
      toolCallId: 't2',
      name: 'read',
      isError: false,
      result: 'x',
      touchedPaths: [],
    });

    expect(messages(during)[0].blocks).toEqual([{ type: 'tool', toolCallId: 't1' }]);
    expect(after.items.map((item) => item.kind)).toEqual(['tool']);
    expect(after.toolCalls.t2.status).toBe('ok');
  });
});

describe('sequence numbers', () => {
  it('ignores an envelope it has already applied', () => {
    const list = envelopes([{ type: 'agentStart' }, { type: 'notice', level: 'info', message: 'hi' }]);
    const once = reduceAll(initialChatState(KEY), list);
    const twice = reduceAll(once, list);

    expect(twice).toBe(once);
    expect(twice.items).toHaveLength(1);
  });

  it('counts a gap but still applies what arrived', () => {
    const state = reduceAll(initialChatState(KEY), [
      { sessionKey: KEY, seq: 1, event: { type: 'agentStart' } },
      { sessionKey: KEY, seq: 4, event: { type: 'notice', level: 'warning', message: 'late' } },
    ]);

    expect(state.gaps).toBe(1);
    expect(state.items).toHaveLength(1);
  });

  it('ignores envelopes of another session', () => {
    const state = reduceEnvelope(initialChatState(KEY), { sessionKey: 'other', seq: 1, event: { type: 'agentStart' } });

    expect(state.phase).toBe('starting');
  });
});

describe('approval scenario', () => {
  it('stops at the approval request with the tool still running', () => {
    const uiIndex = fixtures.agent.streams.approval.findIndex((envelope) => envelope.event.type === 'uiRequest');
    const state = replay('approval', uiIndex + 1);
    const approval = pendingApproval(state);

    expect(approval?.approve).toBe('Approve');
    expect(approval?.deny).toBe('Deny');
    expect(pendingDialog(state)).toBeNull();
    expect(Object.values(state.toolCalls)[0].status).toBe('running');
    expect(state.phase).toBe('streaming');
  });

  it('finishes with the command output after approval', () => {
    const state = replay('approval');

    expect(Object.values(state.toolCalls)[0]).toMatchObject({ status: 'ok', name: 'bash' });
    expect(state.openRequests).toHaveLength(0);
  });

  it('ends aborted with a failed tool call when the request is denied', () => {
    const state = replay('approvalDenied');

    expect(Object.values(state.toolCalls)[0].status).toBe('error');
    expect(state.promptError).toBeNull();
    expect(state.phase).toBe('idle');
  });

  it('drops a request the agent cancels', () => {
    const uiIndex = fixtures.agent.streams.approval.findIndex((envelope) => envelope.event.type === 'uiRequest');
    const open = replay('approval', uiIndex + 1);
    const cancelled = applyEvent(open, { type: 'uiCancel', requestId: open.openRequests[0].id });

    expect(cancelled.openRequests).toHaveLength(0);
  });
});

describe('failure scenarios', () => {
  it('raises a retryable prompt error and a retry banner', () => {
    const state = replay('error');

    expect(state.promptError).toEqual({ message: expect.stringContaining('429'), retryable: true });
    expect(state.items.some((item) => item.kind === 'retry' && item.phase === 'failed')).toBe(true);
    expect(state.items.some((item) => item.kind === 'notice')).toBe(true);
  });

  it('clears the error once a later turn completes', () => {
    const state = applyEvent(replay('error'), { type: 'promptResult', requestId: null, status: 'completed', error: null, retryable: false });

    expect(state.promptError).toBeNull();
  });

  it('turns an exit into a crash with the stderr tail, and interrupts running tools', () => {
    const running = applyEvent(replay('crash', 5), { type: 'toolStart', toolCallId: 'x', name: 'bash', args: {}, intent: null });
    const state = reduceAll({ ...running, lastSeq: 0 }, [{ sessionKey: running.key, seq: 1, event: { type: 'exited', code: 1, signal: null } }]);
    const crashed = replay('crash');

    expect(crashed.phase).toBe('exited');
    expect(crashed.crash?.code).toBe(1);
    expect(crashed.crash?.stderr.length).toBeGreaterThan(0);
    expect(state.toolCalls.x.status).toBe('error');
  });
});

describe('agent UI requests and extras', () => {
  const state = replay('uiRequests');

  it('keeps every unanswered request, oldest first', () => {
    expect(state.openRequests.map((request) => request.kind.type)).toEqual(['confirm', 'input', 'editor']);
    expect(pendingDialog(state)?.kind.type).toBe('confirm');
  });

  it('collects status, widgets, title, notices, editor text and links', () => {
    expect(state.title).toBe('Project setup');
    expect(state.statuses).toEqual({});
    expect(Object.values(state.widgets)[0].lines).toContain('Step 3 of 3');
    expect(state.editorText?.text).toBe('/review src/lib.rs');
    expect(state.items.map((item) => item.kind)).toEqual(expect.arrayContaining(['notice', 'url']));
  });

  it('asks for new editor text every time, even for the same text', () => {
    const again = applyEvent(state, { type: 'editorText', text: '/review src/lib.rs' });

    expect(again.editorText?.nonce).toBe((state.editorText?.nonce ?? 0) + 1);
  });
});

describe('history', () => {
  it('puts older messages in front and folds tool results into their cards', () => {
    const history = fixtures.agent.history;
    const live = applyEvent(initialChatState(KEY), { type: 'notice', level: 'info', message: 'live' });
    const state = prependHistory(live, history.messages, history.nextCursor, history.total);

    expect(state.items[state.items.length - 1].kind).toBe('notice');
    expect(messages(state).map((item) => item.role)).toEqual(['user', 'assistant', 'assistant']);
    expect(state.toolCalls['call-h1'].resultText).toContain('edition');
    expect(state.historyCursor).toBe(history.nextCursor);
    expect(state.items.filter((item) => item.kind === 'message').every((item) => item.order === -1)).toBe(true);
  });
});
