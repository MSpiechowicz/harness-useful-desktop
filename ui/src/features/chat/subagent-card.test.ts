import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { ToolCallState } from '$lib/chat/types';
import type { Subagent } from '$lib/ipc/bindings/Subagent';

import ToolCard from './ToolCard.svelte';

const call: ToolCallState = {
  id: 'call_1',
  name: 'task',
  args: { agent: 'explore', task: 'Map the parser' },
  intent: 'Spawning scouts',
  status: 'ok',
  partial: null,
  result: null,
  resultText: 'Spawned agent `ParserScout`.',
  touchedPaths: [],
};

function subagent(id: string, status: Subagent['status'], extra: Partial<Subagent> = {}): Subagent {
  return {
    id, parentToolCallId: 'call_1', agent: 'explore', status, index: 0, detached: true, description: null, task: `Task of ${id}`,
    intent: null, currentTool: null, currentArgs: null, recentTools: null, recentOutput: null, toolCount: null, tokens: null,
    contextTokens: null, contextWindow: null, costUsd: null, durationMs: null, model: null, ...extra,
  };
}

describe('subagent card', () => {
  it('lists what a task call spawned, live, and expands a row to its recent work', async () => {
    const subagents = {
      ParserScout: subagent('ParserScout', 'running', { index: 0, intent: 'Finding parse callers', tokens: 12_000, recentOutput: ['src/lib.rs:12: pub fn parse'] }),
      DocsReviewer: subagent('DocsReviewer', 'completed', { index: 1, agent: 'reviewer' }),
      Elsewhere: subagent('Elsewhere', 'running', { parentToolCallId: 'call_9' }),
    };

    render(ToolCard, { call, active: true, subagents });

    const list = screen.getByRole('region', { name: 'Subagents' });
    expect(within(list).getByText(/1 running, 1 done/)).toBeInTheDocument();
    expect(within(list).getByText('Finding parse callers')).toBeInTheDocument();
    expect(within(list).queryByText('Elsewhere')).toBeNull();

    await fireEvent.click(within(list).getByText('ParserScout'));

    expect(within(list).getByLabelText('Recent output')).toHaveTextContent('src/lib.rs:12: pub fn parse');
  });

  it('shows no subagent list on other tools', () => {
    render(ToolCard, { call: { ...call, name: 'read' }, active: true, subagents: { ParserScout: subagent('ParserScout', 'running') } });

    expect(screen.queryByRole('region', { name: 'Subagents' })).toBeNull();
  });

  it('lists the running subagents on a running wait call, and only those', () => {
    const subagents = {
      ParserScout: subagent('ParserScout', 'running', { intent: 'Finding parse callers' }),
      DocsReviewer: subagent('DocsReviewer', 'completed', { index: 1 }),
    };
    const wait: ToolCallState = { ...call, name: 'wait', args: {}, intent: null, status: 'running', resultText: null };

    const { rerender } = render(ToolCard, { call: wait, active: true, subagents });

    const list = screen.getByRole('region', { name: 'Subagents' });
    expect(within(list).getByText('Finding parse callers')).toBeInTheDocument();
    expect(within(list).queryByText('DocsReviewer')).toBeNull();

    rerender({ call: { ...wait, status: 'ok' }, active: true, subagents });
    expect(screen.queryByRole('region', { name: 'Subagents' })).toBeNull();
  });
});
