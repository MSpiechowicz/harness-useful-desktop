import type { ChatSession } from '$lib/chat/session.svelte';
import type { SessionRef } from '$lib/ipc/bindings/SessionRef';

import type { AgentSessionsStore } from './agentSession.svelte';
import { nav } from './nav.svelte';

/** The open chat that already runs this saved session, if any; two agents must never append to one file. */
function openChatOf(agents: AgentSessionsStore, ref: SessionRef): ChatSession | undefined {
  return agents.sessions.find((session) => session.info.harness === ref.harness && session.chat.session?.sessionId === ref.id);
}

function startResumed(agents: AgentSessionsStore, ref: SessionRef, project: string): Promise<ChatSession | null> {
  return agents.start({ harness: ref.harness, project, resume: ref, approvalOverride: null, trustOnce: false });
}

/** Opens a saved session in the chat view, reusing the chat that already has it open. */
export async function resumeInChat(agents: AgentSessionsStore, ref: SessionRef, project: string): Promise<boolean> {
  const open = openChatOf(agents, ref);
  if (open) {
    agents.select(open.key);
    nav.go('chat');

    return true;
  }

  const session = await startResumed(agents, ref, project);
  if (!session) {
    return false;
  }

  void session.loadHistory();
  nav.go('chat');

  return true;
}

/**
 * Forks a saved session at an entry: resumes it in a fresh agent, asks the agent to fork, then
 * reloads the history so the chat shows the forked path. A refused fork stops the agent again.
 */
export async function forkInChat(agents: AgentSessionsStore, ref: SessionRef, entryId: string, project: string): Promise<boolean> {
  const session = await startResumed(agents, ref, project);
  if (!session) {
    return false;
  }

  const reply = await session.send({ type: 'fork', entryId });
  if (!reply) {
    await agents.stop(session.key);

    return false;
  }

  await session.loadHistory();
  nav.go('chat');

  return true;
}
