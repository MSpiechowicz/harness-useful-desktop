import type { AgentAction } from '$lib/ipc/bindings/AgentAction';

export type SendMode = 'send' | 'steer' | 'followUp';

interface KeyLike {
  key: string;
  shiftKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  isComposing?: boolean;
}

/**
 * What an Enter keypress in the composer means. Shift+Enter is a newline (null). While the
 * agent streams, Enter and Alt+Enter steer it and Ctrl/Cmd+Enter queues a follow-up.
 */
export function sendIntent(event: KeyLike, streaming: boolean): SendMode | null {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) {
    return null;
  }

  if (!streaming) {
    return 'send';
  }

  return event.ctrlKey || event.metaKey ? 'followUp' : 'steer';
}

/** The action that delivers `message` in this mode. */
export function messageAction(message: string, mode: SendMode): AgentAction {
  switch (mode) {
    case 'steer':
      return { type: 'steer', message };

    case 'followUp':
      return { type: 'followUp', message };

    case 'send':
      return { type: 'prompt', message, streamingBehavior: null };
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
