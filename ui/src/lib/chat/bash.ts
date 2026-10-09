import type { BashOutput } from '$lib/ipc/bindings/BashOutput';

import { appendItem, updateItem } from './state';
import type { BashItem, ChatState } from './types';

/** Adds the (still running) block for a command the user confirmed; returns its key too. */
export function startBash(state: ChatState, command: string): { state: ChatState; key: string } {
  const next = appendItem(state, (key, order): BashItem => ({
    kind: 'bash',
    key,
    order,
    command,
    output: '',
    exitCode: null,
    cancelled: false,
    truncated: false,
    running: true,
  }));

  return { state: next, key: next.items[next.items.length - 1].key };
}

export function finishBash(state: ChatState, key: string, result: BashOutput | { error: string }): ChatState {
  return updateItem(state, key, (item) => {
    if (item.kind !== 'bash') {
      return item;
    }

    if ('error' in result) {
      return { ...item, running: false, output: result.error, exitCode: null };
    }

    return { ...item, running: false, output: result.output, exitCode: result.exitCode, cancelled: result.cancelled, truncated: result.truncated };
  });
}
