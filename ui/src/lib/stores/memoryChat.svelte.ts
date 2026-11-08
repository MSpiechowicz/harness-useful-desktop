import { SvelteMap } from 'svelte/reactivity';

import { t } from '$lib/i18n/i18n.svelte';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { MemoryChatGuards } from '$lib/ipc/bindings/MemoryChatGuards';
import type { MemoryChatStart } from '$lib/ipc/bindings/MemoryChatStart';
import type { MemoryGuardsPatch } from '$lib/ipc/bindings/MemoryGuardsPatch';
import type { MemoryGuards } from '$lib/ipc/bindings/MemoryGuards';
import type { SessionRef } from '$lib/ipc/bindings/SessionRef';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { harness } from './harness.svelte';
import {
  type ChatMemory,
  defaultGuards,
  type GuardKind,
  newChatKey,
  NO_GLOBAL_GUARDS,
  readChatMemory,
  withKind,
} from './memory-chat';
import { memoryRemote } from './memoryRemote.svelte';
import { prefs } from './prefs.svelte';
import { toasts, type ToastsStore } from './toasts.svelte';

export type { ChatMemory } from './memory-chat';

/** What decides whether a chat gets a memory identity, and the "Ask before saving" a new one starts with. */
export interface MemoryChatEnv {
  /** Resolves once the facts below are known. Never rejects: what cannot be read counts as not supported. */
  ready(): Promise<void>;
  /** Whether chats of the harness carry an identity: the harness passes it on, Harness Memory applies guards, and project memory is on. */
  supports(harnessId: HarnessId): boolean;
  askDefaults(): Record<GuardKind, boolean>;
}

let loadingRemote: Promise<void> | null = null;

/** The app's own facts: the harness list, the installed Harness Memory and the preferences. */
export const appEnv: MemoryChatEnv = {
  async ready() {
    if (memoryRemote.value === null) {
      loadingRemote ??= memoryRemote.load().finally(() => {
        loadingRemote = null;
      });
      await loadingRemote;
    }
  },

  supports(harnessId) {
    return harness.get(harnessId)?.capabilities.memoryChatGuards === true && memoryRemote.value?.saveGuardsSupported === true && prefs.value?.memoryEnabled !== false;
  },

  askDefaults() {
    return { notes: prefs.value?.memoryNotesAskDefault ?? false, scripts: prefs.value?.memoryScriptsAskDefault ?? false };
  },
};

/** How often the guards are read again while they could not be read or the old file is still in use, and a view shows them. */
export const GLOBAL_RETRY_MS = 30_000;

const SESSIONS_KEY = 'harness:memory-chats';
/** More resumed sessions than this are not remembered: the oldest are forgotten first. */
const MAX_REMEMBERED = 200;

function sessionKey(harnessId: HarnessId, sessionId: string): string {
  return `${harnessId}:${sessionId}`;
}

function readRemembered(storage: Storage | null): Map<string, ChatMemory> {
  const remembered = new Map<string, ChatMemory>();

  try {
    const record = JSON.parse(storage?.getItem(SESSIONS_KEY) ?? 'null') as Record<string, unknown> | null;

    for (const [id, value] of Object.entries(record ?? {})) {
      const memory = readChatMemory(value);

      if (memory) {
        remembered.set(id, memory);
      }
    }
  } catch {
    // Storage can be unavailable or hold something else: a session then simply gets a new key.
  }

  return remembered;
}

/** Where a new agent's identity comes from: the chat it replaces, else the session it resumes, else a new one. */
export interface LaunchSource {
  resume?: SessionRef | null;
  /** The identity of the chat this agent replaces (a restart) or brings back (the last run's workspace). */
  keep?: ChatMemory | null;
}

/**
 * What agents may save, per chat and everywhere. Every chat has a random key that Harness Memory knows it by and that
 * its harness gets as `HARNESS_MEMORY_SESSION`, kept across restarts and resumes, with the chat's own guards. The
 * global guards (paused, on for every project, on in a repository) are read from Harness Memory's files and written back as they change.
 */
export class MemoryChatStore {
  /** The identity of each running agent, by its session key. */
  private chats = new SvelteMap<string, ChatMemory>();
  /** The identity of each session the user may resume, by `harness:sessionId`, oldest first. */
  private remembered: Map<string, ChatMemory>;
  private reviews = new SvelteMap<string, number>();
  /** The newest revision written for each chat key's guards: every write is higher, so the backend can drop a late one. */
  private revisions = new Map<string, number>();

