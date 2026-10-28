import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import type { ToolCallState } from '$lib/chat/types';
import type { Subagent } from '$lib/ipc/bindings/Subagent';

import { subagentPanel } from '$lib/stores/subagentPanel.svelte';

import SubagentList from './SubagentList.svelte';
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
    contextTokens: null, contextWindow: null, costUsd: null, durationMs: null, model: null,
    label: null, report: null, messageRoute: null, canStop: null, ...extra,
  };
}

afterEach(() => subagentPanel.close('chat-1'));

describe('subagent card', () => {
  it('lists what a task call spawned once nothing runs', () => {
    const subagents = {
      ParserScout: subagent('ParserScout', 'completed', { index: 0, intent: 'Finding parse callers', tokens: 12_000, recentOutput: ['src/lib.rs:12: pub fn parse'] }),
      DocsReviewer: subagent('DocsReviewer', 'failed', { index: 1, agent: 'reviewer' }),
      Elsewhere: subagent('Elsewhere', 'completed', { parentToolCallId: 'call_9' }),
    };

    render(ToolCard, { call, active: false, subagents });

    const list = screen.getByRole('region', { name: 'Subagents' });
    expect(within(list).getByText(/1 done, 1 failed/)).toBeInTheDocument();
    expect(within(list).getByText('DocsReviewer')).toBeInTheDocument();
    expect(within(list).getByText('src/lib.rs:12: pub fn parse')).toBeInTheDocument();
    expect(within(list).queryByText('Elsewhere')).toBeNull();
  });

  it('opens the subagent panel on the clicked row and switches it when another row is clicked, with no inline expansion', async () => {
    const rows = [subagent('ParserScout', 'completed', { recentOutput: ['src/lib.rs:12: pub fn parse'] }), subagent('DocsReviewer', 'failed', { index: 1 })];

    render(SubagentList, { subagents: rows, panelKey: 'chat-1' });

    const scout = screen.getByRole('button', { name: /ParserScout/ });
    const docs = screen.getByRole('button', { name: /DocsReviewer/ });
    expect(scout).toHaveAttribute('aria-pressed', 'false');

    await fireEvent.click(scout);

    expect(subagentPanel.selectedId('chat-1')).toBe('ParserScout');
    expect(scout).toHaveAttribute('aria-pressed', 'true');
    expect(screen.queryByLabelText('Recent output')).toBeNull();

    await fireEvent.click(docs);

    expect(subagentPanel.selectedId('chat-1')).toBe('DocsReviewer');
    expect(scout).toHaveAttribute('aria-pressed', 'false');
    expect(docs).toHaveAttribute('aria-pressed', 'true');
  });

  it('leaves the list to the docked one while any subagent runs', () => {
    render(ToolCard, { call, active: true, subagents: { ParserScout: subagent('ParserScout', 'running') } });

    expect(screen.queryByRole('region', { name: 'Subagents' })).toBeNull();
  });

  it('shows no subagent list on other tools', () => {
    render(ToolCard, { call: { ...call, name: 'read' }, active: false, subagents: { ParserScout: subagent('ParserScout', 'completed') } });

    expect(screen.queryByRole('region', { name: 'Subagents' })).toBeNull();
  });

  it('shows no subagent list on a running wait call', () => {
    const wait: ToolCallState = { ...call, name: 'wait', args: {}, intent: null, status: 'running', resultText: null };

    render(ToolCard, { call: wait, active: true, subagents: { ParserScout: subagent('ParserScout', 'running') } });

    expect(screen.queryByRole('region', { name: 'Subagents' })).toBeNull();
  });
});
