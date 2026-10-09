import { pendingApproval } from '$lib/chat/approval';
import type { ChatState } from '$lib/chat/types';

export interface StatusSummary {
  label: string;
  tone: 'success' | 'warning' | 'danger' | 'neutral';
}

/** One line for the status bar: what the active chat's agent is doing right now. */
export function chatStatus(chat: ChatState): StatusSummary {
  if (chat.phase === 'exited') {
    return { label: 'Agent stopped', tone: 'danger' };
  }

  if (pendingApproval(chat)) {
    return { label: 'Waiting for your approval', tone: 'warning' };
  }

  if (chat.openRequests.length > 0) {
    return { label: 'Waiting for your answer', tone: 'warning' };
  }

  const queued = chat.queue.steering.length + chat.queue.followUp.length;
  const suffix = queued > 0 ? ` · ${queued} queued` : '';

  if (chat.phase === 'streaming') {
    return { label: `Streaming${suffix}`, tone: 'success' };
  }

  if (chat.phase === 'starting') {
    return { label: 'Agent starting…', tone: 'neutral' };
  }

  return { label: `${chat.settledCount > 0 ? 'Settled' : 'Idle'}${suffix}`, tone: 'success' };
}
