import type { UiRequest } from '$lib/ipc/bindings/UiRequest';

import type { ChatState, ToolCallState } from './types';

const APPROVE = /^(approve|allow|accept|yes)\b/i;
const DENY = /^(deny|reject|decline|block|no)\b/i;
const MAX_OPTIONS = 4;

/** An agent `select` request that asks whether a tool may run. */
export interface ApprovalPrompt {
  id: string;
  title: string;
  approve: string;
  deny: string;
  /** Extra answers (e.g. "Approve for this session"), shown as secondary buttons. */
  others: { option: string; detail: string | null }[];
  approveDetail: string | null;
  timeoutMs: number | null;
  /** The tool call the agent says this approval is about; null when it does not say. */
  toolCallId: string | null;
}

/** The approval a request asks for; null for any other dialog, and always for a question, whatever its options say. */
export function asApproval(request: UiRequest): ApprovalPrompt | null {
  const { kind } = request;
  if (request.purpose === 'question' || kind.type !== 'select' || kind.options.length > MAX_OPTIONS) {
    return null;
  }

  const approve = kind.options.find((option) => APPROVE.test(option));
  const deny = kind.options.find((option) => DENY.test(option));
  if (approve === undefined || deny === undefined) {
    return null;
  }

  const detailOf = (option: string): string | null => kind.details[kind.options.indexOf(option)] ?? null;

  return {
    id: request.id,
    title: kind.title,
    approve,
    deny,
    approveDetail: detailOf(approve),
    others: kind.options.filter((option) => option !== approve && option !== deny).map((option) => ({ option, detail: detailOf(option) })),
    timeoutMs: kind.timeoutMs,
    toolCallId: request.toolCallId ?? null,
  };
}

/** The first open request that is an approval; other requests open as dialogs. */
export function pendingApproval(state: ChatState): ApprovalPrompt | null {
  for (const request of state.openRequests) {
    const prompt = asApproval(request);
    if (prompt) {
      return prompt;
    }
  }

  return null;
}

/** The first open request that is not an approval. */
export function pendingDialog(state: ChatState): UiRequest | null {
  return state.openRequests.find((request) => asApproval(request) === null) ?? null;
}

/**
 * The tool call an approval is about: the one the agent names, else the latest one that has not finished.
 * A named call the transcript does not show (yet) gives null, so the card never shows another call's command.
 */
export function approvalSubject(state: ChatState, prompt: Pick<ApprovalPrompt, 'toolCallId'>): ToolCallState | null {
  if (prompt.toolCallId !== null) {
    return state.toolCalls[prompt.toolCallId] ?? null;
  }

  const open = Object.values(state.toolCalls).filter((call) => call.status === 'running' || call.status === 'pending');

  return open[open.length - 1] ?? null;
}
