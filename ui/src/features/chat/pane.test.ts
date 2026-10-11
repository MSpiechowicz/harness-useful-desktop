import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { restart } = vi.hoisted(() => ({ restart: vi.fn() }));
vi.mock('$lib/stores/agentSession.svelte', async (original) => ({
  ...(await original<typeof import('$lib/stores/agentSession.svelte')>()),
  agentSessions: { restart },
}));

import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { chatSession, startedSession } from '../../../tests/chat-helpers';
import { samplePrefs } from '../../../tests/helpers';
import ChatHeader from './ChatHeader.svelte';
import ChatPane from './ChatPane.svelte';
import Composer from './Composer.svelte';
import { chatStatus } from './chat-status';

beforeEach(() => {
  restart.mockReset();
  harness.summaries = fixtures.harnesses;
});

function replayed(name: keyof typeof fixtures.agent.streams, count?: number): AgentEvent[] {
  return fixtures.agent.streams[name].map((envelope) => envelope.event).slice(0, count);
}

describe('approval mode badge', () => {
  it('warns that yolo runs tools without asking', async () => {
    const { session } = chatSession([], { configuredApprovalMode: 'yolo', approvalOverride: null });
    render(ChatHeader, { session });

    const badge = screen.getByText('Approvals: yolo');
    expect(badge.className).toContain('bg-warning-wash');

    const trigger = screen.getByRole('button', { name: /Approval mode: yolo/ });
    await fireEvent.pointerEnter(trigger.parentElement as HTMLElement);

    expect(await screen.findByRole('tooltip')).toHaveTextContent('tools run without asking');
  });

  it('shows the session override instead of the configured mode', () => {
    const { session } = chatSession([], { configuredApprovalMode: 'yolo', approvalOverride: 'alwaysAsk' });
    render(ChatHeader, { session });

    const badge = screen.getByText('Approvals: ask every time');
    expect(badge.className).toContain('bg-success-wash');
    expect(screen.queryByText('Approvals: yolo')).toBeNull();
  });

  it('restarts the session with the chosen mode', async () => {
    const { session } = chatSession([], { configuredApprovalMode: 'yolo', approvalOverride: null });
    render(ChatHeader, { session });

    await fireEvent.click(screen.getByRole('button', { name: /Approval mode: yolo/ }));
    const apply = await screen.findByRole('button', { name: 'Restart with this mode' });
    expect(apply).toBeDisabled();

    await fireEvent.click(screen.getByRole('menuitemradio', { name: /ask every time/ }));
    await fireEvent.click(apply);

    await waitFor(() => expect(restart).toHaveBeenCalledWith(session.key, { approvalOverride: 'alwaysAsk' }));
  });

  it('is left out for a harness without approval modes', () => {
    const { session } = chatSession([], { harness: 'pi', configuredApprovalMode: null });
    render(ChatHeader, { session });

    expect(screen.queryByText(/Approvals:/)).toBeNull();
  });
});

describe('PI trust', () => {
  it('trusts the project only on the explicit restart', async () => {
    const { session } = chatSession([], { harness: 'pi', configuredApprovalMode: null, trustedOnce: false });
    render(ChatHeader, { session });

    await fireEvent.click(screen.getByRole('button', { name: 'Trust project once' }));
    expect(await screen.findByText(/applies to this session only/)).toBeInTheDocument();
    expect(restart).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole('button', { name: 'Restart and trust this project' }));

    expect(restart).toHaveBeenCalledWith(session.key, { trustOnce: true });
  });

  it('shows that the project is trusted for this launch', () => {
    const { session } = chatSession([], { harness: 'pi', configuredApprovalMode: null, trustedOnce: true });
    render(ChatHeader, { session });

    expect(screen.getByText('Project trusted for this session')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Trust project once' })).toBeNull();
  });
});

