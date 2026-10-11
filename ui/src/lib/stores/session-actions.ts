import type { ChatSession } from '$lib/chat/session.svelte';
import type { SessionRef } from '$lib/ipc/bindings/SessionRef';

import type { AgentSessionsStore } from './agentSession.svelte';
import { nav } from './nav.svelte';

/** The open chat that already runs this saved session, if any; two agents must never append to one file. */
function openChatOf(agents: AgentSessionsStore, ref: SessionRef): ChatSession | undefined {
  return agents.sessions.find((session) => session.info.harness === ref.harness && session.chat.session?.sessionId === ref.id);
}

/**
 * A chat the user has not written in yet (no messages, nothing typed, not working) for this harness and project,
 * preferring the one on screen: a resumed or forked session takes its tab instead of opening another.
 */
function emptyChatFor(agents: AgentSessionsStore, ref: SessionRef, project: string): ChatSession | undefined {
  const empty = (session: ChatSession): boolean =>
    session.info.harness === ref.harness && session.info.project === project && !session.streaming && session.chat.items.length === 0 && session.draft.trim() === '';

  return agents.active && empty(agents.active) ? agents.active : agents.sessions.find(empty);
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

  const empty = emptyChatFor(agents, ref, project);
  const session = await startResumed(agents, ref, project);
  if (!session) {
    return false;
  }

  if (empty) {
    await agents.takeOver(empty.key, session.key);
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
  const empty = emptyChatFor(agents, ref, project);
  const session = await startResumed(agents, ref, project);
  if (!session) {
    return false;
  }

  const reply = await session.send({ type: 'fork', entryId });
  if (!reply) {
    await agents.stop(session.key);

    return false;
  }

  if (empty) {
    await agents.takeOver(empty.key, session.key);
  }
  await session.loadHistory();
  nav.go('chat');

  return true;
}
