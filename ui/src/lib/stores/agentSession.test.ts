import { describe, expect, it, vi } from 'vitest';

import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { AgentStartRequest } from '$lib/ipc/bindings/AgentStartRequest';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { PROJECT, sampleInfo, storeWithMock } from '../../../tests/chat-helpers';
import { AgentSessionsStore } from './agentSession.svelte';
import { ToastsStore } from './toasts.svelte';

const REQUEST: AgentStartRequest = { harness: 'omp', project: PROJECT, resume: null, approvalOverride: null, trustOnce: false, utility: false, memoryChat: null };

function messageTexts(chat: { items: { kind: string; blocks?: { type: string; text?: string }[] }[] }): string[] {
  return chat.items.flatMap((item) => (item.kind === 'message' ? (item.blocks ?? []).flatMap((block) => (block.type === 'text' ? [block.text ?? ''] : [])) : []));
}

describe('starting sessions', () => {
  /** A transport whose agent_start waits until the test lets it finish. Finishing waits until the agent was asked to start (the chat's memory identity is settled first). */
  function slowStart(info = sampleInfo()) {
    let finish: (ok: boolean) => void = () => {};
    let asked = false;
    const transport: Transport = {
      invoke: (name) => {
        if (name === 'agent_start') {
          asked = true;

          return new Promise((resolve, reject) => {
            finish = (ok) => (ok ? resolve(info as never) : reject(new IpcFailure({ code: 'process', message: 'omp exited' })));
          });
        }

        return Promise.resolve([] as never);
      },
    };

    return {
      store: new AgentSessionsStore(transport, new ToastsStore()),
      finish: async (ok: boolean) => {
        await vi.waitUntil(() => asked);
        finish(ok);
      },
      info,
    };
  }

  it('shows a new chat as a starting tab at once and swaps in the session when the agent is ready', async () => {
    const { store, finish, info } = slowStart();

    const created = store.create('omp', PROJECT);
    expect(store.starting).toHaveLength(1);
    expect(store.activeStarting?.key).toBe(store.starting[0].key);

    await finish(true);
    await created;

    expect(store.starting).toHaveLength(0);
    expect(store.activeKey).toBe(info.key);
  });

  it('leaves the user on the tab they switched to while the chat was starting', async () => {
    const first = storeWithMock();
    const existing = await first.store.create('omp', PROJECT);
    const { store, finish, info } = slowStart(sampleInfo({ key: 'late-chat' }));
    store.sessions = [existing!];

    const created = store.create('omp', PROJECT);
    store.select(existing!.key);
    await finish(true);
    await created;

    expect(store.activeKey).toBe(existing!.key);
    expect(store.get(info.key)).toBeDefined();
  });

  it('gives unnamed chats a random free creature and cycles through the tabs', async () => {
    // Drop the agent's events: the mock's first `state` event names the chat, and whether it lands before the
    // assertions depends on the runner's speed.
    const { transport } = storeWithMock();
    const unnamed: Transport = {
      invoke: (name, args) => transport.invoke(name, (name === 'agent_start' ? { ...args, onEvent: () => {} } : args) as never),
    };
    // Draws near 1 pick the last free name; a draw of 0 picks the first free one.
    const random = vi.spyOn(Math, 'random').mockReturnValue(0.999);
    const store = new AgentSessionsStore(unnamed, new ToastsStore());
    const first = await store.create('omp', PROJECT);
    const second = await store.create('omp', PROJECT);

    expect(store.title(first!.key)).toBe('Daring Salamander');
    expect(store.title(second!.key)).toBe('Daring Eagle');

    store.cycle(1);
    expect(store.activeKey).toBe(first!.key);
    store.cycle(-1);
    expect(store.activeKey).toBe(second!.key);

    await store.stop(first!.key);
    random.mockReturnValue(0);
    const third = await store.create('omp', PROJECT);
    expect(store.title(third!.key)).toBe('Swift Godzilla');
  });

  it('drops the starting tab when the agent fails to start', async () => {
    const { store, finish } = slowStart();

    const created = store.create('omp', PROJECT);
    await finish(false);

    expect(await created).toBeNull();
    expect(store.starting).toHaveLength(0);
    expect(store.activeKey).toBeNull();
  });

  it('keeps events that arrive before the start command returns', async () => {
    const info = sampleInfo();
    const early: AgentEnvelope = { sessionKey: info.key, seq: 1, event: { type: 'ready', protocolVersion: 1 } };
    const transport: Transport = {
      invoke: async (name, args) => {
        if (name === 'agent_start') {
          (args as { onEvent: (envelope: AgentEnvelope) => void }).onEvent(early);

          return info as never;
        }

        return [] as never;
      },
    };

    const store = new AgentSessionsStore(transport, new ToastsStore());
    const session = await store.start(REQUEST);
    session?.flush();

    expect(session?.chat.readyCount).toBe(1);
    expect(session?.chat.phase).toBe('idle');
  });

  it('shows a toast and adds no tab when the agent cannot start', async () => {
    const notifications = new ToastsStore();
    const transport: Transport = { invoke: () => Promise.reject(new IpcFailure({ code: 'process', message: 'omp is not installed' })) };
    const store = new AgentSessionsStore(transport, notifications);

    expect(await store.start(REQUEST)).toBeNull();
    expect(store.sessions).toHaveLength(0);
    expect(notifications.items[0]).toMatchObject({ title: 'Could not start the agent', description: 'omp is not installed', tone: 'danger' });
  });

  it('keeps a long start failure on screen until dismissed, so its fix can be carried out', async () => {
    const notifications = new ToastsStore();
    const message = 'other users can write to the Harness Memory folder /home/me/.local/share/harness-memory. Run chmod 700 /home/me/.local/share/harness-memory and try again';
    const transport: Transport = { invoke: () => Promise.reject(new IpcFailure({ code: 'process', message })) };
    const store = new AgentSessionsStore(transport, notifications);

    expect(await store.start(REQUEST)).toBeNull();
    expect(notifications.items[0]).toMatchObject({ description: message, duration: 0 });
  });

  it('opens a tab per session, activates the newest and loads the slash catalog once per harness', async () => {
    const { store, of } = storeWithMock();

    const first = await store.create('omp', PROJECT);
    const second = await store.create('omp', PROJECT);

    expect(store.sessions).toHaveLength(2);
    expect(store.active).toBe(second);
    expect(first?.key).not.toBe(second?.key);
    await vi.waitFor(() => expect(first?.catalog.length).toBeGreaterThan(0));
    expect(of('commands_catalog')).toHaveLength(1);
  });
});