  /** The saving root of each folder a chat works in, as `memory_saving_root` answered. A folder without one (gone) is not kept, so it is asked again at the next look. */
  private roots = new SvelteMap<string, string>();
  /** The folders whose root is being asked for. */
  private asking = new Set<string>();
  /** Rises whenever the guards change: an answer asked for before that is out of date. */
  private rootGeneration = 0;
  /** The folders whose root was answered before the guards last changed: the next look at one asks again (and still shows the old answer). */
  private staleRoots = new Set<string>();
  /** Rises with `rootGeneration`, so whatever reads a root looks again. */
  private rootsChanged = $state(0);

  /** What every chat allows; null until it was read (or when it cannot be). */
  global = $state.raw<MemoryGuards | null>(null);
  /** The last read of the global guards failed: the server refuses saves while it cannot read them. */
  globalFailed = $state(false);
  /** Rises with every update of the global guards that went through: a read that started before one may hold an older state and is dropped. */
  private globalGeneration = 0;
  /** The read that is under way: another look at the guards waits for it instead of starting a second. */
  private reading: Promise<void> | null = null;

  /** How many views keep the global guards current, and the one timer they share while a read keeps failing or the old file is in use. */
  private watchers = 0;
  private retry: ReturnType<typeof setInterval> | null = null;

  constructor(
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
    private readonly env: MemoryChatEnv,
    private readonly storage: Storage | null = typeof localStorage === 'undefined' ? null : localStorage,
  ) {
    this.remembered = readRemembered(storage);
  }

  /**
   * The root a chat working in `folder` is listed under in the global guards: a git worktree maps to its main repository, a
   * separate clone stays separate. Asked once per folder, and again at the first look after the global guards changed (the answer
   * depends on the projects listed), showing the old answer until the new one is in, so a switch does not blink. Null while it is not
   * known and for a folder that is gone, which counts as not turned on. A null answer is not kept, so the next look asks again and a
   * folder that comes back is found.
   */
  rootOf(folder: string): string | null {
    if (folder === '') {
      return null;
    }

    // Looks again when the guards changed, so a stale root is asked for by whatever shows it, and only by that.
    void this.rootsChanged;

    const known = this.roots.get(folder);

    if (known === undefined || this.staleRoots.has(folder)) {
      void this.askRoot(folder);
    }

    return known ?? null;
  }

  private async askRoot(folder: string): Promise<void> {
    if (this.asking.has(folder)) {
      return;
    }

    this.asking.add(folder);

    try {
      for (;;) {
        const generation = this.rootGeneration;
        this.staleRoots.delete(folder);
        const root = await this.transport.invoke('memory_saving_root', { folder });

        // The guards changed while it was asked: that answer may be out of date, so ask once more.
        if (generation !== this.rootGeneration) {
          continue;
        }

        if (root === null) {
          this.roots.delete(folder);
        } else {
          this.roots.set(folder, root);
        }

        return;
      }
    } catch {
      // Not known stays off, and the next look at the folder asks again (a root known stays as it was until then).
      if (this.roots.has(folder)) {
        this.staleRoots.add(folder);
      }
    } finally {
      this.asking.delete(folder);
    }
  }

  /** The roots depend on the guards, which changed: a root known is asked again at its next look, and shown as it was until then. */
  private markRootsStale(): void {
    this.rootGeneration += 1;
    this.staleRoots = new Set(this.roots.keys());
    this.rootsChanged += 1;
  }

  /** The identity of a running agent; null for a chat without one (not supported, or started before it was). */
  get(agentKey: string): ChatMemory | null {
    return this.chats.get(agentKey) ?? null;
  }

  /** The memory keys of the running chats. */
  keys(): string[] {
    return [...this.chats.values()].map((memory) => memory.key);
  }

  /**
   * Whether the installed Harness Memory applies the opt-in guards, so the Memory button and the Settings cards have switches that are
   * honoured. A server with guards but without opt-in refuses the new guards file, so it counts as not supported here (chats still
   * carry an identity, see `appEnv.supports`).
   */
  get supported(): boolean {
    return memoryRemote.value?.saveGuardsSupported === true && memoryRemote.value.saveOptInSupported;
  }

