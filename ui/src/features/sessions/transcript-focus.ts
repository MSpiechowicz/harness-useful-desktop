import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';

/**
 * The transcript row showing message `id`: history turns every message into one row except tool results, which fold
 * into the card of the row before them (see `prependHistory`). `null` when the message is not loaded.
 */
export function transcriptRow(messages: ChatMessage[], id: string): number | null {
  let row = -1;

  for (const message of messages) {
    if (message.role !== 'toolResult') {
      row += 1;
    }

    if (message.id === id) {
      return Math.max(row, 0);
    }
  }

  return null;
}

/** The id of the message shown in transcript row `row` (the inverse of `transcriptRow`), or `null`. */
export function rowMessage(messages: ChatMessage[], row: number): ChatMessage | null {
  let current = -1;

  for (const message of messages) {
    if (message.role === 'toolResult') {
      continue;
    }

    current += 1;
    if (current === row) {
      return message;
    }
  }

  return null;
}
