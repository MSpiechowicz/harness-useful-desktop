import { chatStatus } from '../chat/chat-status';
import { tn } from '$lib/i18n/i18n.svelte';
import type { ChatState } from '$lib/chat/types';
import type { GitStatus } from '$lib/ipc/bindings/GitStatus';
import type { ViewId } from '$lib/stores/nav.svelte';

export interface RailBadge {
  /** Short text shown in the badge; empty renders a plain dot. */
  text: string;
  /** Spoken in place of the text: "3 changed files". */
  label: string;
  tone: 'accent' | 'warning';
  /** The badge pulses: something is happening right now. */
  busy: boolean;
}

/**
 * What the rail says about a view without opening it: changed files, and a chat waiting on the user (an approval or a
 * question). A working agent needs no badge: the animated logo and the rail status already say so.
 */
export function railBadge(view: ViewId, chat: ChatState | null, git: GitStatus | null): RailBadge | null {
  if (view === 'chat' && chat) {
    const status = chatStatus(chat);

    if (status.tone === 'warning') {
      return { text: '', label: status.label, tone: 'warning', busy: true };
    }
  }

  if (view === 'changes' && git?.isRepo && git.entries.length > 0) {
    const count = git.entries.length;

    return { text: count > 99 ? '99+' : String(count), label: tn('shell.count.changedFile', count), tone: 'warning', busy: false };
  }

  return null;
}
