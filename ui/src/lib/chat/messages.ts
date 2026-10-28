import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import type { MessageRole } from '$lib/ipc/bindings/MessageRole';

import { appendItem, findLastIndex, messageKey, updateItem } from './state';
import { absorbToolResult, messageText, toDisplayBlocks } from './tools';
import { messageMeta, type ChatState, type DisplayBlock, type MessageItem, type TranscriptItem } from './types';

function emptyMessage(key: string, order: number, role: MessageRole): MessageItem {
  return { kind: 'message', key, order, role, blocks: [], streaming: true, slots: {}, meta: null };
}

export function messageStart(state: ChatState, messageId: string, role: MessageRole): ChatState {
  const key = messageKey(messageId);

  // Tool results live on their tool card; a repeated start changes nothing.
  if (role === 'toolResult' || state.items.some((item) => item.key === key)) {
    return state;
  }

  return appendItem(state, (_key, order) => emptyMessage(key, order, role), key);
}

/** Appends a streamed delta to the text or thinking block at `contentIndex`, creating the block on first sight. */
export function messageDelta(state: ChatState, messageId: string, contentIndex: number, delta: string, kind: 'text' | 'thinking'): ChatState {
  const key = messageKey(messageId);
  let next = state;

  if (findLastIndex(next.items, (item) => item.key === key) === -1) {
    next = messageStart(next, messageId, 'assistant');
  }

  return updateItem(next, key, (item) => {
    if (item.kind !== 'message' || !item.streaming) {
      return item;
    }

    const slot = item.slots[contentIndex];
    if (slot === undefined) {
      const block: DisplayBlock = kind === 'text' ? { type: 'text', text: delta } : { type: 'thinking', text: delta, redacted: false };

      return { ...item, blocks: [...item.blocks, block], slots: { ...item.slots, [contentIndex]: item.blocks.length } };
    }

    const blocks = item.blocks.slice();
    const block = blocks[slot];
    if (block.type === 'text' || block.type === 'thinking') {
      blocks[slot] = { ...block, text: block.text + delta };
    }

    return { ...item, blocks };
  });
}

/** Cards that were added before their message arrived are now shown by that message instead. */
function withoutAdoptedCards(items: TranscriptItem[], blocks: DisplayBlock[]): TranscriptItem[] {
  const adopted = new Set(blocks.flatMap((block) => (block.type === 'tool' ? [block.toolCallId] : [])));
  if (adopted.size === 0) {
    return items;
  }

  return items.filter((item) => !(item.kind === 'tool' && adopted.has(item.toolCallId)));
}

function finishedItem(key: string, order: number, message: ChatMessage, blocks: DisplayBlock[]): MessageItem {
  return { kind: 'message', key, order, role: message.role, blocks, streaming: false, slots: {}, meta: messageMeta(message) };
}

/** The authoritative message replaces whatever was streamed (or is added when nothing was). */
export function messageEnd(state: ChatState, messageId: string, message: ChatMessage): ChatState {
  if (message.role === 'toolResult') {
    return absorbToolResult(state, message);
  }

  const toolCalls = { ...state.toolCalls };
  const blocks = toDisplayBlocks(message.blocks, toolCalls);
  const key = messageKey(messageId);
  const lastUserText = message.role === 'user' ? messageText(message) : state.lastUserText;

  const exists = state.items.some((item) => item.key === key);
  const base = exists
    ? updateItem(state, key, (item) => (item.kind === 'message' ? finishedItem(item.key, item.order, message, blocks) : item))
    : appendItem(state, (_key, order) => finishedItem(key, order, message, blocks), key);

  return { ...base, toolCalls, lastUserText, items: withoutAdoptedCards(base.items, blocks) };
}

/** Role and creation time of a message: the harness gives a message the same time in its live events and in its history. */
function messageIdentity(role: MessageRole, timestamp: number | null): string | null {
  return timestamp === null ? null : `${role}@${timestamp}`;
}

/** The identities of the messages the transcript shows already. */
function shownMessages(state: ChatState): Set<string> {
  const shown = new Set<string>();

  for (const item of state.items) {
    const identity = item.kind === 'message' && item.meta !== null ? messageIdentity(item.role, item.meta.timestamp) : null;
    if (identity !== null) {
      shown.add(identity);
    }
  }

  return shown;
}

/**
 * Puts a page of older messages in front of the transcript. History items carry order -1 so the
 * list never animates them, and tool results fold into their cards like live ones do.
 */
export function prependHistory(state: ChatState, messages: ChatMessage[], nextCursor: string | null, total: number | null): ChatState {
  let working: ChatState = { ...state, items: [] };
  const shown = shownMessages(state);

  for (const message of messages) {
    if (message.role === 'toolResult') {
      working = absorbToolResult(working, message);
      continue;
    }

    // A history read while a turn runs lists the turn's finished messages, which the live stream already showed.
    const identity = messageIdentity(message.role, message.timestamp);
    if (identity !== null && shown.has(identity)) {
      continue;
    }

    const toolCalls = { ...working.toolCalls };
    const blocks = toDisplayBlocks(message.blocks, toolCalls);
    const key = `h${working.nextOrder}`;
    working = { ...working, toolCalls, items: [...working.items, finishedItem(key, -1, message, blocks)], nextOrder: working.nextOrder + 1 };
  }

  return { ...working, items: [...working.items, ...state.items], historyCursor: nextCursor, historyTotal: total };
}
