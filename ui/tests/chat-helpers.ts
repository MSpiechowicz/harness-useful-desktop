import { ChatSession } from '$lib/chat/session.svelte';
import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';
import type { AgentSessionInfo } from '$lib/ipc/bindings/AgentSessionInfo';
import type { CommandArgs, CommandName } from '$lib/ipc/commands';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { defaultScenario } from '$lib/ipc/mock/scenarios';
import { createMockTransport } from '$lib/ipc/mock/transport';
import type { Transport } from '$lib/ipc/transport';
import { AgentSessionsStore } from '$lib/stores/agentSession.svelte';
import { ToastsStore } from '$lib/stores/toasts.svelte';

export type Call = { [K in CommandName]: [K, CommandArgs<K>] }[CommandName];

/** The mock backend, with every command recorded so tests can see what the UI asked for. */
export function recordingTransport() {
  const mock = createMockTransport(defaultScenario(), 0);
  const calls: Call[] = [];

  const transport: Transport = {
    invoke(name, args) {
      calls.push([name, args] as Call);

      return mock.invoke(name, args);
    },
  };

  return {
    transport,
    calls,
    /** Arguments of every call to one command. */
    of: <K extends CommandName>(name: K): CommandArgs<K>[] => calls.filter((call) => call[0] === name).map((call) => call[1] as CommandArgs<K>),
  };
}

export const PROJECT = fixtures.recentProjects[0].path;

export function sampleInfo(overrides: Partial<AgentSessionInfo> = {}): AgentSessionInfo {
  return { ...fixtures.agent.sessionInfo.omp, project: PROJECT, ...overrides };
}

/** A session wired to a recording mock transport, with the given events already applied. */
export function chatSession(events: AgentEvent[] = [], info: Partial<AgentSessionInfo> = {}) {
  const backend = recordingTransport();
  const notifications = new ToastsStore();
  const session = new ChatSession(sampleInfo(info), backend.transport, notifications);
  session.catalog = fixtures.commands[session.info.harness];

  events.forEach((event, index) => session.ingest({ sessionKey: session.key, seq: index + 1, event }));
  session.flush();

  return { session, notifications, ...backend };
}

export function storeWithMock() {
  const backend = recordingTransport();
  const notifications = new ToastsStore();

  return { store: new AgentSessionsStore(backend.transport, notifications), notifications, ...backend };
}

/** Events that put a session in the streaming state, as the agent does at the start of a turn. */
export const STREAMING: AgentEvent[] = [{ type: 'ready', protocolVersion: 1 }, { type: 'agentStart' }];

/** A session started on the mock backend, so requests that need a live agent (bash, restart) work. */
export async function startedSession(harness: 'omp' | 'pi' = 'omp') {
  const backend = storeWithMock();
  const session = await backend.store.create(harness, PROJECT);
  if (!session) {
    throw new Error('the mock agent did not start');
  }

  session.catalog = fixtures.commands[harness];

  return { session, ...backend };
}
