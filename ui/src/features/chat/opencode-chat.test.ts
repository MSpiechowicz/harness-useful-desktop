import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { restart } = vi.hoisted(() => ({ restart: vi.fn() }));
vi.mock('$lib/stores/agentSession.svelte', async (original) => ({
  ...(await original<typeof import('$lib/stores/agentSession.svelte')>()),
  agentSessions: { restart, sessions: [], activeKey: null, activeStarting: null, title: () => 'Chat' },
}));

import type { ChatSession } from '$lib/chat/session.svelte';
import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';
import type { SessionState } from '$lib/ipc/bindings/SessionState';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { withOpenCodeChat } from '$lib/ipc/mock/opencode-chat';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { chatSession, startedSession } from '../../../tests/chat-helpers';
import AgentPicker from './AgentPicker.svelte';
import ChatBar from './ChatBar.svelte';
import ChatControls from './ChatControls.svelte';
import Composer from './Composer.svelte';
import HarnessPicker from './HarnessPicker.svelte';
import { runSlash } from './slash-run';

beforeEach(() => {
  restart.mockReset();
  // The harness list as the backend gives it once OpenCode chat is on.
  harness.summaries = fixtures.harnesses.map(withOpenCodeChat);
});

afterEach(() => {
  harness.summaries = fixtures.harnesses;
  Object.assign(agentSessions, { active: null });
});

const reported = (agent: string | null): AgentEvent => ({ type: 'state', state: { ...fixtures.agent.state.state, agent } as SessionState });

/** A chat with OpenCode on the mock backend, once it has told the app which agent it runs. */
async function openCodeChat() {
  const started = await startedSession('opencode');
  await waitFor(() => expect(started.session.chat.session?.agent).toBe('build'));

  return started;
}

/** The agent starts or ends a turn, as its event stream says. */
function turn(session: ChatSession, type: 'agentStart' | 'settled'): void {
  const event: AgentEvent = type === 'settled' ? { type, aborted: false } : { type };

  session.ingest({ sessionKey: session.key, seq: session.chat.lastSeq + 1, event });
  session.flush();
}

const agentButton = () => screen.getByRole('button', { name: /^Agent: / });