  /**
   * The identity for an agent about to start, or null when its chat gets none. Waits for what decides that, so the first
   * chat of a launch does not start before the installed Harness Memory is known.
   */
  async launch(harnessId: HarnessId, source: LaunchSource = {}): Promise<MemoryChatStart | null> {
    await this.env.ready();

    if (!this.env.supports(harnessId)) {
      return null;
    }

    const resumed = source.resume ? this.remembered.get(sessionKey(source.resume.harness, source.resume.id)) : undefined;
    const known = source.keep ?? resumed;

    return known ? { key: known.key, guards: structuredClone(known.guards) } : { key: newChatKey(), guards: defaultGuards(this.env.askDefaults()) };
  }

  /** The agent started with `start` (null for a chat without one): its chat now has that identity. */
  attach(agentKey: string, start: MemoryChatStart | null, resume: SessionRef | null = null): void {
    if (!start) {
      return;
    }

    this.chats.set(agentKey, { key: start.key, guards: start.guards });

    if (resume) {
      this.remember(agentKey, resume.harness, resume.id);
    }
  }

  /** The agent stopped. The session keeps its identity for a later resume. */
  release(agentKey: string): void {
    this.chats.delete(agentKey);
    this.reviews.delete(agentKey);
  }

  /** The session `sessionId` of agent `agentKey` keeps this chat's identity when it is resumed, even in another run. */
  remember(agentKey: string, harnessId: HarnessId, sessionId: string): void {
    const memory = this.chats.get(agentKey);

    if (!memory) {
      return;
    }

    const id = sessionKey(harnessId, sessionId);
    const known = this.remembered.get(id);

    if (known && known.key === memory.key && JSON.stringify(known.guards) === JSON.stringify(memory.guards)) {
      return;
    }

    this.remembered.delete(id);
    this.remembered.set(id, { key: memory.key, guards: structuredClone(memory.guards) });

    while (this.remembered.size > MAX_REMEMBERED) {
      this.remembered.delete(this.remembered.keys().next().value as string);
    }

    try {
      this.storage?.setItem(SESSIONS_KEY, JSON.stringify(Object.fromEntries(this.remembered)));
    } catch {
      // A convenience: without it a resumed session starts with the preferences' defaults.
    }
  }

  /** Changes what the agents of a chat may save. Rolled back, with a toast, when Harness Memory cannot be told. True when it was told. */
  async setGuards(agentKey: string, change: (guards: MemoryChatGuards) => MemoryChatGuards): Promise<boolean> {
    const memory = this.chats.get(agentKey);

    if (!memory) {
      return false;
    }

    const before = memory.guards;
    const next = change(before);
    this.chats.set(agentKey, { ...memory, guards: next });

    try {
      const result = await this.transport.invoke('memory_chat_guards_set', { key: memory.key, guards: next, revision: this.nextRevision(memory.key) });

      if (result && !result.applied) {
        // A write of a higher revision came first (an earlier run's): count on from above it and write what the chat allows now, once.
        const guards = this.chats.get(agentKey)?.guards ?? next;

        await this.transport.invoke('memory_chat_guards_set', { key: memory.key, guards, revision: this.nextRevision(memory.key, result.revision) });
      }

      return true;
    } catch (error) {
      if (this.chats.get(agentKey)?.guards === next) {
        this.chats.set(agentKey, { ...memory, guards: before });
      }

      this.failed(error);

      return false;
    }
  }

  /** Rises with every write of a key, and starts above any earlier run's (the clock), so a restart continues above the last value. */
  private nextRevision(key: string, above = 0): number {
    const revision = Math.max(Date.now(), (this.revisions.get(key) ?? 0) + 1, above + 1);

    this.revisions.set(key, revision);

    return revision;
  }

  setKind(agentKey: string, kind: GuardKind, change: Partial<MemoryChatGuards[GuardKind]>): Promise<boolean> {
    return this.setGuards(agentKey, (guards) => withKind(guards, kind, change));
  }

  /** Pauses or resumes saving in a chat, and says so with a toast once Harness Memory was told. */
  async setPaused(agentKey: string, paused: boolean): Promise<void> {
    if (await this.setGuards(agentKey, (guards) => ({ ...guards, paused }))) {
      this.notifications.push(paused ? { title: t('memory.guards.toast.paused'), description: t('memory.guards.pausedNote') } : { title: t('memory.guards.toast.resumed'), description: t('memory.guards.toast.resumedNote'), tone: 'success' });
    }
  }

  /** "Check again": asks the chat's agent to save what it learned now. Says why, with a toast, when it cannot. */
  async checkAgain(agentKey: string): Promise<boolean> {
    try {
      await this.transport.invoke('chat_memory_check_again', { agentId: agentKey });

      return true;
    } catch (error) {
      this.notifications.push({ title: t('memory.guards.check.failed'), description: IpcFailure.from(error).message, tone: 'danger' });

      return false;
    }
  }

