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
}

export function asApproval(request: UiRequest): ApprovalPrompt | null {
  const { kind } = request;
  if (kind.type !== 'select' || kind.options.length > MAX_OPTIONS) {
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

/** The tool call an approval is most likely about: the latest one that has not finished. */
export function approvalSubject(state: ChatState): ToolCallState | null {
  const open = Object.values(state.toolCalls).filter((call) => call.status === 'running' || call.status === 'pending');

  return open[open.length - 1] ?? null;
}