describe('header controls', () => {
  it('lists models grouped by provider and switches the model from the composer', async () => {
    const { session, of } = await startedSession();
    session.apply((state) => ({ ...state, session: fixtures.agent.state.state }));
    render(Composer, { session });

    await fireEvent.click(screen.getByRole('button', { name: /Claude Sonnet 4.5/ }));
    const group = await screen.findByRole('group', { name: 'Models' });

    await waitFor(() => expect(within(group).getByText('GPT-5')).toBeInTheDocument());
    expect(within(group).getByText('anthropic')).toBeInTheDocument();
    expect(within(group).getByText('ollama')).toBeInTheDocument();

    await fireEvent.click(within(group).getByRole('menuitemradio', { name: /GPT-5/ }));

    await waitFor(() => expect(of('agent_send').map((call) => call.action)).toContainEqual({ type: 'setModel', provider: 'openai', modelId: 'gpt-5' }));
    await waitFor(() => expect(session.chat.session?.model?.id).toBe('gpt-5'));
  });

  it('shows the context usage in the composer', async () => {
    const { session } = await startedSession();
    session.apply((state) => ({ ...state, session: fixtures.agent.state.state }));
    render(Composer, { session });

    expect(screen.getByRole('meter', { name: 'Context window used' })).toHaveAttribute('aria-valuenow', '9');
  });

  it('loads statistics on demand', async () => {
    const { session } = await startedSession();
    session.apply((state) => ({ ...state, session: fixtures.agent.state.state }));
    render(ChatHeader, { session });

    await fireEvent.click(screen.getByRole('button', { name: 'Session statistics' }));

    expect(await screen.findByText('$0.0874')).toBeInTheDocument();
  });
});