describe('agent picker', () => {
  it('names the agent of an OpenCode chat and lists the agents, the current one checked', async () => {
    const { session } = await openCodeChat();
    render(AgentPicker, { session });

    await waitFor(() => expect(agentButton()).toHaveTextContent('Build'));
    await fireEvent.click(agentButton());
    const menu = await screen.findByRole('menu', { name: 'Choose agent' });

    expect(within(menu).getAllByRole('menuitemradio').map((row) => row.textContent?.trim())).toEqual(['Build', 'Plan']);
    expect(within(menu).getByRole('menuitemradio', { name: 'Build' })).toHaveAttribute('aria-checked', 'true');
    expect(within(menu).getByRole('menuitemradio', { name: 'Plan' })).toHaveAttribute('aria-checked', 'false');
  });

  it('switches the agent and shows the new one', async () => {
    const { session, of } = await openCodeChat();
    render(AgentPicker, { session });

    await waitFor(() => expect(agentButton()).toHaveTextContent('Build'));
    await fireEvent.click(agentButton());
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: 'Plan' }));

    await waitFor(() => expect(agentButton()).toHaveTextContent('Plan'));
    expect(of('agent_send').map((call) => call.action).filter((action) => action.type === 'setAgent')).toEqual([{ type: 'setAgent', agent: 'plan' }]);
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('opens from the slash command that stands for it', async () => {
    const { session } = await openCodeChat();
    render(AgentPicker, { session });

    await runSlash(session, { type: 'gui', target: 'agentPicker' });

    expect(await screen.findByRole('menu', { name: 'Choose agent' })).toBeInTheDocument();
  });

  it('is shut while the agent works, and says why', async () => {
    const { session } = await openCodeChat();
    render(AgentPicker, { session });
    await waitFor(() => expect(agentButton()).toHaveTextContent('Build'));

    turn(session, 'agentStart');

    await waitFor(() => expect(agentButton()).toBeDisabled());
    expect(agentButton()).toHaveAttribute('title', 'The agent can be switched once the current reply is done');

    turn(session, 'settled');
    await waitFor(() => expect(agentButton()).toBeEnabled());
  });

  it('does not switch the agent while a turn runs, even when asked from elsewhere', async () => {
    const { session, of } = await openCodeChat();
    turn(session, 'agentStart');

    await session.chooseAgent({ id: 'plan', name: 'Plan', description: null });

    expect(of('agent_send').filter((call) => call.action.type === 'setAgent')).toEqual([]);
    expect(session.chat.session?.agent).toBe('build');
  });

  it('is left out for a harness without an agent picker, whatever its state says', () => {
    const { session } = chatSession([{ type: 'ready', protocolVersion: 1 }, reported('build')]);
    render(AgentPicker, { session });

    expect(session.chat.session?.agent).toBe('build');
    expect(screen.queryByRole('button', { name: /^Agent: / })).toBeNull();
  });

  it('waits for the chat to report its agent before it shows', () => {
    const { session } = chatSession([{ type: 'ready', protocolVersion: 1 }, reported(null)], { harness: 'opencode' });
    render(AgentPicker, { session });

    expect(screen.queryByRole('button', { name: /^Agent: / })).toBeNull();
  });

  it('sits in the composer of an OpenCode chat and not in the composer of an OMP chat', async () => {
    const { session } = await openCodeChat();
    const { unmount } = render(Composer, { session });
    await waitFor(() => expect(agentButton()).toBeInTheDocument());
    unmount();

    const omp = chatSession([{ type: 'ready', protocolVersion: 1 }]);
    render(Composer, { session: omp.session });

    expect(screen.queryByRole('button', { name: /^Agent: / })).toBeNull();
  });

  it('says when the agents could not be loaded, and asks again when it opens', async () => {
    const { session } = await openCodeChat();
    const send = session.send.bind(session);
    let failures = 2;
    vi.spyOn(session, 'send').mockImplementation(async (action, options) => {
      if (action.type === 'getAvailableAgents' && failures > 0) {
        failures -= 1;

        return null;
      }

      return send(action, options);
    });
    render(AgentPicker, { session });
    await waitFor(() => expect(session.agentsFailed).toBe(true));

    await fireEvent.click(agentButton());
    expect(await screen.findByRole('alert')).toHaveTextContent('The agents could not be loaded. Open the list again to retry.');
    expect(screen.queryByText('Loading agents…')).toBeNull();

    await fireEvent.click(agentButton());
    await fireEvent.click(agentButton());
    expect(await screen.findByRole('menuitemradio', { name: 'Build' })).toBeInTheDocument();
    expect(screen.queryByRole('alert')).toBeNull();
  });
});

describe('the /agent command', () => {
  afterEach(() => {
    toasts.items = [];
  });

  it('says why it cannot switch while a reply runs, and opens nothing', async () => {
    const { session } = await openCodeChat();
    turn(session, 'agentStart');

    await runSlash(session, { type: 'gui', target: 'agentPicker' });

    expect(session.ui.agentPicker).toBe(false);
    expect(toasts.items.at(-1)).toMatchObject({ title: 'The agent can be switched once the current reply is done' });
  });

  it('says it is not available in a chat without an agent picker', async () => {
    const { session } = chatSession([{ type: 'ready', protocolVersion: 1 }, reported('build')]);

    await runSlash(session, { type: 'gui', target: 'agentPicker' });

    expect(session.ui.agentPicker).toBe(false);
    expect(toasts.items.at(-1)).toMatchObject({ title: 'Not available in the desktop app' });
  });
});

describe('preview badge', () => {
  it('marks the chat header of a harness whose chat is a preview, with what it means', async () => {
    const { session } = chatSession([{ type: 'ready', protocolVersion: 1 }], { harness: 'opencode', configuredApprovalMode: null });
    Object.assign(agentSessions, { active: session });
    render(ChatBar);

    const badge = screen.getByText('Preview');
    expect(badge).toBeInTheDocument();

    await fireEvent.pointerEnter(badge.closest('span[role="presentation"]') as HTMLElement);
    expect(await screen.findByRole('tooltip')).toHaveTextContent('Chat with OpenCode is new. Tell us what breaks.');
  });

  it('is not on the header of a chat that is not a preview', () => {
    const { session } = chatSession([{ type: 'ready', protocolVersion: 1 }]);
    Object.assign(agentSessions, { active: session });
    render(ChatBar);

    expect(screen.queryByText('Preview')).toBeNull();
  });

  it('says so on the harness row of the picker, in words and not as a badge', async () => {
    const { session } = chatSession([{ type: 'ready', protocolVersion: 1 }]);
    render(HarnessPicker, { session });

    await fireEvent.click(screen.getByRole('button', { name: /^Harness: / }));
    const menu = await screen.findByRole('menu');
    const row = within(menu).getByRole('menuitemradio', { name: /^OpenCode\b/ });

    expect(row).toHaveTextContent('Preview');
    expect(row).toHaveAttribute('title', 'Chat with OpenCode is new. Tell us what breaks.');
    expect(row).not.toHaveAttribute('aria-disabled', 'true');
    expect(within(menu).getByRole('menuitemradio', { name: /^Oh My Pi\b/ })).not.toHaveTextContent('Preview');
  });
});