  /** Asks the review of the chat's waiting saves to be brought into view. */
  requestReview(agentKey: string): void {
    this.reviews.set(agentKey, (this.reviews.get(agentKey) ?? 0) + 1);
  }

  /** Counts the requests to review: the chat scrolls its waiting card into view whenever it changes. */
  reviewRequests(agentKey: string): number {
    return this.reviews.get(agentKey) ?? 0;
  }

  /**
   * Reads what every chat allows. A failure leaves what was known (or nothing) and is remembered for the cards to say. A read already under
   * way is joined, not repeated. A read that started before an update is dropped, so it cannot bring back what the update changed.
   */
  loadGlobal(): Promise<void> {
    this.reading ??= this.read().finally(() => {
      this.reading = null;
    });

    return this.reading;
  }

  private async read(): Promise<void> {
    const generation = this.globalGeneration;

    try {
      const next = await this.transport.invoke('memory_guards_get', undefined);

      if (generation === this.globalGeneration) {
        this.take(next, false);
        this.globalFailed = false;
      }
    } catch {
      if (generation === this.globalGeneration) {
        this.globalFailed = true;
      }
    }

    this.syncRetry();
  }

  /**
   * A view that shows the global guards keeps them current: they are read now, and again every `GLOBAL_RETRY_MS` while a read
   * failed or the old file is still in use. All views share one timer. Returns the function that stops watching.
   */
  watchGlobal(): () => void {
    let watching = true;

    this.watchers += 1;
    void this.loadGlobal();

    return () => {
      if (!watching) {
        return;
      }

      watching = false;
      this.watchers -= 1;
      this.syncRetry();
    };
  }

  /** The timer runs only while someone watches and a read keeps failing or the old file is still in use. */
  private syncRetry(): void {
    const needed = this.watchers > 0 && (this.globalFailed || this.global?.migrationPending === true);

    if (needed && this.retry === null) {
      this.retry = setInterval(() => void this.loadGlobal(), GLOBAL_RETRY_MS);
    } else if (!needed && this.retry !== null) {
      clearInterval(this.retry);
      this.retry = null;
    }
  }

  /**
   * Takes the guards as the backend answered. The roots are asked again when the answer differs from what was known (or `changed`:
   * after an update), and a read of equal content keeps the object, so nothing that reads it runs again.
   */
  private take(next: MemoryGuards, changed: boolean): void {
    const previous = this.global;
    const differs = previous !== null && JSON.stringify(previous) !== JSON.stringify(next);

    if (previous === null || differs) {
      this.global = next;
    }

    if (changed || differs) {
      this.markRootsStale();
    }
  }

  /** What every chat allows, all off while it was not read yet (saving is opt-in). */
  get guards(): MemoryGuards {
    return this.global ?? NO_GLOBAL_GUARDS;
  }

  /** One change to the global guards, applied by the backend to what is saved now. The answer is the new state. */
  async updateGlobal(patch: MemoryGuardsPatch): Promise<void> {
    try {
      const next = await this.transport.invoke('memory_guards_update', { patch });

      // A read that started before this answer may hold the state from before the change.
      this.globalGeneration += 1;
      this.take(next, true);
      this.globalFailed = false;
      this.syncRetry();
    } catch (error) {
      this.failed(error);
    }
  }

  setGlobalPause(kind: GuardKind, paused: boolean): Promise<void> {
    return this.updateGlobal({ kind, paused });
  }

  /** Saving a kind in a repository (its saving root, or any folder in it) is turned on, or off again. */
  setRepoOn(kind: GuardKind, repo: string, on: boolean): Promise<void> {
    return this.updateGlobal(on ? { kind, addRepo: repo } : { kind, removeRepo: repo });
  }

  /** Saving a kind in every project, including ones opened later, is turned on or back to the listed projects only. */
  setEveryProject(kind: GuardKind, on: boolean): Promise<void> {
    return this.updateGlobal({ kind, everyProject: on });
  }

  /** Turns saving on in a repository by a folder the user picked; the backend stores its saving root. */
  addRepo(kind: GuardKind, folder: string): Promise<void> {
    return this.setRepoOn(kind, folder, true);
  }

  private failed(error: unknown): void {
    this.notifications.push({ title: t('memory.guards.toast.failed'), description: IpcFailure.from(error).message, tone: 'danger' });
  }
}

export const memoryChat = new MemoryChatStore(ipc, toasts, appEnv);
