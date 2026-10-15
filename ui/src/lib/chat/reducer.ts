import { t } from '$lib/i18n/i18n.svelte';
import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';
import type { NoticeLevel } from '$lib/ipc/bindings/NoticeLevel';
import type { UiRequest } from '$lib/ipc/bindings/UiRequest';

import { asApproval } from './approval';
import { messageDelta, messageEnd, messageStart } from './messages';
import { appendItem, findLastIndex, updateItem } from './state';
import { applySubagent } from './subagents';
import { interruptRunningTools, toolEnd, toolStart, toolUpdate } from './tools';
import type { BannerItem, BannerPhase, ChatState, TranscriptItem } from './types';

const STDERR_LINES = 20;

export function addNotice(state: ChatState, level: NoticeLevel, message: string): ChatState {
  return appendItem(state, (key, order) => ({ kind: 'notice', key, order, level, message }));
}

/** Removes an answered or cancelled agent UI request. */
export function dropRequest(state: ChatState, requestId: string): ChatState {
  if (!state.openRequests.some((request) => request.id === requestId)) {
    return state;
  }

  return { ...state, openRequests: state.openRequests.filter((request) => request.id !== requestId) };
}

function openRequest(state: ChatState, request: UiRequest): ChatState {
  if (state.openRequests.some((open) => open.id === request.id)) {
    return state;
  }

  return { ...state, openRequests: [...state.openRequests, request] };
}

/** Streaming messages stop streaming, tool calls without an end are marked interrupted, and approvals (which only make sense mid-turn) go away. */
function finishTurn(state: ChatState): ChatState {
  const streaming = state.items.some((item) => item.kind === 'message' && item.streaming);
  const items = streaming ? state.items.map((item): TranscriptItem => (item.kind === 'message' && item.streaming ? { ...item, streaming: false, slots: {} } : item)) : state.items;

  const openRequests = state.openRequests.filter((request) => asApproval(request) === null);

  return interruptRunningTools({ ...state, items, openRequests });
}

/** Starts a banner, or finishes the running one of the same kind. */
function banner(state: ChatState, kind: BannerItem['kind'], phase: 'start' | 'end', text: string, failed: boolean): ChatState {
  const finished: BannerPhase = failed ? 'failed' : 'done';

  if (phase === 'start') {
    return appendItem(state, (key, order) => ({ kind, key, order, phase: 'running', text }));
  }

  const index = findLastIndex(state.items, (item) => item.kind === kind && item.phase === 'running');
  if (index === -1) {
    return appendItem(state, (key, order) => ({ kind, key, order, phase: finished, text }));
  }

  return updateItem(state, state.items[index].key, (item) => ({ ...(item as BannerItem), phase: finished, text }));
}

function retryText(event: Extract<AgentEvent, { type: 'retry' }>): string {
  const label = event.phase === 'start' ? t('chat.retry.start') : t('chat.retry.end');
  const text = event.attempt !== null && event.maxAttempts !== null ? t('chat.retry.attempt', { label, attempt: event.attempt, max: event.maxAttempts }) : label;

  return event.message ? t('chat.retry.detail', { text, message: event.message }) : text;
}

function compactionText(event: Extract<AgentEvent, { type: 'compaction' }>): string {
  if (event.phase === 'start') {
    return event.reason ? t('chat.compaction.runningReason', { reason: event.reason }) : t('chat.compaction.running');
  }

  return event.error ? t('chat.compaction.failed', { error: event.error }) : t('chat.compaction.done');
}

function promptResult(state: ChatState, event: Extract<AgentEvent, { type: 'promptResult' }>): ChatState {
  if (event.status === 'error') {
    return { ...state, promptError: { message: event.error ?? t('chat.promptError.fallback'), retryable: event.retryable } };
  }

  return { ...state, promptError: null };
}

function stderrLine(state: ChatState, line: string): ChatState {
  return { ...state, stderrTail: [...state.stderrTail, line].slice(-STDERR_LINES) };
}

