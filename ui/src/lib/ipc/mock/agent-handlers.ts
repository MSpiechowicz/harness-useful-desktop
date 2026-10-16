import type { AgentAction } from '../bindings/AgentAction';
import type { AgentReply } from '../bindings/AgentReply';
import type { AgentSessionInfo } from '../bindings/AgentSessionInfo';
import type { AgentStartRequest } from '../bindings/AgentStartRequest';
import type { SessionState } from '../bindings/SessionState';
import { IpcFailure } from '../transport';
import { MockAgentStream } from './agent-player';
import { fixtures } from './fixtures';
import { withSampleRecall } from './memory-recall-sample';
import type { MockState } from './state';
import type { Handlers } from './types';

type AgentCommands = 'agent_start' | 'agent_send' | 'agent_ui_respond' | 'agent_bash' | 'agent_stop' | 'agent_list' | 'agent_move_project';

const MAX_SESSIONS = 8;
const THINKING_LEVELS = ['off', 'minimal', 'low', 'medium', 'high'];
const EXPORT_PATH = '/home/demo/Projects/demo/session-export.html';

interface MockSession {
  info: AgentSessionInfo;
  stream: MockAgentStream;
  state: SessionState;
  /** Whether the first message went out yet; only that one gets project-memory notes. */
  prompted: boolean;
}

const ACK: AgentReply = { type: 'ack' };

function infoFor(request: AgentStartRequest): AgentSessionInfo {
  const base = structuredClone(fixtures.agent.sessionInfo[request.harness]);
  const supportsOverride = request.harness === 'omp';

  return {
    ...base,
    key: crypto.randomUUID(),
    harness: request.harness,
    project: request.project,
    startedAt: new Date().toISOString(),
    approvalOverride: supportsOverride ? request.approvalOverride : null,
    trustedOnce: request.harness === 'pi' && request.trustOnce,
  };
}

function lastAssistantText(): string {
  const reply = fixtures.agent.history;
  const last = [...reply.messages].reverse().find((message) => message.role === 'assistant' && message.blocks.some((block) => block.type === 'text'));
  const block = last?.blocks.find((candidate) => candidate.type === 'text');

  return block?.type === 'text' ? block.text : '';
}

