import type { UiRequest } from '$lib/ipc/bindings/UiRequest';

import type { ChatState, ToolCallState } from './types';

const APPROVE = /^(approve|allow|accept|yes)\b/i;
const APPROVE_ONCE = /^approve once$/i;
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
  /** True when the agent marks this as an approval, so one without a tool call is about nothing in this transcript. */
  marked: boolean;
}

/** The approval a request asks for; null for any other dialog, and always for a question, whatever its options say. */
export function asApproval(request: UiRequest): ApprovalPrompt | null {
  const { kind } = request;
  if (request.purpose === 'question' || kind.type !== 'select') {
    return null;
  }

  // Beside a grant that remembers more, the primary answer is always this call only.
  const once = kind.options.find((option) => APPROVE_ONCE.test(option));
  if (once === undefined && kind.options.length > MAX_OPTIONS) {
    return null;
  }

  const approve = once ?? kind.options.find((option) => APPROVE.test(option));
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
    marked: request.purpose === 'approval',
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

/** The tool an OMP approval title names: `Allow tool: bash`. */
const TITLE_TOOL = /^Allow tool: (\S+)/;

/**
 * The tool calls an approval may be about: the one the agent names, else the open calls of the tool its title names,
 * else the only open call. An approval the agent marks but ties to no call (a subagent's) is about none of them, and a
 * named call the transcript does not show (yet) gives none, so the card never shows another call's command as its own.
 */
export function approvalCalls(state: ChatState, prompt: Pick<ApprovalPrompt, 'toolCallId' | 'marked' | 'title'>): ToolCallState[] {
  if (prompt.toolCallId !== null) {
    const named = state.toolCalls[prompt.toolCallId];

    return named ? [named] : [];
  }

  if (prompt.marked) {
    return [];
  }

  const open = Object.values(state.toolCalls).filter((call) => call.status === 'running' || call.status === 'pending');
  const tool = TITLE_TOOL.exec(prompt.title)?.[1];

  if (tool !== undefined) {
    return open.filter((call) => call.name === tool);
  }

  return open.length === 1 ? open : [];
}
