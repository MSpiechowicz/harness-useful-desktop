import type { ChatLabel } from '$lib/chat/naming';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { SessionRoutineTag } from '$lib/ipc/bindings/SessionRoutineTag';
import { isHarnessId } from '$lib/ipc/harness-ids';

import { type AgentSessionsStore, resumable } from './agentSession.svelte';
import { harness } from './harness.svelte';
import type { ChatMemory } from './memoryChat.svelte';
import { readChatMemory } from './memory-chat';
import { nav, VIEW_IDS, type ViewId } from './nav.svelte';

/**
 * Where the user left off: the open chats that have a conversation, the one on screen, and the page. Kept in the
 * webview's storage (a per-device convenience); a missing or unreadable record just means a fresh start.
 */
interface SavedWorkspace {
  view: ViewId;
  chats: {
    harness: HarnessId;
    /** Empty for the window of a routine run that has no saved conversation yet. */
    sessionId: string;
    project: string;
    /** The helper's name and description, and whether the user renamed the chat since. */
    label?: ChatLabel | null;
    userNamed?: boolean;
    /** The run the chat is the live window of, to look at it again after a reload. */
    routine?: SessionRoutineTag | null;
    /** The chat's memory key and guards, so the reopened chat is the same one for Harness Memory. */
    memory?: ChatMemory | null;
  }[];
  /** Index into `chats` of the chat that was showing, or -1. */
  active: number;
}

const KEY = 'harness:workspace';
/** More than this many restored agents at once would slow the start down; the rest stay in Sessions. */
const MAX_RESTORED = 6;

export function saveWorkspace(agents: AgentSessionsStore): void {
  const open = agents.sessions.flatMap((session) => {
    const ref = session.ended ? null : resumable(session);

    return ref || (session.routine && !session.ended) ? [{ session, ref }] : [];
  });
  const record: SavedWorkspace = {
    view: nav.current,
    chats: open.map(({ session, ref }) => {
      // A conversation that can be resumed from the Sessions list keeps its memory key and guards there too.
      if (ref) {
        agents.memory.remember(session.key, ref.harness, ref.id);
      }

      return {
        harness: session.info.harness,
        sessionId: ref?.id ?? '',
        project: session.info.project,
        label: session.aiLabel,
        userNamed: session.userNamed,
        routine: session.routine,
        memory: agents.memory.get(session.key),
      };
    }),
    active: open.findIndex(({ session }) => session.key === agents.activeKey),
  };

  try {
    localStorage.setItem(KEY, JSON.stringify(record));
  } catch {
    // Storage can be unavailable; restoring is a convenience.
  }
}

function isLabel(value: unknown): value is ChatLabel {
  const label = value as ChatLabel | null;

  return typeof label?.name === 'string' && label.name !== '' && typeof label.description === 'string';
}

function sameSession(a: SavedWorkspace['chats'][number], b: SavedWorkspace['chats'][number]): boolean {
  if (a.sessionId === '' || b.sessionId === '') {
    return a.routine?.runId !== undefined && a.routine.runId === b.routine?.runId;
  }

  return a.harness === b.harness && a.sessionId === b.sessionId;
}

function isRoutine(value: unknown): value is SessionRoutineTag {
  const tag = value as SessionRoutineTag | null;

  return typeof tag?.runId === 'string' && typeof tag.routineId === 'string' && typeof tag.name === 'string';
}

/** Keeps the first chat of each session: two agents must never run (and append to) one session. */
function uniqueSessions(chats: SavedWorkspace['chats']): SavedWorkspace['chats'] {
  return chats.filter((chat, index) => chats.findIndex((other) => sameSession(other, chat)) === index);
}

/** The view a record names. The Memory view was split into Notes and Scripts, and a record from before that opens Notes. */
function savedView(view: string): ViewId {
  if (view === 'memory') {
    return 'notes';
  }

  return VIEW_IDS.find((id) => id === view) ?? 'chat';
}

function readWorkspace(): SavedWorkspace | null {
  try {
    const record = JSON.parse(localStorage.getItem(KEY) ?? 'null') as SavedWorkspace | null;
    if (!record || !Array.isArray(record.chats)) {
      return null;
    }

    const wanted = typeof record.active === 'number' ? record.chats[record.active] : undefined;
    const chats = uniqueSessions(
      record.chats
        .filter((chat) => typeof chat?.sessionId === 'string' && typeof chat.project === 'string' && isHarnessId(chat.harness) && harness.canChat(chat.harness))
        .map((chat) => ({ ...chat, label: isLabel(chat.label) ? chat.label : null, userNamed: chat.userNamed === true, routine: isRoutine(chat.routine) ? chat.routine : null, memory: readChatMemory(chat.memory) }))
        .filter((chat) => chat.sessionId !== '' || chat.routine !== null),
    );

    return {
      view: savedView(record.view),
      chats,
      // The index counted the chats as saved; find the same session among those kept.
      active: wanted ? chats.findIndex((chat) => sameSession(chat, wanted)) : -1,
    };
  } catch {
    return null;
  }
}

let restoring: Promise<boolean> | null = null;
/** Saving waits for the restore: the empty start of a launch must not overwrite what is being restored. */
let restoreSettled = false;

/** Saves the workspace once the launch's restore has run (see [`restoreWorkspace`]). */
export function saveWorkspaceWhenReady(agents: AgentSessionsStore): void {
  if (restoreSettled) {
    saveWorkspace(agents);
  }
}

/**
 * Reopens the chats and the page from the last run, once per launch. Resolves true when at least one chat came back
 * (the caller then skips starting a fresh one).
 */
export function restoreWorkspace(agents: AgentSessionsStore): Promise<boolean> {
  restoring ??= (async () => {
    const saved = readWorkspace();
    if (!saved || saved.chats.length === 0) {
      if (saved && saved.view !== 'chat') {
        nav.go(saved.view);
      }

      return false;
    }

    if (saved.view !== 'chat') {
      nav.go(saved.view);
    }

    const chats = saved.chats.slice(0, MAX_RESTORED).map((chat) => ({ harness: chat.harness, project: chat.project, sessionId: chat.sessionId, label: chat.label ?? null, userNamed: chat.userNamed ?? false, routine: chat.routine ?? null, memory: chat.memory ?? null }));

    return (await agents.restore(chats, saved.active)) > 0;
  })().finally(() => {
    restoreSettled = true;
  });

  return restoring;
}
