import type { SubagentStep } from '$lib/ipc/bindings/SubagentStep';
import type { SubagentRoute } from '$lib/ipc/bindings/SubagentRoute';

import { findLastIndex, own } from './state';
import type { ChatState, SentStep, SubagentStepEntry, TranscriptItem } from './types';

/** The steps kept per subagent: the oldest go first. */
export const MAX_STEPS = 200;
/** The subagents whose steps and clock are kept: past it the oldest finished one is forgotten. */
export const MAX_TRACKED = 64;

function lastWhere<T>(list: readonly T[], matches: (item: T) => boolean): T | undefined {
  return list[findLastIndex(list, matches)];
}

const isRunningTool = (step: SubagentStep): boolean => step.kind === 'tool' && step.status === 'running';

/** The step of a tool call or a prose block, not a message the user sent. */
export function isAgentStep(entry: SubagentStepEntry): entry is SubagentStep {
  return entry.kind !== 'you';
}

/** Forgets the oldest finished subagents beyond `MAX_TRACKED`, with their steps. A running one is never forgotten. */
export function evictFinished(state: ChatState): ChatState {
  const ids = new Set([...Object.keys(state.subagentClock), ...Object.keys(state.subagentSteps)]);
  if (ids.size <= MAX_TRACKED) {
    return state;
  }

  const finished = [...ids]
    .filter((id) => own(state.subagentClock, id)?.endedAt != null)
    .sort((left, right) => (own(state.subagentClock, left)?.endedAt ?? 0) - (own(state.subagentClock, right)?.endedAt ?? 0));
  const drop = new Set(finished.slice(0, ids.size - MAX_TRACKED));
  if (drop.size === 0) {
    return state;
  }

  const keep = <T>(record: Record<string, T>): Record<string, T> => Object.fromEntries(Object.entries(record).filter(([id]) => !drop.has(id)));

  return { ...state, subagentSteps: keep(state.subagentSteps), subagentClock: keep(state.subagentClock) };
}

/** Only the newest output is shown, so a step that brings output takes it from the older ones (bounds memory). */
function keepLatestOutput(entries: SubagentStepEntry[], step: SubagentStep): SubagentStepEntry[] {
  if (step.output === null) {
    return entries;
  }

  return entries.map((entry) => (isAgentStep(entry) && entry.id !== step.id && entry.output !== null ? { ...entry, output: null } : entry));
}

function withEntries(state: ChatState, subagentId: string, entries: SubagentStepEntry[]): ChatState {
  const capped = entries.length > MAX_STEPS ? entries.slice(entries.length - MAX_STEPS) : entries;

  return evictFinished({ ...state, subagentSteps: { ...state.subagentSteps, [subagentId]: capped } });
}

/** A step from the harness. A later one with the same id replaces the earlier (a tool call is `running`, then `done`). */
export function applySubagentStep(state: ChatState, subagentId: string, step: SubagentStep): ChatState {
  const entries = keepLatestOutput(own(state.subagentSteps, subagentId) ?? [], step);
  let index = -1;

  for (let at = entries.length - 1; at >= 0; at -= 1) {
    const entry = entries[at];
    if (isAgentStep(entry) && entry.id === step.id) {
      index = at;
      break;
    }
  }

  if (index === -1) {
    return withEntries(state, subagentId, [...entries, step]);
  }

  const next = entries.slice();
  next[index] = step;

  return withEntries(state, subagentId, next);
}

/** Lists a message the user sent to the subagent among its steps. */
export function addSentMessage(state: ChatState, subagentId: string, text: string, route: SubagentRoute, now: number): ChatState {
  const entries = own(state.subagentSteps, subagentId) ?? [];
  const sent: SentStep = { kind: 'you', id: `you:${now}:${entries.length}`, text, at: now, route };

  return withEntries(state, subagentId, [...entries, sent]);
}

/** The messages the user sent to the subagent, oldest first. */
export function sentMessages(entries: readonly SubagentStepEntry[]): SentStep[] {
  return entries.filter((entry): entry is SentStep => entry.kind === 'you');
}

/** What a subagent is doing right now: the tool it runs (`name`) with its arguments, or its own prose. */
export interface CurrentAction {
  name: string | null;
  detail: string;
}

function actionOf(step: SubagentStep): CurrentAction {
  return step.kind === 'tool' ? { name: step.name, detail: step.detail } : { name: null, detail: step.detail };
}

/** The tool still running, else the latest step; without steps, what the snapshot says (OMP reports it there). */
export function currentAction(entries: readonly SubagentStepEntry[], snapshot: { currentTool: string | null; currentArgs: string | null; intent: string | null }): CurrentAction | null {
  const steps = entries.filter(isAgentStep);
  const latest = lastWhere(steps, isRunningTool) ?? steps.at(-1);

  if (latest) {
    return actionOf(latest);
  }
  if (snapshot.currentTool) {
    return { name: snapshot.currentTool, detail: snapshot.currentArgs ?? '' };
  }

  return snapshot.intent ? { name: null, detail: snapshot.intent } : null;
}

/** The step that is running its tool now (the one row of the list that says so). */
export function liveStepId(entries: readonly SubagentStepEntry[]): string | null {
  return lastWhere(entries.filter(isAgentStep), isRunningTool)?.id ?? null;
}

/** The tool calls the steps list: how many of them there are. */
export function toolStepCount(entries: readonly SubagentStepEntry[]): number {
  return entries.filter((entry) => isAgentStep(entry) && entry.kind === 'tool').length;
}

/** The newest tool step that kept the end of its output, for the Output tab. */
export function latestOutput(entries: readonly SubagentStepEntry[]): SubagentStep | null {
  return lastWhere(entries.filter(isAgentStep), (step) => step.kind === 'tool' && Boolean(step.output)) ?? null;
}

/** Where a tool call sits in the transcript (a message block or a card of its own), or -1 when it is not there. */
export function indexOfToolCall(items: readonly TranscriptItem[], toolCallId: string): number {
  return items.findIndex((item) => (item.kind === 'tool' && item.toolCallId === toolCallId) || (item.kind === 'message' && item.blocks.some((block) => block.type === 'tool' && block.toolCallId === toolCallId)));
}

/** The lines of a result the card shows before "Show all". */
export const OUTPUT_TAIL_LINES = 20;

export interface OutputView {
  lines: string[];
  /** The line number of the first line shown, counted in the whole result. */
  first: number;
  /** The lines the result had before the cut. */
  total: number;
  /** More lines are held than the tail shows. */
  canExpand: boolean;
}

/** The end of a tool result as the Output tab shows it: the last lines, or all the harness kept with `showAll`. */
export function outputView(step: SubagentStep, showAll: boolean): OutputView {
  const held = (step.output ?? '').split('\n');
  if (held.at(-1) === '') {
    held.pop();
  }

  const total = Math.max(step.outputLines ?? held.length, held.length);
  const lines = showAll ? held : held.slice(-OUTPUT_TAIL_LINES);

  return { lines, first: total - lines.length + 1, total, canExpand: held.length > OUTPUT_TAIL_LINES };
}