describe('chat pane', () => {
  it('shows the crash card with the last output and restarts the session', async () => {
    const { session } = chatSession(replayed('crash'));
    render(ChatPane, { session });

    expect(screen.getByRole('alert', { name: 'Agent stopped' })).toHaveTextContent('exited with code 1');
    expect(screen.getByText(/Cannot read properties of undefined/)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Restart session' }));

    expect(restart).toHaveBeenCalledWith(session.key, {});
    expect(screen.getByRole('combobox', { name: 'Message' })).toBeDisabled();
  });

  it('shows a retryable error with a Retry button that resends the last message', async () => {
    const { session, of } = chatSession(replayed('error'));
    render(ChatPane, { session });

    expect(screen.getAllByRole('alert').some((alert) => alert.textContent?.includes('The agent could not finish that'))).toBe(true);

    await fireEvent.click(screen.getByRole('button', { name: 'Retry' }));

    await waitFor(() => expect(of('agent_send').map((call) => call.action)).toContainEqual({ type: 'prompt', message: 'Summarize the open TODOs.', streamingBehavior: null }));
  });

  it('pins an approval card above the composer while the agent waits', () => {
    const approvalIndex = fixtures.agent.streams.approval.findIndex((envelope) => envelope.event.type === 'uiRequest');
    const { session } = chatSession(replayed('approval', approvalIndex + 1));
    render(ChatPane, { session });

    expect(screen.getByRole('region', { name: 'Approval needed' })).toHaveTextContent('Allow bash: cargo test?');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('opens a dialog for other agent requests, one at a time', async () => {
    const { session } = chatSession(replayed('uiRequests'));
    render(ChatPane, { session });

    expect(await screen.findByRole('dialog', { name: 'Install hooks' })).toBeInTheDocument();
    expect(screen.getByText('Step 3 of 3')).toBeInTheDocument();
  });

  it('offers a link the agent shared with a copy button, and does not open it', () => {
    const { session } = chatSession(replayed('uiRequests'));
    render(ChatPane, { session });

    expect(screen.getByText('https://github.com/settings/tokens')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Copy link' })).toBeInTheDocument();
  });

  it('shows what the status bar says about each state', () => {
    expect(chatStatus(chatSession([{ type: 'ready', protocolVersion: 1 }]).session.chat)).toEqual({ label: 'Idle', tone: 'success' });
    expect(chatStatus(chatSession([{ type: 'ready', protocolVersion: 1 }, { type: 'agentStart' }]).session.chat).label).toBe('Streaming');
    expect(chatStatus(chatSession(replayed('basic')).session.chat).label).toBe('Settled');
    expect(chatStatus(chatSession(replayed('crash')).session.chat).tone).toBe('danger');
  });
});

describe('transcript', () => {
  it('shows tool calls as cards that expand to the result, and edits link to the changes view', async () => {
    const { session } = chatSession(replayed('basic'));
    render(ChatPane, { session });

    const card = await screen.findByRole('button', { name: /Return Option from parse/ });
    expect(card).toHaveAttribute('aria-expanded', 'false');

    await fireEvent.click(card);

    expect(await screen.findByText('Updated src/lib.rs (1 replacement)')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'View changes' }));

    expect(nav.current).toBe('changes');
    nav.go('chat');
  });

  it('collapses the thinking block once the message has ended, and opens it on request', async () => {
    const { session } = chatSession(replayed('basic'));
    render(ChatPane, { session });

    const toggle = await screen.findByRole('button', { name: /Thought process/ });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');

    await fireEvent.click(toggle);

    expect(await screen.findByText(/The parser panics on empty input/)).toBeInTheDocument();
  });

  it('hides the thought process when the chat header toggle turns it off', async () => {
    prefs.value = samplePrefs();
    try {
      const { session } = chatSession(replayed('basic'));
      render(ChatPane, { session });

      expect(await screen.findByRole('button', { name: /Thought process/ })).toBeInTheDocument();

      prefs.value = samplePrefs({ showThinking: false });

      await waitFor(() => expect(screen.queryByRole('button', { name: /Thought process/ })).toBeNull());
      expect(screen.getByRole('button', { name: 'Show thought process' })).toHaveAttribute('aria-pressed', 'false');
    } finally {
      prefs.value = null;
    }
  });

  it('keeps the thinking block open and marked as live while it streams', () => {
    const { session } = chatSession([
      { type: 'ready', protocolVersion: 1 },
      { type: 'agentStart' },
      { type: 'messageStart', messageId: 'a', role: 'assistant' },
      { type: 'thinkingDelta', messageId: 'a', contentIndex: 0, delta: 'weighing options' },
    ]);
    render(ChatPane, { session });

    expect(screen.getByRole('button', { name: /Thinking…/ })).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('weighing options')).toBeInTheDocument();
  });

  it('marks a running tool, and a tool that was denied as failed', () => {
    const approvalIndex = fixtures.agent.streams.approval.findIndex((envelope) => envelope.event.type === 'uiRequest');
    const running = chatSession(replayed('approval', approvalIndex + 1));
    const view = render(ChatPane, { session: running.session });
    expect(view.container.querySelector('[data-tool-status="running"]')).not.toBeNull();
    view.unmount();

    const denied = chatSession(replayed('approvalDenied'));
    const second = render(ChatPane, { session: denied.session });
    expect(second.container.querySelector('[data-tool-status="error"]')).not.toBeNull();
  });
});

describe('shell commands from the composer', () => {
  it('runs the command that was shown in the confirmation', async () => {
    const { session, of } = await startedSession();
    render(ChatPane, { session });

    const box = screen.getByRole('combobox', { name: 'Message' });
    await fireEvent.input(box, { target: { value: '!echo hi' } });
    await fireEvent.keyDown(box, { key: 'Enter' });
    await fireEvent.click(await screen.findByRole('button', { name: 'Run command' }));

    await waitFor(() => expect(of('agent_bash')).toEqual([{ key: session.key, command: 'echo hi', confirmed: true }]));
    expect(session.draft).toBe('');
  });
});

describe('tool results', () => {
  it('fold into their tool card instead of showing as separate rows', () => {
    const { session } = chatSession(replayed('basic'));
    render(ChatPane, { session });

    expect(session.chat.items.every((item) => item.kind !== 'message' || item.role !== 'toolResult')).toBe(true);
    expect(Object.values(session.chat.toolCalls).every((call) => call.resultText !== null)).toBe(true);
  });
});

