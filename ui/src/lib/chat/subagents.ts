import { t } from '$lib/i18n/i18n.svelte';
import type { AgentAction } from '$lib/ipc/bindings/AgentAction';
import type { Subagent } from '$lib/ipc/bindings/Subagent';
import type { SubagentStatus } from '$lib/ipc/bindings/SubagentStatus';

import { own } from './state';
import { evictFinished } from './subagent-steps';
import type { ChatState, SubagentClock } from './types';

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

export function isSubagentActive(subagent: Subagent): boolean {
  return subagent.status === 'pending' || subagent.status === 'running';
}

/**
 * When the subagent started and ended on this computer's clock. A harness that reports how long it has run puts the start
 * that far back, so the timer agrees with it; the end is the moment the snapshot stopped being active.
 */
function nextClock(previous: SubagentClock | undefined, incoming: Subagent, merged: Subagent, now: number): SubagentClock {
  const active = isSubagentActive(merged);
  // Only a figure this snapshot carries counts: the merged one may be an older snapshot's.
  const reported = active && incoming.durationMs !== null ? now - incoming.durationMs : null;

  return {
    startedAt: reported ?? previous?.startedAt ?? now - (incoming.durationMs ?? 0),
    endedAt: active ? null : (previous?.endedAt ?? now),
  };
}

export function applySubagent(state: ChatState, subagent: Subagent, now = Date.now()): ChatState {
  const merged = mergeSubagent(own(state.subagents, subagent.id), subagent);

  return evictFinished({
    ...state,
    subagents: { ...state.subagents, [subagent.id]: merged },
    subagentClock: { ...state.subagentClock, [subagent.id]: nextClock(own(state.subagentClock, subagent.id), subagent, merged, now) },
  });
}

/** How long the subagent has run, or ran: the live timer while it is active, the harness's own figure (else the clock's) after. */
export function elapsedMs(subagent: Subagent, clock: SubagentClock | undefined, now: number): number | null {
  if (subagent.status === 'pending') {
    return null;
  }
  if (isSubagentActive(subagent)) {
    return clock ? Math.max(0, now - clock.startedAt) : subagent.durationMs;
  }

  return subagent.durationMs ?? (clock?.endedAt != null ? Math.max(0, clock.endedAt - clock.startedAt) : null);
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

export function statusLabel(status: SubagentStatus): string {
  switch (status) {
    case 'pending':
      return t('chat.subagents.status.pending');

    case 'running':
      return t('chat.subagents.status.running');

    case 'completed':
      return t('chat.subagents.status.completed');

    case 'failed':
      return t('chat.subagents.status.failed');

    case 'aborted':
      return t('chat.subagents.status.aborted');
  }
}

/** What a subagent is doing now, in one line. */
export function activity(subagent: Subagent): string {
  if (subagent.status === 'pending') {
    return t('chat.subagents.waitingToStart');
  }
  if (subagent.status !== 'running') {
    // A finished one shows what it found last; the assignment is one click away.
    return subagent.recentOutput?.at(-1) ?? subagent.description ?? subagent.task ?? statusLabel(subagent.status);
  }

  return subagent.intent ?? (subagent.currentTool ? `${subagent.currentTool} ${subagent.currentArgs ?? ''}`.trim() : (subagent.description ?? t('chat.subagents.working')));
}

/** The last line of the final report: what a finished subagent says last. */
export function reportTail(report: string | null): string | null {
  const lines = (report ?? '').split('\n').map((line) => line.trim()).filter((line) => line !== '');

  return lines.at(-1) ?? null;
}

/** `5s`, `1m 05s`. */
export function elapsed(ms: number | null): string | null {
  if (ms === null) {
    return null;
  }

  const seconds = Math.round(ms / 1000);

  return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, '0')}s`;
}

/** The panel title: what the subagent was spawned for, else its id. */
export function subagentTitle(subagent: Subagent): string {
  return subagent.label ?? subagent.id;
}

/** Which subagent Ctrl+] opens: the first that is running, else the newest. */
export function defaultPanelTarget(subagents: Record<string, Subagent>): string | null {
  const all = Object.values(subagents);

  return (all.find((subagent) => subagent.status === 'running') ?? all.at(-1))?.id ?? null;
}

/** Where a message typed in the panel goes: straight to the subagent, through the orchestrator, or nowhere. */
export type MessageDelivery = 'direct' | 'orchestrator' | 'finished';

export function messageDelivery(subagent: Subagent): MessageDelivery | null {
  if (subagent.messageRoute === null) {
    return null;
  }
  if (!isSubagentActive(subagent)) {
    return 'finished';
  }

  return subagent.messageRoute === 'direct' ? 'direct' : 'orchestrator';
}

/** The note the orchestrator receives with the user's message. It is for the model, so it stays in English in every language. */
export function orchestratorNote(subagent: Subagent, text: string): string {
  return `Message for subagent ${quoted(subagentTitle(subagent))} (${quoted(subagent.id)}): ${text}`;
}

/** A name the agent wrote, on one line and in quotes, so it cannot pass for more of the user's message. */
function quoted(name: string): string {
  return JSON.stringify(name.replace(/\s+/g, ' ').trim());
}

/** The action that carries the user's message to the subagent, and how it travels; null when the subagent takes none. */
export function messageRequest(subagent: Subagent, text: string): { action: AgentAction; route: 'direct' | 'orchestrator' } | null {
  const delivery = messageDelivery(subagent);

  if (delivery === null) {
    return null;
  }
  if (delivery === 'direct') {
    return { action: { type: 'messageSubagent', id: subagent.id, message: text }, route: 'direct' };
  }

  return { action: { type: 'prompt', message: orchestratorNote(subagent, text), images: [], streamingBehavior: 'steer' }, route: 'orchestrator' };
}
