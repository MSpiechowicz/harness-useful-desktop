import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import type { SessionNode } from '$lib/ipc/bindings/SessionNode';

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

/**
 * The transcript row to show for tree node `id`: its own message, else that of its nearest ancestor with one. Model
 * changes, labels and nodes on other branches have no message of their own, so they land on the closest message
 * above them on the active path.
 */
export function focusRow(nodes: SessionNode[], messages: ChatMessage[], id: string): number | null {
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const seen = new Set<string>();
  let current: string | null = id;

  while (current !== null && !seen.has(current)) {
    seen.add(current);
    const row = transcriptRow(messages, current);
    if (row !== null) {
      return row;
    }

    current = byId.get(current)?.parentId ?? null;
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
