import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest';

import { ChatSession } from '$lib/chat/session.svelte';
import type { AgentAction } from '$lib/ipc/bindings/AgentAction';
import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';
import type { Subagent } from '$lib/ipc/bindings/Subagent';
import type { SubagentStep } from '$lib/ipc/bindings/SubagentStep';
import { fixtures } from '$lib/ipc/mock/fixtures';
import type { Transport } from '$lib/ipc/transport';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { subagentPanel } from '$lib/stores/subagentPanel.svelte';
import { ToastsStore } from '$lib/stores/toasts.svelte';

import { sampleInfo, STREAMING } from '../../../tests/chat-helpers';
import { samplePrefs } from '../../../tests/helpers';
import ChatPane from './ChatPane.svelte';

const CALL = 'call_task';

beforeAll(() => {
  prefs.value = samplePrefs({ motion: 'off' });
});

beforeEach(() => {
  harness.summaries = fixtures.harnesses;
});

function scout(overrides: Partial<Subagent> = {}): Subagent {
  return {
    id: 'ParserScout', parentToolCallId: CALL, agent: 'explore', status: 'running', index: 0, detached: true, description: null, task: 'Map every caller of `parse`.',
    intent: null, currentTool: null, currentArgs: null, recentTools: null, recentOutput: null, toolCount: 3, tokens: 12_000, contextTokens: 9000, contextWindow: 200_000,
    costUsd: 0.012, durationMs: 31_000, model: 'anthropic/claude-haiku-4-5', label: 'Map the parser', report: null, messageRoute: 'direct', canStop: true, ...overrides,
  };
}

function step(id: string, overrides: Partial<SubagentStep> = {}): AgentEvent {
  return { type: 'subagentStep', subagentId: 'ParserScout', step: { id, kind: 'tool', name: 'Read', detail: 'src/lib.rs', status: 'done', at: 1_000, output: null, outputLines: null, ...overrides } };
}

/** A chat whose agent works, with a `task` call in the transcript and the given subagent; every action the UI sends is kept and answered with an ack. */
function mount(subagent: Subagent | null = scout(), more: AgentEvent[] = [], { withCall = true, streaming = true } = {}) {
  const sent: AgentAction[] = [];
  const transport: Transport = {
    async invoke(name, args) {
      if (name === 'agent_send') {
        sent.push((args as { action: AgentAction }).action);
      }

      return { type: 'ack' } as never;
    },
  };
  const session = new ChatSession(sampleInfo(), transport, new ToastsStore());
  const events: AgentEvent[] = [
    ...(streaming ? STREAMING : [{ type: 'ready', protocolVersion: 1 } as AgentEvent]),
    ...(withCall ? [{ type: 'toolStart', toolCallId: CALL, name: 'task', args: {}, intent: null } as AgentEvent] : []),
    ...(subagent ? [{ type: 'subagent', subagent } as AgentEvent] : []),
    ...more,
  ];
  events.forEach((event, index) => session.ingest({ sessionKey: session.key, seq: index + 1, event }));
  session.flush();
  render(ChatPane, { session });

  /** The actions that reached the agent, apart from the background looks the chat asks for on its own. */
  const actions = () => sent.filter((action) => !['refreshCommands', 'getState', 'getAvailableModels'].includes(action.type));

  return { session, actions };
}

async function openRow(name = 'ParserScout') {
  await fireEvent.click(screen.getByRole('button', { name: new RegExp(name) }));

  return screen.findByRole('region', { name: /Map the parser|Review/ });
}

afterEach(() => {
  subagentPanel.close(sampleInfo().key);
});