describe('approval badge of a harness that keeps its own approval setting', () => {
  it('names the harness settings as the source until a mode is chosen for the chat', async () => {
    const { session } = chatSession([], { harness: 'opencode', configuredApprovalMode: null, approvalOverride: null });
    render(ChatControls, { session });

    expect(screen.getByText('Approvals: OpenCode settings')).toBeInTheDocument();
    const trigger = screen.getByRole('button', { name: 'Approval mode: OpenCode settings. Change it' });
    await fireEvent.pointerEnter(trigger.parentElement as HTMLElement);

    expect(await screen.findByRole('tooltip')).toHaveTextContent('Follows your OpenCode settings.');
  });

  it('shows the mode chosen for this chat, from the session and not from the settings', () => {
    const { session } = chatSession([], { harness: 'opencode', configuredApprovalMode: null, approvalOverride: 'write' });
    render(ChatControls, { session });

    expect(screen.getByText('Approvals: ask for writes')).toBeInTheDocument();
    expect(screen.queryByText(/OpenCode settings/)).toBeNull();
  });

  it('offers the same three modes as OMP, and restarts the chat with the one picked', async () => {
    const { session } = chatSession([], { harness: 'opencode', configuredApprovalMode: null, approvalOverride: null });
    render(ChatControls, { session });

    await fireEvent.click(screen.getByRole('button', { name: /Approval mode: OpenCode settings/ }));
    const choices = await screen.findAllByRole('menuitemradio');
    expect(choices.map((choice) => choice.textContent)).toEqual([
      expect.stringContaining('Use my configuration'),
      expect.stringContaining('ask every time'),
      expect.stringContaining('ask for writes'),
      expect.stringContaining('yolo'),
    ]);
    expect(screen.getByText('From your OpenCode settings')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('menuitemradio', { name: /ask for writes/ }));
    await fireEvent.click(screen.getByRole('button', { name: 'Restart with this mode' }));

    await waitFor(() => expect(restart).toHaveBeenCalledWith(session.key, { approvalOverride: 'write' }));
  });
});

describe('the queue of a harness that cannot withdraw a message', () => {
  it('sends nothing when asked to remove a queued message', async () => {
    const { session, of } = chatSession([{ type: 'ready', protocolVersion: 1 }], { harness: 'pi' });

    expect(await session.removeQueued('later', 'followUp')).toBe(false);

    expect(of('agent_send').filter((call) => call.action.type === 'removeQueued')).toEqual([]);
  });

  it('removes a queued message when the harness can withdraw it', async () => {
    const { session, of } = await openCodeChat();

    await session.removeQueued('later', 'followUp');

    expect(of('agent_send').map((call) => call.action)).toContainEqual({ type: 'removeQueued', message: 'later', queue: 'followUp' });
  });
});

describe('compact in a harness that takes no instructions for it', () => {
  it('compacts at once when picked from the menu, without asking for instructions', async () => {
    const { session, of } = await openCodeChat();
    render(Composer, { session });
    const box = screen.getByRole('combobox', { name: 'Message' });

    await fireEvent.input(box, { target: { value: '/compa' } });
    await waitFor(() => expect(screen.getAllByRole('option')[0]).toHaveTextContent('/compact'));
    await fireEvent.keyDown(box, { key: 'Enter' });

    await waitFor(() => expect(of('agent_send').map((call) => call.action)).toContainEqual({ type: 'compact', instructions: null }));
    expect(session.ui.ask).toBeNull();
  });
});
