import { describe, expect, it } from 'vitest';

import type { Subagent } from '$lib/ipc/bindings/Subagent';

import { applyEvent } from './reducer';
import { initialChatState } from './state';
import { activeSubagentCount, dockedSubagents, spawnedBy } from './subagents';

function subagent(overrides: Partial<Subagent> = {}): Subagent {
  return {
    id: 'ParserScout',
    parentToolCallId: 'call_1',
    agent: 'explore',
    status: 'running',
    index: 0,
    detached: true,
    description: null,
    task: 'Map the parser',
    intent: 'Finding parse callers',
    currentTool: 'grep',
    currentArgs: 'fn parse',
    recentTools: [{ tool: 'read', args: 'src/lib.rs', isError: false }],
    recentOutput: ['src/lib.rs:12: pub fn parse'],
    toolCount: 4,
    tokens: 12_000,
    contextTokens: 9000,
    contextWindow: 200_000,
    costUsd: 0.012,
    durationMs: 31_000,
    model: 'anthropic/claude-sonnet-4-5',
    ...overrides,
  };
}

const lifecycle = (id: string, status: Subagent['status']): Subagent =>
  subagent({ id, status, task: null, intent: null, currentTool: null, currentArgs: null, recentTools: null, recentOutput: null, toolCount: null, tokens: null, contextTokens: null, contextWindow: null, costUsd: null, durationMs: null, model: null });

describe('subagents', () => {
  it('upserts snapshots, keeping progress that a lifecycle frame does not carry', () => {
    let state = applyEvent(initialChatState('s'), { type: 'subagent', subagent: subagent() });
    state = applyEvent(state, { type: 'subagent', subagent: lifecycle('ParserScout', 'completed') });

    expect(state.subagents.ParserScout.status).toBe('completed');
    expect(state.subagents.ParserScout.tokens).toBe(12_000);
    expect(state.subagents.ParserScout.recentOutput).toEqual(['src/lib.rs:12: pub fn parse']);
  });

  it("counts the active ones and lists a call's subagents in spawn order", () => {
    let state = initialChatState('s');
    for (const snapshot of [subagent({ id: 'B', index: 1 }), subagent({ id: 'A', index: 0, status: 'completed' }), subagent({ id: 'C', parentToolCallId: 'call_2', status: 'pending' })]) {
      state = applyEvent(state, { type: 'subagent', subagent: snapshot });
    }

    expect(activeSubagentCount(state)).toBe(2);
    expect(spawnedBy(state.subagents, 'call_1').map((entry) => entry.id)).toEqual(['A', 'B']);
  });

  it('docks only the calls that still have a live subagent, not earlier finished iterations', () => {
    let state = initialChatState('s');
    for (const snapshot of [
      subagent({ id: 'Scout1', parentToolCallId: 'call_1', status: 'completed' }),
      subagent({ id: 'Review2', parentToolCallId: 'call_2', status: 'running' }),
      subagent({ id: 'Security2', parentToolCallId: 'call_2', status: 'completed', index: 1 }),
    ]) {
      state = applyEvent(state, { type: 'subagent', subagent: snapshot });
    }

    expect(dockedSubagents(state.subagents).map((entry) => entry.id)).toEqual(['Review2', 'Security2']);
  });
});