describe('subagent panel', () => {
  it('opens from a row with the subagent title, what it does now and a Stop for the one that can be stopped', async () => {
    mount(scout(), [step('t1', { status: 'running', name: 'Bash', detail: 'cargo test parse' })]);

    const panel = await openRow();

    expect(within(panel).getByRole('heading', { level: 2, name: 'Map the parser' })).toBeInTheDocument();
    expect(within(panel).getByText('explore · anthropic/claude-haiku-4-5 · background')).toBeInTheDocument();
    expect(within(panel).getByTestId('subagent-live')).toHaveAttribute('data-status', 'running');
    expect(within(panel).getByText('cargo test parse', { selector: 'span' })).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Stop' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /ParserScout/ })).toHaveAttribute('aria-pressed', 'true');
  });

  it('switches to another subagent when its row is clicked', async () => {
    const other = scout({ id: 'DocsReviewer', label: 'Review the docs', index: 1, task: null });
    const { session } = mount(scout(), [{ type: 'subagent', subagent: other }]);

    await openRow();
    await fireEvent.click(screen.getByRole('button', { name: /DocsReviewer/ }));

    expect(await screen.findByRole('region', { name: 'Review the docs' })).toBeInTheDocument();
    expect(subagentPanel.selectedId(session.key)).toBe('DocsReviewer');
  });

  it('sends stopSubagent for this subagent only', async () => {
    const { actions } = mount();
    const panel = await openRow();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Stop' }));

    await waitFor(() => expect(actions()).toEqual([{ type: 'stopSubagent', id: 'ParserScout' }]));
  });

  it('offers no Stop to a subagent that cannot be stopped or no longer runs', async () => {
    mount(scout({ canStop: false }), [{ type: 'subagent', subagent: scout({ id: 'Done', label: 'Finished one', index: 1, status: 'completed', canStop: true }) }]);

    const panel = await openRow();
    expect(within(panel).queryByRole('button', { name: 'Stop' })).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: /Done/ }));
    const finished = await screen.findByRole('region', { name: 'Finished one' });
    expect(within(finished).queryByRole('button', { name: 'Stop' })).toBeNull();
    expect(within(finished).getByTestId('subagent-live')).toHaveAttribute('data-status', 'completed');
  });

  it('closes on Escape without counting toward the double Escape that stops the agent', async () => {
    const { actions, session } = mount();
    await openRow();

    await fireEvent.keyDown(window, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByTestId('subagent-live')).toBeNull());

    expect(subagentPanel.selectedId(session.key)).toBeNull();
    expect(screen.queryByText('Esc again to stop')).toBeNull();

    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(await screen.findByText('Esc again to stop')).toBeInTheDocument();
    expect(actions()).toEqual([]);
  });

  it('leaves an Escape that something else took to that', async () => {
    const { session } = mount();
    await openRow();
    const press = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
    press.preventDefault();

    window.dispatchEvent(press);

    expect(subagentPanel.selectedId(session.key)).toBe('ParserScout');
  });

  it('offers no tabs to a subagent that has only its activity, and the Brief once it has a task', async () => {
    mount(scout({ task: null }), [{ type: 'subagent', subagent: scout({ id: 'WithTask', index: 1, label: 'Review it' }) }]);

    const panel = await openRow();
    expect(within(panel).queryByRole('radiogroup', { name: 'Subagent view' })).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: /WithTask/ }));
    const tabs = within(await screen.findByRole('region', { name: 'Review it' })).getByRole('radiogroup', { name: 'Subagent view' });

    expect(within(tabs).getAllByRole('radio').map((tab) => tab.textContent?.trim())).toEqual(['Activity', 'Brief']);
  });

  it('lists the steps with the tool that runs now, and shows the end of a tool result on the Output tab', async () => {
    const lines = Array.from({ length: 40 }, (_, index) => `case ${index + 1} ok`).join('\n');
    mount(scout(), [
      step('t1', { name: 'Read', detail: 'src/lib.rs' }),
      step('t2', { name: 'Bash', detail: 'cargo test', output: lines, outputLines: 212 }),
      step('t3', { kind: 'text', name: null, detail: 'The parser is small.' }),
      step('t4', { name: 'Grep', detail: 'fn parse', status: 'running' }),
    ]);

    const panel = await openRow();
    const steps = within(panel).getByRole('list', { name: 'Steps' });
    expect(within(steps).getAllByRole('listitem')).toHaveLength(4);
    expect(within(steps).getByText('Says')).toBeInTheDocument();
    expect(within(steps).getByText('running')).toBeInTheDocument();
    expect(within(panel).getByText('Tokens')).toBeInTheDocument();
    expect(within(panel).getByText('Context')).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('radio', { name: 'Output' }));

    const output = within(panel).getByRole('region', { name: 'Output' });
    expect(within(output).getAllByRole('listitem')).toHaveLength(20);
    expect(within(panel).getByText('Last 20 of 212 lines')).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Show all' }));

    expect(within(output).getAllByRole('listitem')).toHaveLength(40);
    expect(within(panel).getByRole('button', { name: 'Show last lines' })).toBeInTheDocument();
  });

  it('shows the final report as the Result of a finished subagent', async () => {
    mount(scout({ status: 'completed', report: 'Two fixtures are **empty**.', canStop: false, durationMs: 9_000 }));

    const panel = await openRow();
    await fireEvent.click(within(panel).getByRole('radio', { name: 'Result' }));

    expect(within(panel).getByText('Final report')).toBeInTheDocument();
    expect(within(panel).getByText('returned to the orchestrator · 9s')).toBeInTheDocument();
    expect(within(panel).getByText('empty', { selector: 'strong' })).toBeInTheDocument();
  });

  it('shows the brief with the call that spawned it, and Jump to it marks that card and the row', async () => {
    const { session } = mount();
    const panel = await openRow();

    await fireEvent.click(within(panel).getByRole('radio', { name: 'Brief' }));

    expect(within(panel).getByText('Orchestrator')).toBeInTheDocument();
    expect(within(panel).getByText(/Spawned by the Task call at/)).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: /Jump to it/ }));

    expect(subagentPanel.jump).toMatchObject({ key: session.key, toolCallId: CALL });
    await waitFor(() => expect(screen.getByRole('button', { name: /ParserScout/ })).toHaveAttribute('data-flash'));
  });

  it('leaves out the spawn line when the call is not in the transcript', async () => {
    mount(scout(), [], { withCall: false });
    const panel = await openRow();

    await fireEvent.click(within(panel).getByRole('radio', { name: 'Brief' }));

    expect(within(panel).queryByText(/Spawned by/)).toBeNull();
    expect(within(panel).queryByRole('button', { name: /Jump to it/ })).toBeNull();
  });

  it('shows the brief when the harness names the spawning call after a built-in key', async () => {
    mount(scout({ parentToolCallId: '__proto__' }), [], { withCall: false });
    const panel = await openRow();

    await fireEvent.click(within(panel).getByRole('radio', { name: 'Brief' }));

    expect(within(panel).getByText(/Map every caller of/)).toBeTruthy();
    expect(within(panel).queryByText(/Spawned by/)).toBeNull();
  });
});

