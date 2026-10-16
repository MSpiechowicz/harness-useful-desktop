import type { ChatLabel } from '$lib/chat/naming';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { isHarnessId } from '$lib/ipc/harness-ids';

import { type AgentSessionsStore, resumable } from './agentSession.svelte';
import { harness } from './harness.svelte';
import { nav, VIEW_IDS, type ViewId } from './nav.svelte';

/**
 * Where the user left off: the open chats that have a conversation, the one on screen, and the page. Kept in the
 * webview's storage (a per-device convenience); a missing or unreadable record just means a fresh start.
 */
interface SavedWorkspace {
  view: ViewId;
  chats: { harness: HarnessId; sessionId: string; project: string; /** The helper's name and description, and whether the user renamed the chat since. */ label?: ChatLabel | null; userNamed?: boolean }[];
  /** Index into `chats` of the chat that was showing, or -1. */
  active: number;
}

const KEY = 'harness:workspace';
/** More than this many restored agents at once would slow the start down; the rest stay in Sessions. */
const MAX_RESTORED = 6;

export function saveWorkspace(agents: AgentSessionsStore): void {
  const open = agents.sessions.flatMap((session) => {
    const ref = session.ended ? null : resumable(session);

    return ref ? [{ session, ref }] : [];
  });
  const record: SavedWorkspace = {
    view: nav.current,
    chats: open.map(({ session, ref }) => ({ harness: ref.harness, sessionId: ref.id, project: session.info.project, label: session.aiLabel, userNamed: session.userNamed })),
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
  return a.harness === b.harness && a.sessionId === b.sessionId;
}

/** Keeps the first chat of each session: two agents must never run (and append to) one session. */
function uniqueSessions(chats: SavedWorkspace['chats']): SavedWorkspace['chats'] {
  return chats.filter((chat, index) => chats.findIndex((other) => sameSession(other, chat)) === index);
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
        .map((chat) => ({ ...chat, label: isLabel(chat.label) ? chat.label : null, userNamed: chat.userNamed === true })),
    );

    return {
      view: VIEW_IDS.includes(record.view) ? record.view : 'chat',
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

    const chats = saved.chats.slice(0, MAX_RESTORED).map((chat) => ({ harness: chat.harness, project: chat.project, sessionId: chat.sessionId, label: chat.label ?? null, userNamed: chat.userNamed ?? false }));

    return (await agents.restore(chats, saved.active)) > 0;
  })().finally(() => {
    restoreSettled = true;
  });

  return restoring;
}
