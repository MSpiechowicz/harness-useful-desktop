import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import type { MessageRole } from '$lib/ipc/bindings/MessageRole';

import { hasPendingSend, pendingIndexFor } from './pending-send';
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

  // The words of a user message are known at its end: until then the message the app already shows stands for it.
  if (role === 'user' && hasPendingSend(state)) {
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

  // An echo delivered twice must not take a second waiting message's place: the one already shown only updates.
  const shown = state.items.find((item) => item.key === key);
  const echoed = message.role === 'user' && !(shown?.kind === 'message' && shown.send) ? pendingIndexFor(state, messageText(message)) : -1;
  if (echoed !== -1) {
    return { ...adoptEcho(state, echoed, key, message, blocks), toolCalls, lastUserText };
  }

  const exists = shown !== undefined;
  const base = exists
    ? updateItem(state, key, (item) => (item.kind === 'message' ? { ...finishedItem(item.key, item.order, message, blocks), send: item.send } : item))
    : appendItem(state, (_key, order) => finishedItem(key, order, message, blocks), key);

  return { ...base, toolCalls, lastUserText, items: withoutAdoptedCards(base.items, blocks) };
}

/** The echo of a message the app showed already takes that message's place and order, and keeps its row key as `send.id`. */
function adoptEcho(state: ChatState, at: number, key: string, message: ChatMessage, blocks: DisplayBlock[]): ChatState {
  const pending = state.items[at];
  if (pending.kind !== 'message' || !pending.send) {
    return state;
  }

  const echoed: MessageItem = { ...finishedItem(key, pending.order, message, blocks), send: { ...pending.send, status: 'sent' } };
  // The echo may already stand in the list as an empty message of its own: that one goes, this one takes the pending message's place.
  const items = state.items.flatMap((item, index) => (index === at ? [echoed] : item.key === key ? [] : [item]));

  return { ...state, items: withoutAdoptedCards(items, blocks) };
}

/** Role and creation time of a message: the harness gives a message the same time in its live events and in its history. */
function messageIdentity(role: MessageRole, timestamp: number | null): string | null {
  return timestamp === null ? null : `${role}@${timestamp}`;
}

/** The identities of the messages the transcript shows already: role and time, and the session entry id when known. */
function shownMessages(state: ChatState): Set<string> {
  const shown = new Set<string>();

  for (const item of state.items) {
    if (item.kind !== 'message' || item.meta === null) {
      continue;
    }

    const identity = messageIdentity(item.role, item.meta.timestamp);
    if (identity !== null) {
      shown.add(identity);
    }

    // a live message can carry the id its history entry has, but not the same time (Claude Code's own prompt)
    if (item.meta.entryId !== null) {
      shown.add(`id:${item.meta.entryId}`);
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
    if ((identity !== null && shown.has(identity)) || (message.id !== null && shown.has(`id:${message.id}`))) {
      continue;
    }

    const toolCalls = { ...working.toolCalls };
    const blocks = toDisplayBlocks(message.blocks, toolCalls);
    const key = `h${working.nextOrder}`;
    working = { ...working, toolCalls, items: [...working.items, finishedItem(key, -1, message, blocks)], nextOrder: working.nextOrder + 1 };
  }

  return { ...working, items: [...working.items, ...state.items], historyCursor: nextCursor, historyTotal: total };
}