/** In-memory agent backend: one `MockAgentStream` per started session. */
export function agentHandlers(state: MockState, pace: () => number): Pick<Handlers, AgentCommands> {
  const sessions = new Map<string, MockSession>();

  function session(key: string): MockSession {
    const found = sessions.get(key);
    if (!found) {
      throw IpcFailure.from({ code: 'notFound', message: 'That agent session is not running.' });
    }

    return found;
  }

  /** What the user message shows: the first message of a chat carries sample memory notes while the preference is on. */
  function shownPrompt(target: MockSession, message: string): string {
    const first = !target.prompted;
    target.prompted = true;
    const { memoryEnabled, memoryRecall } = state.snapshot();

    return first && memoryEnabled && memoryRecall && !message.startsWith('/') ? withSampleRecall(message) : message;
  }

  function send(target: MockSession, action: AgentAction): AgentReply {
    const { stream } = target;

    switch (action.type) {
      case 'prompt':
        if (!stream.streaming) {
          void stream.play(action.message, shownPrompt(target, action.message));
        } else if (action.streamingBehavior) {
          stream.enqueue(action.streamingBehavior === 'steer' ? 'steering' : 'followUp', action.message);
        } else {
          throw IpcFailure.from({ code: 'invalidInput', message: 'The agent is busy; send it as steering or a follow-up.' });
        }

        return { type: 'prompt', agentInvoked: true, disposition: null };

      case 'steer':
      case 'followUp':
        stream.enqueue(action.type === 'steer' ? 'steering' : 'followUp', action.message);

        return ACK;

      case 'removeQueued':
        return { type: 'queueRemoval', removed: stream.dequeue(action.queue === 'steer' ? 'steering' : 'followUp', action.message) };

      case 'abort':
        stream.abort();

        return ACK;

      default:
        return configure(target, action);
    }
  }

  /** Everything that is not about the running turn: pickers, naming, read-only queries. */
  function configure(target: MockSession, action: AgentAction): AgentReply {
    const { stream } = target;

    switch (action.type) {
      case 'getState':
        return { type: 'state', state: structuredClone(target.state) };

      case 'getAvailableModels':
        return structuredClone(fixtures.agent.models);

      case 'getAvailableThinkingLevels':
        return { type: 'thinkingLevels', levels: THINKING_LEVELS };

      case 'getSessionStats':
        return structuredClone(fixtures.agent.stats);

      case 'getHistory':
        return structuredClone(fixtures.agent.history);

      case 'getLastAssistantText':
        return { type: 'text', text: lastAssistantText() };

      case 'exportHtml':
        return { type: 'exported', path: EXPORT_PATH };

      case 'refreshCommands':
        return { type: 'commands', commands: structuredClone(fixtures.commands[target.info.harness]) };

      case 'setModel': {
        const model = fixtures.agent.models.models.find((option) => option.provider === action.provider && option.id === action.modelId) ?? null;
        target.state = { ...target.state, model };
        stream.emit({ type: 'modelChanged', model });

        return ACK;
      }

      case 'setThinkingLevel':
        target.state = { ...target.state, thinkingLevel: action.level };
        stream.emit({ type: 'thinkingChanged', level: action.level });

        return ACK;

      case 'setSessionName':
        target.state = { ...target.state, sessionName: action.name };
        stream.emit({ type: 'state', state: target.state });

        return ACK;

      case 'compact':
        stream.emit({ type: 'compaction', phase: 'start', reason: 'manual', error: null });
        setTimeout(() => stream.emit({ type: 'compaction', phase: 'end', reason: 'manual', error: null }), 600 * pace());

        return ACK;

      default:
        return ACK;
    }
  }

  return {
    agent_start: ({ request, onEvent }) => {
      state.requireKnown(request.project);
      if (sessions.size >= MAX_SESSIONS) {
        throw IpcFailure.from({ code: 'unsupported', message: `At most ${MAX_SESSIONS} agent sessions can run at once.` });
      }

      const info = infoFor(request);
      const stream = new MockAgentStream(info.key, onEvent, pace);
      const current = structuredClone(fixtures.agent.state.state);
      sessions.set(info.key, { info, stream, state: current, prompted: false });

      // Like the real backend, the first events arrive right after the command returns.
      setTimeout(() => {
        stream.emit({ type: 'ready', protocolVersion: info.protocolVersion });
        stream.emit({ type: 'state', state: current });
      }, 0);

      return structuredClone(info);
    },

    agent_send: ({ key, action }) => send(session(key), action),

    agent_ui_respond: ({ key, response }) => {
      session(key).stream.respond(response);

      return null;
    },

    agent_bash: ({ key, command, confirmed }) => {
      session(key);
      if (!confirmed) {
        throw IpcFailure.from({ code: 'forbidden', message: 'Running a command needs the user to confirm it first.' });
      }

      return { output: `$ ${command}\n(mock) command output\n`, exitCode: 0, cancelled: false, truncated: false };
    },

    agent_stop: ({ key }) => {
      sessions.get(key)?.stream.close();
      sessions.delete(key);

      return null;
    },

    agent_move_project: ({ key, project }) => {
      const entry = session(key);
      if (entry.info.harness !== 'omp') {
        throw IpcFailure.from({ code: 'unsupported', message: 'This harness cannot move a session to another project.' });
      }
      if (entry.stream.streaming) {
        throw IpcFailure.from({ code: 'unsupported', message: 'The agent is working; move the chat once it is done.' });
      }

      entry.info = { ...entry.info, project };

      return structuredClone(entry.info);
    },

    agent_list: () => [...sessions.values()].map((entry) => structuredClone(entry.info)),
  };
}
