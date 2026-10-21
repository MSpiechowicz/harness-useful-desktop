import type { KindGuard } from '$lib/ipc/bindings/KindGuard';
import type { MemoryChatGuards } from '$lib/ipc/bindings/MemoryChatGuards';
import type { MemoryGuards } from '$lib/ipc/bindings/MemoryGuards';

/** The two kinds of thing an agent can save. */
export type GuardKind = 'notes' | 'scripts';

export const GUARD_KINDS: readonly GuardKind[] = ['notes', 'scripts'];

/** A chat's memory identity: the key Harness Memory knows it by and what its agents may save. */
export interface ChatMemory {
  key: string;
  guards: MemoryChatGuards;
}

/** Where saving a kind is switched off, the widest place first. */
export type OffReason = 'global' | 'repo' | 'chat';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

/** A new chat's guards: everything on, "Ask before saving" as the preferences say. */
export function defaultGuards(ask: Record<GuardKind, boolean>): MemoryChatGuards {
  return { paused: false, notes: { on: true, ask: ask.notes }, scripts: { on: true, ask: ask.scripts } };
}

export const NO_GLOBAL_GUARDS: MemoryGuards = { notes: { paused: false, offRepos: [] }, scripts: { paused: false, offRepos: [] } };

function isKindGuard(value: unknown): value is KindGuard {
  const guard = value as KindGuard | null;

  return typeof guard?.on === 'boolean' && typeof guard.ask === 'boolean';
}

/** A chat memory read back from storage, or null for anything that is not one (the record is a convenience, a bad one is dropped). */
export function readChatMemory(value: unknown): ChatMemory | null {
  const memory = value as ChatMemory | null;

  if (typeof memory?.key !== 'string' || !UUID.test(memory.key)) {
    return null;
  }

  const guards = memory.guards as MemoryChatGuards | null;

  if (typeof guards?.paused !== 'boolean' || !isKindGuard(guards.notes) || !isKindGuard(guards.scripts)) {
    return null;
  }

  return { key: memory.key, guards: { paused: guards.paused, notes: { ...guards.notes }, scripts: { ...guards.scripts } } };
}

/** A lowercase UUID v4 for a new chat. */
export function newChatKey(): string {
  return crypto.randomUUID().toLowerCase();
}

/** The guards with one kind's switches changed. */
export function withKind(guards: MemoryChatGuards, kind: GuardKind, change: Partial<KindGuard>): MemoryChatGuards {
  return { ...guards, [kind]: { ...guards[kind], ...change } };
}

/** Why a kind cannot be saved in a chat in repository `root`, or null when it can: paused everywhere, off in the repository, or off in the chat. */
export function offReason(kind: GuardKind, chat: MemoryChatGuards, global: MemoryGuards, root: string | null): OffReason | null {
  if (global[kind].paused) {
    return 'global';
  }

  if (root !== null && global[kind].offRepos.includes(root)) {
    return 'repo';
  }

  return chat[kind].on ? null : 'chat';
}

/** Whether nothing of either kind can be saved in the chat, so pausing and checking again have nothing to act on. */
export function everythingOff(chat: MemoryChatGuards, global: MemoryGuards, root: string | null): boolean {
  return GUARD_KINDS.every((kind) => offReason(kind, chat, global, root) !== null);
}