describe('talking to the agent', () => {
  it('streams a reply into the transcript and refreshes state once it settles', async () => {
    const { store, of } = storeWithMock();
    const session = await store.create('omp', PROJECT);

    await session?.submit('Make parse handle empty input', 'send');
    await vi.waitFor(() => expect(session?.chat.settledCount).toBe(1));

    expect(messageTexts(session!.chat).join('\n')).toContain('Make parse handle empty input');
    expect(messageTexts(session!.chat).join('\n')).toContain('Return `Option<Config>`');
    expect(of('agent_send').map((call) => call.action.type)).toEqual(expect.arrayContaining(['refreshCommands', 'prompt', 'getState']));
    expect(session?.commands.some((command) => command.name === 'review')).toBe(true);
  });

  it('answers an approval request: the card goes away and the agent gets the answer', async () => {
    const { store, of } = storeWithMock();
    const session = await store.create('omp', PROJECT);

    await session?.submit('needs approval', 'send');
    await vi.waitFor(() => expect(session?.approval).not.toBeNull());
    expect(session?.approval?.title).toMatch(/bash/);

    const prompt = session!.approval!;
    await session?.respond(prompt.id, { type: 'value', value: prompt.deny });

    expect(session?.approval).toBeNull();
    expect(of('agent_ui_respond')[0].response).toEqual({ id: prompt.id, outcome: { type: 'value', value: 'Deny' } });
    await vi.waitFor(() => expect(session?.chat.phase).toBe('idle'));
    expect(Object.values(session!.chat.toolCalls)[0].status).toBe('error');
  });

  it('retries the last message after a retryable failure', async () => {
    const { store, of } = storeWithMock();
    const session = await store.create('omp', PROJECT);

    await session?.submit('an error please', 'send');
    await vi.waitFor(() => expect(session?.chat.promptError?.retryable).toBe(true));
    await session?.retry();

    const prompts = of('agent_send').flatMap((call) => (call.action.type === 'prompt' ? [call.action.message] : []));
    // The retry resends the message as the agent got it, hook context included (the mock adds it to first prompts).
    expect(prompts[0]).toBe('an error please');
    expect(prompts[1]).toMatch(/^<harness-hook-context>\n[\s\S]*\n<\/harness-hook-context>\n\nan error please$/);
    expect(session?.chat.promptError).toBeNull();
  });

  it('shows the crash state when the agent exits', async () => {
    const { store } = storeWithMock();
    const session = await store.create('omp', PROJECT);

    await session?.submit('crash now', 'send');
    await vi.waitFor(() => expect(session?.ended).toBe(true));

    expect(session?.chat.crash?.stderr.length).toBeGreaterThan(0);
  });

  it('turns a rejected action into a toast and a null reply', async () => {
    const { store, notifications } = storeWithMock();
    const session = await store.create('omp', PROJECT);
    await store.stop(session!.key);

    expect(await session?.send({ type: 'getState' })).toBeNull();
    expect(notifications.items[0].title).toBe('The agent did not accept that');
  });
});