describe('subagent message box', () => {
  async function type(text: string) {
    const field = screen.getByRole('textbox', { name: 'Message to the subagent' });
    await fireEvent.input(field, { target: { value: text } });

    return field;
  }

  it('sends a direct message on Enter, lists it among the steps and keeps Shift+Enter a newline', async () => {
    const { actions } = mount();
    const panel = await openRow();

    expect(within(panel).getByRole('textbox', { name: 'Message to the subagent' })).toHaveAttribute('placeholder', 'Message Map the parser…');
    expect(within(panel).getByRole('button', { name: 'Send' })).toBeDisabled();

    const field = await type('also check the CLI');
    await fireEvent.keyDown(field, { key: 'Enter', shiftKey: true });
    expect(actions()).toEqual([]);

    await fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(actions()).toEqual([{ type: 'messageSubagent', id: 'ParserScout', message: 'also check the CLI' }]));
    expect(await within(panel).findByText('also check the CLI')).toBeInTheDocument();
    expect(field).toHaveValue('');
  });

  it('sends through the orchestrator for a subagent that takes no direct message, and says so', async () => {
    const { actions } = mount(scout({ messageRoute: 'orchestrator' }));
    const panel = await openRow();

    expect(within(panel).getByRole('textbox', { name: 'Message to the subagent' })).toHaveAttribute('placeholder', 'Ask the orchestrator to follow up…');

    await type('use the other fixture');
    await fireEvent.click(within(panel).getByRole('button', { name: 'Send' }));

    await waitFor(() =>
      expect(actions()).toEqual([{ type: 'prompt', message: 'Message for subagent "Map the parser" ("ParserScout"): use the other fixture', images: [], streamingBehavior: 'steer' }]),
    );
    expect(await within(panel).findByText('Sent to the orchestrator')).toBeInTheDocument();
  });

  it('asks the orchestrator to follow up for a finished subagent', async () => {
    mount(scout({ status: 'completed', canStop: false }));
    const panel = await openRow();

    expect(within(panel).getByRole('textbox', { name: 'Message to the subagent' })).toHaveAttribute('placeholder', 'Ask the orchestrator to follow up…');
  });

  it('is left out when the harness gave no route', async () => {
    mount(scout({ messageRoute: null }));
    await openRow();

    expect(screen.queryByRole('textbox', { name: 'Message to the subagent' })).toBeNull();
  });
});
