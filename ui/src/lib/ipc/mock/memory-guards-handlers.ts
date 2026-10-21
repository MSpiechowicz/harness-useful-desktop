import type { HeldItem } from '../bindings/HeldItem';
import type { HeldSaved } from '../bindings/HeldSaved';
import type { JsonValue } from '../bindings/serde_json/JsonValue';
import type { MemoryChatGuards } from '../bindings/MemoryChatGuards';
import type { MemoryChatStart } from '../bindings/MemoryChatStart';
import type { MemoryGuards } from '../bindings/MemoryGuards';
import type { MemoryGuardsPatch } from '../bindings/MemoryGuardsPatch';
import { IpcFailure } from '../transport';
import { PICKED_FOLDER, reheldRevision, SAMPLE_CHAT_KEY, sampleHeld } from './memory-guards-fixtures';
import type { Handlers } from './types';

type GuardCommands =
  | 'memory_guards_get'
  | 'memory_guards_update'
  | 'memory_chat_guards_set'
  | 'memory_held_list'
  | 'memory_held_save'
  | 'memory_held_skip'
  | 'memory_held_listen'
  | 'dialog_pick_folder';

/** What the held saves start as: none, the three samples, or the samples with an agent that holds the first one saved or skipped again just before the user's save or skip lands (`changed`). */
export type HeldScenario = 'empty' | 'ok' | 'changed';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

/** `?held=empty` starts without held saves, `?held=changed` has the agent hold the first save the user saves or skips again first; the default holds a note save, a note update and a script save. */
export function heldScenarioFromLocation(): HeldScenario {
  const held = typeof location !== 'undefined' ? new URLSearchParams(location.search).get('held') : null;

  return held === 'empty' || held === 'changed' ? held : 'ok';
}

function invalid(message: string): IpcFailure {
  return IpcFailure.from({ code: 'invalidInput', message });
}

function checkKey(key: string, what: string): void {
  if (!UUID.test(key)) {
    throw invalid(`${what} is a lowercase UUID`);
  }
}

/** The repository a folder belongs to, as the backend canonicalises it: the path without a trailing slash. */
function repoOf(path: string): string {
  if (!path.startsWith('/')) {
    throw invalid('a repository is an absolute folder path');
  }

  return path.length > 1 ? path.replace(/\/+$/, '') : path;
}

function sortedRepos(repos: string[]): string[] {
  return [...new Set(repos.map(repoOf))].sort();
}

/** `guards` with `patch` applied, as the backend applies it to what is saved now. */
function patched(guards: MemoryGuards, patch: MemoryGuardsPatch): MemoryGuards {
  const kind = guards[patch.kind];
  let offRepos = [...kind.offRepos];

  if (patch.removeRepo !== undefined) {
    const root = repoOf(patch.removeRepo);
    offRepos = offRepos.filter((repo) => repo !== patch.removeRepo && repo !== root);
  }

  if (patch.addRepo !== undefined) {
    offRepos.push(patch.addRepo);
  }

  return { ...guards, [patch.kind]: { paused: patch.paused ?? kind.paused, offRepos: sortedRepos(offRepos) } };
}

/**
 * The save guards and held saves of the mock backend, kept for the page's lifetime. The sample held saves belong to the first chat
 * started with a memory identity, so a preview chat shows its waiting cards.
 */
export class MockMemoryGuards {
  private global: MemoryGuards = { notes: { paused: false, offRepos: [] }, scripts: { paused: false, offRepos: [] } };
  private readonly chats = new Map<string, MemoryChatGuards>();
  /** The newest revision written for each chat's guards. */
  private readonly revisions = new Map<string, number>();
  /** The memory key of each running agent session. */
  private readonly agents = new Map<string, { key: string; project: string }>();
  private held: HeldItem[];
  private sampleOwner: string | null = null;
  private listener: (() => void) | null = null;
  /** `changed`: the agent holds the next save the user saves or skips again before that lands. */
  private holdAgainFirst: boolean;
  private reheld = 0;

  constructor(scenario: () => HeldScenario = heldScenarioFromLocation) {
    const start = scenario();

    this.held = start === 'empty' ? [] : sampleHeld();
    this.holdAgainFirst = start === 'changed';
  }

  /** The project of agent `agentKey`, while it runs with a memory identity. */
  projectOf(agentKey: string): string | null {
    return this.agents.get(agentKey)?.project ?? null;
  }

  /** The agent holds save `heldId` again, with a new body: its revision changes, as when it saves the same title again. */
  agentHoldsAgain(heldId: string): void {
    const item = this.held.find((candidate) => candidate.heldId === heldId);

    if (!item) {
      return;
    }

    this.reheld += 1;
    const args = item.arguments as Record<string, JsonValue>;
    item.arguments = { ...args, body: `${String(args.body ?? '')}\n\nChanged by the agent.` };
    item.revision = reheldRevision(this.reheld);
    this.changed();
  }

