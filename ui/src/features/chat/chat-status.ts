import { t, tn } from '$lib/i18n/i18n.svelte';
import { pendingApproval } from '$lib/chat/approval';
import type { ChatState } from '$lib/chat/types';

export interface StatusSummary {
  label: string;
  tone: 'success' | 'accent' | 'warning' | 'danger' | 'neutral';
}

/** One line for the rail footer: what the active chat's agent is doing right now. */
export function chatStatus(chat: ChatState): StatusSummary {
  if (chat.phase === 'exited') {
    return { label: t('chat.status.stopped'), tone: 'danger' };
  }

  if (pendingApproval(chat)) {
    return { label: t('chat.status.approval'), tone: 'warning' };
  }

  if (chat.openRequests.length > 0) {
    return { label: t('chat.status.answer'), tone: 'warning' };
  }

  const queued = chat.queue.steering.length + chat.queue.followUp.length;
  const withQueue = (status: string): string => (queued > 0 ? t('chat.status.withQueue', { status, queued: tn('chat.status.queued', queued) }) : status);

  if (chat.phase === 'streaming') {
    return { label: withQueue(t('chat.status.streaming')), tone: 'accent' };
  }

  if (chat.phase === 'starting') {
    return { label: t('chat.status.starting'), tone: 'neutral' };
  }

  return { label: withQueue(chat.settledCount > 0 ? t('chat.status.settled') : t('chat.status.idle')), tone: 'success' };
}
