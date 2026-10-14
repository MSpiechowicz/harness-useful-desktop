import { t } from '$lib/i18n/i18n.svelte';
import type { ChatState } from '$lib/chat/types';
import type { AppStatus } from '$lib/stores/app.svelte';

import { chatStatus } from '../chat/chat-status';

export interface RailStatus {
  label: string;
  tone: 'success' | 'accent' | 'warning' | 'danger';
  /** The dot pings: the app is connected and nothing is wrong. */
  live: boolean;
}

/**
 * The rail footer's one status line: whether the desktop runtime answers, and otherwise what the active agent is
 * doing. Kept short to fit the rail.
 */
export function railStatus(appStatus: AppStatus, chat: ChatState | null): RailStatus {
  const agent = chat ? chatStatus(chat) : null;

  const label =
    appStatus === 'ready' ? (agent?.label ?? t('shell.status.ready')) : appStatus === 'loading' ? t('shell.status.connecting') : t('shell.status.unavailable');
  const tone: RailStatus['tone'] =
    appStatus !== 'ready' || agent?.tone === 'warning' ? 'warning' : agent?.tone === 'danger' ? 'danger' : agent?.tone === 'accent' ? 'accent' : 'success';

  return { label, tone, live: tone === 'success' || tone === 'accent' };
}
