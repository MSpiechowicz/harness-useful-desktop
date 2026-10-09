import { chatStatus } from '../chat/chat-status';
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

/** What the rail says about a view without opening it: unsent work, pending questions, running agents. */
export function railBadge(view: ViewId, chat: ChatState | null, git: GitStatus | null): RailBadge | null {
  if (view === 'chat' && chat) {
    const status = chatStatus(chat);

    if (status.tone === 'warning') {
      return { text: '', label: status.label, tone: 'warning', busy: true };
    }

    if (chat.phase === 'streaming') {
      return { text: '', label: 'the agent is working', tone: 'accent', busy: true };
    }
  }

  if (view === 'changes' && git?.isRepo && git.entries.length > 0) {
    const count = git.entries.length;

    return { text: count > 99 ? '99+' : String(count), label: `${count} changed ${count === 1 ? 'file' : 'files'}`, tone: 'accent', busy: false };
  }

  return null;
}
