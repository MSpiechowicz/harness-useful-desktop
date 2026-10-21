import type { MessageKey } from "$lib/i18n/locales/en";
import {
  GUARD_KINDS,
  offReason,
  type ChatMemory,
  type GuardKind,
  type OffReason,
} from "$lib/stores/memory-chat";
import { memoryChat } from "$lib/stores/memoryChat.svelte";
import { memoryHeld, type HeldOutcome } from "$lib/stores/memoryHeld.svelte";
import { agentSessions } from "$lib/stores/agentSession.svelte";
import type { HeldItem } from "$lib/ipc/bindings/HeldItem";

import { repositoryOf } from "./memory-repository";

/** What a chat says about memory under its last turn. */
export interface ChatMemoryState {
  /** The saves waiting for the user's review. */
  waiting: HeldItem[];
  /** What the user saved or skipped from the card lately. */
  outcomes: HeldOutcome[];
  /** Saving is paused in this chat. */
  paused: boolean;
  /** What each kind does here (every kind saves while paused: that is said on its own). */
  kinds: Record<GuardKind, KindState>;
  /** What the chat's repository is called, for the line that says it is off there. */
  repoName: string;
}

/** What happens to a kind a chat's agent wants to save. */
export interface KindState {
  /** Where saving is switched off, or null when it is not. */
  off: OffReason | null;
  /** The agent saves only what the user reviews. */
  ask: boolean;
}

/** One sentence of the line a chat says about saving: a message key and what it names. */
export interface LineSentence {
  key: MessageKey;
  params?: Record<string, string>;
}

/** What happens to `kind` in the chat: where it is off, else whether the user reviews it. A paused chat is not asked about, it says so on its own. */
function kindState(
  memory: ChatMemory,
  kind: GuardKind,
  root: string | null,
): KindState {
  const off = memory.guards.paused
    ? null
    : offReason(kind, memory.guards, memoryChat.guards, root);

  return {
    off,
    ask: !memory.guards.paused && off === null && memory.guards[kind].ask,
  };
}

/** The memory state of the chat whose agent is `agentKey`; null for a chat without a memory identity. Reads the stores, so call it where a change should show. */
export function chatMemoryState(agentKey: string): ChatMemoryState | null {
  const memory = memoryChat.get(agentKey);

  if (!memory) {
    return null;
  }

  const repository = repositoryOf(
    agentSessions.get(agentKey)?.info.project ?? "",
  );
  const paused = memory.guards.paused;
  const root = repository?.root ?? null;
  const kinds = {
    notes: kindState(memory, "notes", root),
    scripts: kindState(memory, "scripts", root),
  };

  return {
    waiting: memoryHeld.of(memory.key),
    outcomes: memoryHeld.outcomesOf(memory.key),
    paused,
    kinds,
    repoName: repository?.name ?? "",
  };
}

/**
 * Whether the chat has anything to show under its last turn, so the list pins a row for it. That the chat is paused or off is said only
 * once there is a conversation to say it under: an empty chat has its own invitation to write.
 */
export function hasMemoryRow(agentKey: string, conversation: boolean): boolean {
  const state = chatMemoryState(agentKey);

  if (state === null) {
    return false;
  }

  return (
    state.waiting.length > 0 ||
    state.outcomes.length > 0 ||
    (conversation && (state.paused || memoryLine(state).length > 0))
  );
}

/** The sentence that names why saving `kind` is off beyond the chat itself; null when the chat is the reason. */
function reasonOf(
  kind: GuardKind,
  state: ChatMemoryState,
): LineSentence | null {
  const off = state.kinds[kind].off;

  if (off === "repo") {
    return {
      key:
        kind === "notes"
          ? "memory.guards.line.reason.notes.repo"
          : "memory.guards.line.reason.scripts.repo",
      params: { repo: state.repoName },
    };
  }

  if (off === "global") {
    return {
      key:
        kind === "notes"
          ? "memory.guards.line.reason.notes.global"
          : "memory.guards.line.reason.scripts.global",
    };
  }

  return null;
}

/** The sentences of one line about what the chat saves, empty when everything saves normally. A paused chat says so on its own line. */
export function memoryLine(state: ChatMemoryState): LineSentence[] {
  const { notes, scripts } = state.kinds;

  if (notes.off && scripts.off) {
    return [
      { key: "memory.guards.line.off" } satisfies LineSentence,
      reasonOf("notes", state),
      reasonOf("scripts", state),
    ].filter((sentence) => sentence !== null);
  }

  const sentences: Array<LineSentence | null> = [];

  for (const kind of GUARD_KINDS) {
    if (state.kinds[kind].off) {
      sentences.push(
        {
          key:
            kind === "notes"
              ? "memory.guards.line.noNotes"
              : "memory.guards.line.noScripts",
        },
        reasonOf(kind, state),
      );
    }
  }

  if (notes.ask && scripts.ask) {
    sentences.push({ key: "memory.guards.line.ask.both" });
  } else if (notes.ask) {
    sentences.push({
      key: scripts.off
        ? "memory.guards.line.wait.notes"
        : "memory.guards.line.ask.notes",
    });
  } else if (scripts.ask) {
    sentences.push({
      key: notes.off
        ? "memory.guards.line.wait.scripts"
        : "memory.guards.line.ask.scripts",
    });
  }

  return sentences.filter((sentence) => sentence !== null);
}
