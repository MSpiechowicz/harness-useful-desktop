import type { AgentAction } from '$lib/ipc/bindings/AgentAction';
import type { ImageAttachment } from '$lib/ipc/bindings/ImageAttachment';

export type SendMode = 'send' | 'steer' | 'followUp';

interface KeyLike {
  key: string;
  shiftKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  isComposing?: boolean;
}

/** What a message does while the agent works: reaches it at its next step, or waits until it has finished. */
export type BusyMode = 'steer' | 'followUp';

/** A second Escape within this long after the first stops the agent. */
export const STOP_WINDOW_MS = 1500;

/**
 * What an Enter keypress in the composer means. Shift+Enter is a newline (null). While the agent streams, Enter does
 * what the composer's switch says (steer by default) and Ctrl/Cmd+Enter does the other one.
 */
export function sendIntent(event: KeyLike, streaming: boolean, preferred: BusyMode = 'steer'): SendMode | null {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) {
    return null;
  }

  if (!streaming) {
    return 'send';
  }

  const other: BusyMode = preferred === 'steer' ? 'followUp' : 'steer';

  return event.ctrlKey || event.metaKey ? other : preferred;
}

/** One Escape arms the stop; a second inside the window (`armedAt` is when the first came) carries it out. */
export function escapeStep(armedAt: number | null, now: number): 'arm' | 'stop' {
  return armedAt !== null && now - armedAt <= STOP_WINDOW_MS ? 'stop' : 'arm';
}

/** The composer shortcut that takes the latest waiting message back into the draft. */
export const TAKE_BACK = 'alt+arrowup';

/** A message waiting for the agent, as the queue above the composer lists it. */
export interface QueuedRow {
  message: string;
  queue: BusyMode;
  /** False for steering the agent already took into its running response. */
  withdrawable: boolean;
}

/** The queue in the order the agent takes it: steering first, then follow-ups. */
export function queuedRows(queue: { steering: string[]; followUp: string[]; liveSteered: number }): QueuedRow[] {
  return [
    ...queue.steering.map((message, index) => ({ message, queue: 'steer' as const, withdrawable: index >= queue.liveSteered })),
    ...queue.followUp.map((message) => ({ message, queue: 'followUp' as const, withdrawable: true })),
  ];
}

/** What the take-back shortcut withdraws, like OMP's dequeue key: the newest steering, else the newest follow-up. */
export function latestWithdrawable(rows: QueuedRow[]): QueuedRow | null {
  const open = rows.filter((row) => row.withdrawable);
  const steering = open.filter((row) => row.queue === 'steer');

  return steering.at(-1) ?? open.at(-1) ?? null;
}

/** The action that delivers `message` and its images in this mode. */
export function messageAction(message: string, mode: SendMode, images: ImageAttachment[] = []): AgentAction {
  switch (mode) {
    case 'steer':
      return { type: 'steer', message, images };

    case 'followUp':
      return { type: 'followUp', message, images };

    case 'send':
      return { type: 'prompt', message, images, streamingBehavior: null };
  }
}

/** The command of a `!command` draft, or null when the draft is not (or only just starts) a bash command. */
export function bashCommand(draft: string): string | null {
  if (!draft.startsWith('!')) {
    return null;
  }

  const command = draft.slice(1).trim();

  return command === '' ? null : command;
}

export function isBashDraft(draft: string): boolean {
  return draft.startsWith('!');
}