/** Applies one event; never throws, unknown events change nothing. */
export function applyEvent(state: ChatState, event: AgentEvent): ChatState {
  switch (event.type) {
    case 'ready':
      return { ...state, protocolVersion: event.protocolVersion, readyCount: state.readyCount + 1, phase: state.phase === 'starting' ? 'idle' : state.phase };

    case 'state':
      return { ...state, session: event.state, phase: event.state.isStreaming && state.phase !== 'exited' ? 'streaming' : state.phase };

    case 'agentStart':
      return { ...state, phase: state.phase === 'exited' ? state.phase : 'streaming', promptError: null };

    case 'agentEnd':
      return { ...finishTurn(state), phase: state.phase === 'exited' ? state.phase : 'idle' };

    case 'settled':
      return { ...finishTurn(state), phase: state.phase === 'exited' ? state.phase : 'idle', settledCount: state.settledCount + 1 };

    case 'turnStart':
    case 'turnEnd':
    case 'unknown':
      return state;

    case 'messageStart':
      return messageStart(state, event.messageId, event.role);

    case 'textDelta':
      return messageDelta(state, event.messageId, event.contentIndex, event.delta, 'text');

    case 'thinkingDelta':
      return messageDelta(state, event.messageId, event.contentIndex, event.delta, 'thinking');

    case 'messageEnd':
      return messageEnd(state, event.messageId, event.message);

    case 'toolStart':
      return toolStart(state, event.toolCallId, event.name, event.args, event.intent);

    case 'toolUpdate':
      return toolUpdate(state, event.toolCallId, event.partial);

    case 'toolEnd':
      return toolEnd(state, event.toolCallId, event.name, event.isError, event.result, event.touchedPaths);

    case 'subagent':
      return applySubagent(state, event.subagent);

    case 'commandsUpdated':
      return { ...state, commands: event.commands };

    case 'commandOutput':
      return appendItem(state, (key, order) => ({ kind: 'output', key, order, text: event.text }));

    case 'promptResult':
      return promptResult(state, event);

    case 'queueUpdate':
      return { ...state, queue: { steering: event.steering, followUp: event.followUp, liveSteered: event.liveSteered } };

    case 'compaction':
      return banner(state, 'compaction', event.phase === 'start' ? 'start' : 'end', compactionText(event), event.error !== null);

    case 'retry':
      return banner(state, 'retry', event.phase === 'start' ? 'start' : 'end', retryText(event), /fail|error/i.test(event.message ?? ''));

    case 'modelChanged':
      return state.session ? { ...state, session: { ...state.session, model: event.model } } : state;

    case 'thinkingChanged':
      return state.session ? { ...state, session: { ...state.session, thinkingLevel: event.level } } : state;

    case 'uiRequest':
      return openRequest(state, event.request);

    case 'uiCancel':
      return dropRequest(state, event.requestId);

    case 'notice':
      return addNotice(state, event.level, event.message);

    case 'status': {
      const { [event.key]: _removed, ...rest } = state.statuses;

      return { ...state, statuses: event.text === null ? rest : { ...rest, [event.key]: event.text } };
    }

    case 'widget': {
      const { [event.key]: _removed, ...rest } = state.widgets;

      return { ...state, widgets: event.lines.length === 0 ? rest : { ...rest, [event.key]: { lines: event.lines, placement: event.placement } } };
    }

    case 'title':
      return { ...state, title: event.title };

    case 'editorText':
      return { ...state, editorText: { text: event.text, nonce: (state.editorText?.nonce ?? 0) + 1 } };

    case 'openUrl':
      return appendItem(state, (key, order) => ({ kind: 'url', key, order, url: event.url, instructions: event.instructions }));

    case 'frameError': {
      const message = event.originalType ? t('chat.frameError.typed', { type: event.originalType, error: event.error }) : t('chat.frameError', { error: event.error });

      return addNotice(state, 'warning', message);
    }

    case 'stderr':
      return stderrLine(state, event.line);

    case 'exited':
      return { ...finishTurn(state), phase: 'exited', openRequests: [], crash: { code: event.code, signal: event.signal, stderr: state.stderrTail } };
  }
}

/**
 * The single entry point for agent events: ignores envelopes of other sessions and duplicates,
 * counts gaps in `seq`, and applies the rest in order.
 */
export function reduceEnvelope(state: ChatState, envelope: AgentEnvelope): ChatState {
  if (envelope.sessionKey !== state.key || envelope.seq <= state.lastSeq) {
    return state;
  }

  const gapped = envelope.seq > state.lastSeq + 1;

  return applyEvent({ ...state, lastSeq: envelope.seq, gaps: state.gaps + (gapped ? 1 : 0) }, envelope.event);
}

/** A new session starts empty: the transcript, calls, requests and queue of the old one go away. */
export function clearTranscript(state: ChatState): ChatState {
  return {
    ...state,
    items: [],
    toolCalls: {},
    openRequests: [],
    queue: { steering: [], followUp: [], liveSteered: 0 },
    promptError: null,
    lastUserText: null,
    historyCursor: null,
    historyTotal: null,
    title: null,
  };
}
