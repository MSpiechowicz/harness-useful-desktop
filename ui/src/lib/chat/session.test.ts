import { describe, expect, it } from 'vitest';

import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';

import { chatSession, STREAMING } from '../../../tests/chat-helpers';

function toolEnd(id: string, name: string): AgentEvent {
  return { type: 'toolEnd', toolCallId: id, name, isError: false, result: 'ok', touchedPaths: [] };
}

describe('chat session', () => {
  it('counts finished tool runs, a commit through bash included, so git state can follow the agent', () => {
    const { session } = chatSession([
      ...STREAMING,
      { type: 'toolStart', toolCallId: 't1', name: 'edit', args: { path: 'a.rs' }, intent: null },
      toolEnd('t1', 'edit'),
      { type: 'toolStart', toolCallId: 't2', name: 'bash', args: { command: 'git commit -m x' }, intent: null },
    ]);
    expect(session.toolRuns).toBe(1);

    session.ingest({ sessionKey: session.key, seq: 10, event: toolEnd('t2', 'bash') });
    session.flush();

    expect(session.toolRuns).toBe(2);
  });
});
