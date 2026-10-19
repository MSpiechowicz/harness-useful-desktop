import type { AgentOption } from '../bindings/AgentOption';
import type { AgentSessionInfo } from '../bindings/AgentSessionInfo';
import type { Capabilities } from '../bindings/Capabilities';
import type { HarnessId } from '../bindings/HarnessId';
import type { HarnessSummary } from '../bindings/HarnessSummary';
import type { SessionState } from '../bindings/SessionState';
import { fixtures } from './fixtures';

/**
 * What the backend reports for OpenCode once its chat is on. The recorded fixtures are generated from the Rust catalog, so
 * the mock states the chat side of it here until they are regenerated, and a regenerated fixture then agrees with it.
 */
export const OPENCODE_CHAT_CAPABILITIES: Pick<Capabilities, 'chat' | 'agentPicker' | 'chatPreview' | 'approvalModes' | 'removeQueued' | 'moveSession' | 'sessionTree'> = {
  chat: true,
  agentPicker: true,
  chatPreview: true,
  approvalModes: true,
  removeQueued: true,
  moveSession: false,
  sessionTree: false,
};

/** The primary agents the mock OpenCode offers, the first one being the chat's start agent. */
export const OPENCODE_AGENTS: AgentOption[] = [
  { id: 'build', name: 'Build', description: 'The default agent. It edits files and runs commands.' },
  { id: 'plan', name: 'Plan', description: 'Reads and plans. It changes nothing until you switch back.' },
];

/** The harness summary as the backend lists it with OpenCode chat on; any other harness is returned as it is. */
export function withOpenCodeChat(summary: HarnessSummary): HarnessSummary {
  return summary.info.id === 'opencode' ? { ...summary, capabilities: { ...summary.capabilities, ...OPENCODE_CHAT_CAPABILITIES } } : summary;
}

/** The capabilities the mock backend gives a harness. */
export function capabilitiesOf(harness: HarnessId): Capabilities | undefined {
  const summary = fixtures.harnesses.find((candidate) => candidate.info.id === harness);

  return summary && withOpenCodeChat(summary).capabilities;
}

/** The session info of a new chat of `harness`: the recorded one, else (OpenCode) an OMP one with no configured approval mode. */
export function baseSessionInfo(harness: HarnessId): AgentSessionInfo {
  const recorded = (fixtures.agent.sessionInfo as Partial<Record<HarnessId, AgentSessionInfo>>)[harness];
  if (recorded) {
    return structuredClone(recorded);
  }

  // OpenCode keeps its approval rules in its own settings, which the app does not read.
  return { ...structuredClone(fixtures.agent.sessionInfo.omp), harness, configuredApprovalMode: null, protocolVersion: null };
}

/** The state a new chat of `harness` starts with: a chat with an agent picker starts on the first agent. */
export function startState(harness: HarnessId): SessionState {
  const state = structuredClone(fixtures.agent.state.state);

  return capabilitiesOf(harness)?.agentPicker ? { ...state, agent: OPENCODE_AGENTS[0].id } : state;
}
