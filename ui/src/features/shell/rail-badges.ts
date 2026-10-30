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

/** What waits for the user in the Routines view: routines to approve, and runs that ask something. */
export interface RoutineAttention {
  pending: number;
  needsYou: number;
}

/**
 * What the rail says about a view without opening it: changed files, hooks and routines waiting for approval, a routine run that asks something, and a chat waiting on the user (an approval or a
 * question). A working agent needs no badge: the animated logo and the rail status already say so.
 */
export function railBadge(view: ViewId, chat: ChatState | null, git: GitStatus | null, pendingHooks = 0, routines: RoutineAttention = { pending: 0, needsYou: 0 }, hookUpdates = 0): RailBadge | null {
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

  if (view === 'hooks' && pendingHooks + hookUpdates > 0) {
    return hooksBadge(pendingHooks, hookUpdates);
  }

  if (view === 'routines' && routines.pending + routines.needsYou > 0) {
    return routinesBadge(routines);
  }

  return null;
}

/** The count of what waits in Hooks: hooks to approve (urgent, warning) and Harness family hooks with a newer catalog text (accent, when nothing waits for approval). */
function hooksBadge(pending: number, updates: number): RailBadge {
  const total = pending + updates;
  const parts = [pending > 0 ? tn('shell.count.hooksToApprove', pending) : '', updates > 0 ? tn('shell.count.hookUpdates', updates) : ''];

  return { text: total > 99 ? '99+' : String(total), label: parts.filter(Boolean).join(', '), tone: pending > 0 ? 'warning' : 'accent', busy: false };
}

/** The count of what waits in Routines. A run that asks something is urgent, so it makes the badge pulse. */
function routinesBadge({ pending, needsYou }: RoutineAttention): RailBadge {
  const total = pending + needsYou;
  const parts = [pending > 0 ? tn('shell.count.routinesToApprove', pending) : '', needsYou > 0 ? tn('shell.count.routinesNeedYou', needsYou) : ''];

  return { text: total > 99 ? '99+' : String(total), label: parts.filter(Boolean).join(', '), tone: 'warning', busy: needsYou > 0 };
}