describe('tabs', () => {
  it('closing the active tab stops its agent and selects a neighbour', async () => {
    const { store, of } = storeWithMock();
    const first = await store.create('omp', PROJECT);
    const second = await store.create('omp', PROJECT);

    await store.stop(second!.key);

    expect(of('agent_stop')).toEqual([{ key: second!.key }]);
    expect(store.active).toBe(first);
    expect(store.sessions).toHaveLength(1);
  });

  it('restarts in place with a new approval mode and resumes the same conversation', async () => {
    const { store, of } = storeWithMock();
    const first = await store.create('omp', PROJECT);
    const second = await store.create('omp', PROJECT);
    first!.draft = 'half-written';
    await vi.waitFor(() => expect(first?.chat.session?.sessionId).toBeTruthy());
    first!.chat.items = [{ kind: 'message', id: 'm1', role: 'user', blocks: [{ type: 'text', text: 'hi' }] } as never];

    const fresh = await store.restart(first!.key, { approvalOverride: 'alwaysAsk' });

    expect(store.sessions.map((session) => session.key)).toEqual([fresh!.key, second!.key]);
    expect(store.active).toBe(fresh);
    expect(fresh?.info.approvalOverride).toBe('alwaysAsk');
    expect(fresh?.draft).toBe('half-written');
    expect(of('agent_start').at(-1)?.request).toMatchObject({
      resume: { harness: 'omp', id: fixtures.agent.state.state.sessionId },
      approvalOverride: 'alwaysAsk',
      trustOnce: false,
    });
    expect(of('agent_stop')).toEqual([{ key: first!.key }]);
  });

  it('restarts a chat with no message yet as a new session: the agent has saved nothing to resume', async () => {
    const { store, of } = storeWithMock();
    const chat = await store.create('omp', PROJECT);
    await vi.waitFor(() => expect(chat?.chat.session?.sessionId).toBeTruthy());

    await store.restart(chat!.key, { approvalOverride: 'alwaysAsk' });

    expect(of('agent_start').at(-1)?.request.resume).toBeNull();
  });

  it('trusts a PI project only for the restart that asks for it', async () => {
    const { store, of } = storeWithMock();
    const session = await store.create('pi', PROJECT);
    await vi.waitFor(() => expect(session?.chat.session).not.toBeNull());

    const fresh = await store.restart(session!.key, { trustOnce: true });
    const again = await store.restart(fresh!.key, {});

    expect(fresh?.info.trustedOnce).toBe(true);
    expect(again?.info.trustedOnce).toBe(false);
    expect(of('agent_start').map((call) => call.request.trustOnce)).toEqual([false, true, false]);
  });
});

describe('handing a chat to another harness', () => {
  it('replaces the chat in its own tab, keeping its place and draft, never adding a tab', async () => {
    const { store } = storeWithMock();
    const first = await store.create('omp', PROJECT);
    const second = await store.create('omp', PROJECT);
    const third = await store.create('omp', PROJECT);
    second!.draft = 'half a thought';

    const fresh = await store.changeHarness(second!.key, 'pi');

    expect(fresh?.info.harness).toBe('pi');
    expect(store.sessions.map((session) => session.key)).toEqual([first!.key, fresh!.key, third!.key]);
    expect(store.starting).toHaveLength(0);
    expect(store.activeKey).toBe(fresh!.key);
    expect(fresh!.draft).toBe('half a thought');
    expect(store.get(second!.key)).toBeUndefined();
  });

  it('leaves the chat as it was when the other harness does not start', async () => {
    const { store } = storeWithMock();
    const chat = await store.create('omp', PROJECT);
    const failing: Transport = { invoke: () => Promise.reject(new IpcFailure({ code: 'process', message: 'pi exited' })) };
    const broken = new AgentSessionsStore(failing, new ToastsStore());
    broken.sessions = [chat!];

    expect(await broken.changeHarness(chat!.key, 'pi')).toBeNull();
    expect(broken.sessions).toEqual([chat]);
    expect(chat!.switchingTo).toBeNull();
  });
});

describe('reloading chats after a plugin changed', () => {
  it('restarts the idle chats of that harness in place and leaves other harnesses alone', async () => {
    const { store } = storeWithMock();
    const chat = await store.create('omp', PROJECT);
    const other = await store.create('pi', PROJECT);

    expect(await store.reloadIdle('omp')).toBe(1);

    expect(store.sessions).toHaveLength(2);
    expect(store.get(chat!.key)).toBeUndefined();
    expect(store.sessions[0].info.harness).toBe('omp');
    expect(store.get(other!.key)).toBe(other);
    expect(await store.reloadIdle('omp')).toBe(1);
  });
});
