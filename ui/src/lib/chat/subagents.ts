import type { Subagent } from '$lib/ipc/bindings/Subagent';

import type { ChatState } from './types';

/** A newer snapshot over an older one: fields the new frame did not carry (lifecycle frames carry no progress) keep their value. */
export function mergeSubagent(previous: Subagent | undefined, next: Subagent): Subagent {
  if (!previous) {
    return next;
  }

  const merged = { ...previous } as Record<string, unknown>;
  for (const [field, value] of Object.entries(next)) {
    if (value !== null && value !== undefined) {
      merged[field] = value;
    }
  }

  return merged as unknown as Subagent;
}

export function applySubagent(state: ChatState, subagent: Subagent): ChatState {
  return { ...state, subagents: { ...state.subagents, [subagent.id]: mergeSubagent(state.subagents[subagent.id], subagent) } };
}

export function isSubagentActive(subagent: Subagent): boolean {
  return subagent.status === 'pending' || subagent.status === 'running';
}

/** How many of the session's subagents are still pending or running (the tab's busy count). */
export function activeSubagentCount(state: ChatState): number {
  return Object.values(state.subagents).filter(isSubagentActive).length;
}

/** The subagents a `task` call spawned, in spawn order. */
export function spawnedBy(subagents: Record<string, Subagent>, toolCallId: string): Subagent[] {
  return Object.values(subagents)
    .filter((subagent) => subagent.parentToolCallId === toolCallId)
    .sort((left, right) => left.index - right.index);
}

/** Whether a `task` call has a pending or running subagent, i.e. its rows live in the docked list. */
export function isDockedCall(subagents: Record<string, Subagent>, toolCallId: string): boolean {
  return Object.values(subagents).some((subagent) => subagent.parentToolCallId === toolCallId && isSubagentActive(subagent));
}

/** What the docked list shows: every subagent of a `task` call that still has one running. Calls that finished stay out. */
export function dockedSubagents(subagents: Record<string, Subagent>): Subagent[] {
  const all = Object.values(subagents);
  const live = new Set(all.filter(isSubagentActive).map((subagent) => subagent.parentToolCallId));

  return all.filter((subagent) => live.has(subagent.parentToolCallId));
}