  /** `agent_start` started agent `agentKey` in `project` with `memoryChat`: its guards are written, and it may own the sample held saves. */
  chatStarted(agentKey: string, project: string, memoryChat: MemoryChatStart | null): void {
    if (!memoryChat) {
      return;
    }

    checkKey(memoryChat.key, 'a memory chat key');
    this.chats.set(memoryChat.key, structuredClone(memoryChat.guards));
    this.agents.set(agentKey, { key: memoryChat.key, project });

    if (this.sampleOwner === null) {
      this.sampleOwner = memoryChat.key;
      this.held = this.held.map((item) => (item.chatKey === SAMPLE_CHAT_KEY ? { ...item, chatKey: memoryChat.key } : item));
      this.changed();
    }
  }

  /** Whether the memory-save hook of agent `agentKey` is skipped: the chat paused, or notes and scripts both off for it, its project or everywhere. */
  skipsMemorySave(agentKey: string): boolean {
    const agent = this.agents.get(agentKey);
    const chat = agent ? this.chats.get(agent.key) : undefined;

    if (chat?.paused) {
      return true;
    }

    const off = (kind: 'notes' | 'scripts') => {
      const everywhere = this.global[kind];
      return chat?.[kind].on === false || everywhere.paused || (agent !== undefined && everywhere.offRepos.includes(agent.project));
    };

    return off('notes') && off('scripts');
  }

  handlers(): Pick<Handlers, GuardCommands> {
    return {
      memory_guards_get: () => structuredClone(this.global),
      memory_guards_update: ({ patch }) => {
        this.global = patched(this.global, patch);

        return structuredClone(this.global);
      },
      memory_chat_guards_set: ({ key, guards, revision }) => {
        checkKey(key, 'a memory chat key');

        const newest = this.revisions.get(key);
        if (newest !== undefined && revision <= newest) {
          return { applied: false, revision: newest };
        }

        this.chats.set(key, structuredClone(guards));
        this.revisions.set(key, revision);

        return { applied: true, revision };
      },
      memory_held_list: ({ chatKey }) => {
        checkKey(chatKey, 'a memory chat key');

        return structuredClone(this.held.filter((item) => item.chatKey === chatKey));
      },
      memory_held_save: ({ heldId, edited, revision }) => this.save(heldId, edited, revision),
      memory_held_skip: ({ heldId, revision }) => {
        this.settle(heldId, revision);
        return null;
      },
      memory_held_listen: ({ onEvent }) => {
        this.listener = () => onEvent({ kind: 'changed' });
        return null;
      },
      // Picks a fixed folder and changes nothing else (no project opened or remembered).
      dialog_pick_folder: () => PICKED_FOLDER,
    };
  }

  private save(heldId: string, edited: JsonValue | null, revision: string | null): HeldSaved {
    if (edited !== null && (typeof edited !== 'object' || Array.isArray(edited))) {
      throw invalid('an edited draft is an object');
    }

    const item = this.settle(heldId, revision);

    if (item.kind === 'script') {
      const name = (edited as { name?: unknown } | null)?.name ?? (item.arguments as { name?: unknown }).name;
      return { kind: 'script', name: String(name) };
    }

    const saved = item.current as { id?: number } | null;
    return { kind: 'note', id: saved?.id ?? 100 + this.held.length };
  }

  /** Takes held save `heldId` off the list for a save or skip at `revision` (when given): `changed` when the agent held it again since. */
  private settle(heldId: string, revision: string | null): HeldItem {
    if (revision !== null && !/^[0-9a-f]{64}$/.test(revision)) {
      throw invalid('a held save revision is 64 lowercase hex digits');
    }

    if (this.holdAgainFirst) {
      this.holdAgainFirst = false;
      this.agentHoldsAgain(heldId);
    }

    const current = this.held.find((candidate) => candidate.heldId === heldId);
    if (current && revision !== null && current.revision !== revision) {
      throw IpcFailure.from({ code: 'changed', message: 'the agent changed this save after you saw it, so nothing was saved' });
    }

    return this.take(heldId);
  }

  private take(heldId: string): HeldItem {
    checkKey(heldId, 'a held save id');

    const index = this.held.findIndex((item) => item.heldId === heldId);
    if (index === -1) {
      throw IpcFailure.from({ code: 'notFound', message: 'that held save is gone' });
    }

    const [item] = this.held.splice(index, 1);
    this.changed();

    return item;
  }

  private changed(): void {
    this.listener?.();
  }
}
