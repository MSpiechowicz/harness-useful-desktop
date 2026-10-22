import type { ChatSession } from '$lib/chat/session.svelte';
import { t, tn } from '$lib/i18n/i18n.svelte';
import type { SessionRef } from '$lib/ipc/bindings/SessionRef';
import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';

import type { AgentSessionsStore } from './agentSession.svelte';
import { nav } from './nav.svelte';
import { sessionList } from './sessions.svelte';
import { toasts } from './toasts.svelte';
import { viewState } from './view-state.svelte';

/** The open chat that already runs this saved session, if any; two agents must never append to one file. */
export function openChatOf(agents: AgentSessionsStore, ref: SessionRef): ChatSession | undefined {
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

/** Whether a start trusts the folder once: only after the user picked "Trust once" on the toast of a refused start. */
interface TrustOption {
  trustOnce?: boolean;
}

/** Starts an agent on saved session `ref`; `retry` is the caller's flow again, trusting the folder once (see `AgentSessionsStore.start`). */
function startResumed(agents: AgentSessionsStore, ref: SessionRef, project: string, trustOnce: boolean, retry: () => void): Promise<ChatSession | null> {
  return agents.start({ harness: ref.harness, project, resume: ref, approvalOverride: null, trustOnce, utility: false, memoryChat: null }, { retry });
}

/** Opens a saved session in the chat view, reusing the chat that already has it open. */
export async function resumeInChat(agents: AgentSessionsStore, ref: SessionRef, project: string, { trustOnce = false }: TrustOption = {}): Promise<boolean> {
  const open = openChatOf(agents, ref);
  if (open) {
    agents.select(open.key);
    nav.go('chat');

    return true;
  }

  const empty = emptyChatFor(agents, ref, project);
  const session = await startResumed(agents, ref, project, trustOnce, () => void resumeInChat(agents, ref, project, { trustOnce: true }));
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
 * Forks a saved session at an entry. A chat that already runs the session forks in place; otherwise the session is
 * resumed in a fresh agent first. Either way the history is reloaded so the chat shows the forked path. A refused
 * fork stops the agent that was started for it, never a chat the user already had open.
 */
export async function forkInChat(agents: AgentSessionsStore, ref: SessionRef, entryId: string, project: string, { trustOnce = false }: TrustOption = {}): Promise<boolean> {
  const open = openChatOf(agents, ref);
  if (open) {
    return forkOpenChat(agents, open, entryId);
  }

  const empty = emptyChatFor(agents, ref, project);
  const session = await startResumed(agents, ref, project, trustOnce, () => void forkInChat(agents, ref, entryId, project, { trustOnce: true }));
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

async function forkOpenChat(agents: AgentSessionsStore, chat: ChatSession, entryId: string): Promise<boolean> {
  const reply = await chat.send({ type: 'fork', entryId });
  if (!reply) {
    return false;
  }

  await chat.reloadHistory();
  agents.select(chat.key);
  nav.go('chat');

  return true;
}

/** Asks the user to confirm a deletion; resolves `true` when they do. */
export type Confirm = () => Promise<boolean>;

/** How a deletion ended: `skipped` is a copy that is gone or the backend no longer counts as empty, `open` a session a live agent holds. */
type Removal = 'deleted' | 'skipped' | 'open' | 'failed';

/** Deletes one saved session, telling the user when it fails; a refusal because an agent holds it is left to the caller to report. */
async function removeSession(ref: SessionRef, project: string, emptyCopyOnly: boolean): Promise<Removal> {
  try {
    await ipc.invoke('session_delete', { session: ref, project, emptyCopyOnly });

    return 'deleted';
  } catch (error) {
    const failure = IpcFailure.from(error);

    // The backend refuses a session a live agent holds, one a chat of this window may not know about yet.
    if (failure.code === 'forbidden') {
      return 'open';
    }

    // A copy that is already gone, or whose empty-copy check no longer holds (its parent went first, or it gained entries), is no failure to report.
    if (emptyCopyOnly && (failure.code === 'invalidInput' || failure.code === 'notFound')) {
      return 'skipped';
    }

    // A session other sessions were forked from (Codex) stays until those forks are deleted; the app never deletes them for the user.
    if (failure.code === 'conflict') {
      toasts.push({ title: t('sessions.delete.failed'), description: t('sessions.delete.hasForks'), tone: 'warning' });

      return 'failed';
    }

    toasts.push({ title: t('sessions.delete.failed'), description: failure.message, tone: 'danger' });

    return 'failed';
  }
}

function refuseOpenChat(): void {
  toasts.push({ title: t('sessions.delete.openInChat'), description: t('sessions.delete.openInChatBody'), tone: 'warning' });
}

/**
 * Deletes a saved session once the user confirms. A session an open chat runs is refused: the chat appends to its
 * file, so it has to be closed first.
 */
export async function deleteSession(agents: AgentSessionsStore, ref: SessionRef, project: string, confirm: Confirm): Promise<boolean> {
  if (openChatOf(agents, ref)) {
    refuseOpenChat();

    return false;
  }

  if (!(await confirm())) {
    return false;
  }

  const removal = await removeSession(ref, project, false);
  if (removal === 'open') {
    refuseOpenChat();

    return false;
  }

  if (removal !== 'deleted') {
    // The file may be gone although the call failed; the list says whether the row is a ghost.
    await sessionList.refresh();
    const listed = sessionList.data;
    if (viewState.sessionId === ref.id && listed !== null && !listed.some((summary) => summary.ref.id === ref.id)) {
      viewState.sessionId = null;
    }

    return false;
  }

  if (viewState.sessionId === ref.id) {
    viewState.sessionId = null;
  }
  await sessionList.refresh();

  return true;
}

/** The copies ordered so that every copy comes before the session it was forked from: a parent's deletion would void its children's empty-copy check. */
function childrenFirst(copies: SessionSummary[]): SessionSummary[] {
  const byId = new Map(copies.map((copy) => [copy.ref.id, copy]));

  // The number of copies above this one; the visited set keeps a malformed parent loop from spinning.
  const depth = (copy: SessionSummary): number => {
    const seen = new Set([copy.ref.id]);
    let parent = copy.parentSession ? byId.get(copy.parentSession) : undefined;
    let levels = 0;

    while (parent && !seen.has(parent.ref.id)) {
      seen.add(parent.ref.id);
      levels += 1;
      parent = parent.parentSession ? byId.get(parent.parentSession) : undefined;
    }

    return levels;
  };

  return copies
    .map((copy) => ({ copy, depth: depth(copy) }))
    .sort((a, b) => b.depth - a.depth)
    .map(({ copy }) => copy);
}

/**
 * Deletes the empty copies one after the other, deepest first, once the user confirms. A copy that is already gone, no
 * longer empty, or held by an agent (an open chat, or one the backend knows) is skipped and the rest carry on; only
 * another failure stops the batch. Resolves with how many were deleted; the toast also says how many stayed because a
 * chat has them open.
 */
export async function cleanUpCopies(agents: AgentSessionsStore, copies: SessionSummary[], project: string, confirm: Confirm): Promise<number> {
  const doomed = childrenFirst(copies.filter((copy) => !openChatOf(agents, copy.ref)));
  if (doomed.length === 0 || !(await confirm())) {
    return 0;
  }

  let deleted = 0;
  let kept = copies.length - doomed.length;
  let failed = false;
  for (const copy of doomed) {
    const removal = await removeSession(copy.ref, project, true);
    if (removal === 'skipped') {
      continue;
    }

    if (removal === 'open') {
      kept += 1;

      continue;
    }

    if (removal === 'failed') {
      failed = true;

      break;
    }

    deleted += 1;
    if (viewState.sessionId === copy.ref.id) {
      viewState.sessionId = null;
    }
  }

  await sessionList.refresh();

  const keptNote = kept > 0 ? tn('sessions.cleanup.keptOpen', kept) : undefined;
  if (deleted > 0) {
    toasts.push({ title: tn('sessions.cleanup.done', deleted), description: keptNote, tone: 'success' });
  } else if (keptNote && !failed) {
    toasts.push({ title: keptNote, tone: 'warning' });
  }

  return deleted;
}
