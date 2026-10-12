import { t } from '$lib/i18n/i18n.svelte';
import type { ChatBlock } from '$lib/ipc/bindings/ChatBlock';
import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';

import { appendItem, findLastIndex, updateItem } from './state';
import type { ChatState, DisplayBlock, MessageItem, ToolCallState, ToolItem } from './types';

type JsonObject = { [key in string]: JsonValue };

export function isJsonObject(value: JsonValue | null | undefined): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** The readable text of a tool result or partial: its text parts, a plain string, or pretty JSON. */
export function resultText(value: JsonValue | null): string | null {
  if (value === null) {
    return null;
  }
  if (typeof value === 'string') {
    return value;
  }

  if (isJsonObject(value)) {
    const content = value.content;
    const parts = Array.isArray(content) ? content.flatMap((part) => (isJsonObject(part) && part.type === 'text' && typeof part.text === 'string' ? [part.text] : [])) : [];
    if (parts.length > 0) {
      return parts.join('\n');
    }
    if (typeof value.text === 'string') {
      return value.text;
    }
  }

  return JSON.stringify(value, null, 2);
}

export function blankToolCall(id: string, name: string): ToolCallState {
  return { id, name, args: null, intent: null, status: 'pending', partial: null, result: null, resultText: null, touchedPaths: [] };
}

/** Message blocks as the transcript shows them; tool calls are registered (or refreshed) in `calls`. */
export function toDisplayBlocks(blocks: ChatBlock[], calls: Record<string, ToolCallState>): DisplayBlock[] {
  return blocks.map((block): DisplayBlock => {
    switch (block.type) {
      case 'text':
        return { type: 'text', text: block.text };

      case 'thinking':
        return { type: 'thinking', text: block.text, redacted: block.redacted };

      case 'toolCall': {
        const known = calls[block.id] ?? blankToolCall(block.id, block.name);
        calls[block.id] = { ...known, name: block.name, args: block.args, intent: block.intent ?? known.intent };

        return { type: 'tool', toolCallId: block.id };
      }

      case 'image':
        return { type: 'image', mimeType: block.mimeType };

      case 'other':
        return { type: 'other', kind: block.kind };
    }
  });
}

/** Text of a message's text blocks. */
export function messageText(message: ChatMessage): string {
  return message.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n');
}

/** A tool result message belongs to the card of its call; it never gets a transcript row of its own. */
export function absorbToolResult(state: ChatState, message: ChatMessage): ChatState {
  const id = message.toolCallId;
  if (id === null) {
    return state;
  }

  const known = state.toolCalls[id] ?? blankToolCall(id, message.toolName ?? 'tool');
  const failed = message.isError === true || known.status === 'error';
  const next: ToolCallState = { ...known, resultText: known.resultText ?? messageText(message), status: failed ? 'error' : 'ok' };

  return { ...state, toolCalls: { ...state.toolCalls, [id]: next } };
}

function isReferenced(state: ChatState, id: string): boolean {
  return state.items.some((item) => (item.kind === 'tool' && item.toolCallId === id) || (item.kind === 'message' && item.blocks.some((block) => block.type === 'tool' && block.toolCallId === id)));
}

/** Puts a card for `id` into the message being streamed, or after the transcript when no message is. */
function attach(state: ChatState, id: string): ChatState {
  if (isReferenced(state, id)) {
    return state;
  }

  const index = findLastIndex(state.items, (item) => item.kind === 'message' && item.role === 'assistant' && item.streaming);
  if (index === -1) {
    return appendItem(state, (key, order): ToolItem => ({ kind: 'tool', key, order, toolCallId: id }));
  }

  const target = state.items[index] as MessageItem;

  return updateItem(state, target.key, () => ({ ...target, blocks: [...target.blocks, { type: 'tool', toolCallId: id }] }));
}

export function toolStart(state: ChatState, id: string, name: string, args: JsonValue, intent: string | null): ChatState {
  const known = state.toolCalls[id] ?? blankToolCall(id, name);
  const call: ToolCallState = { ...known, name, args, intent: intent ?? known.intent, status: 'running' };

  return attach({ ...state, toolCalls: { ...state.toolCalls, [id]: call } }, id);
}

export function toolUpdate(state: ChatState, id: string, partial: JsonValue): ChatState {
  const known = state.toolCalls[id];
  if (!known) {
    return state;
  }

  return { ...state, toolCalls: { ...state.toolCalls, [id]: { ...known, partial: resultText(partial) } } };
}

export function toolEnd(state: ChatState, id: string, name: string, isError: boolean, result: JsonValue, touchedPaths: string[]): ChatState {
  const known = state.toolCalls[id] ?? blankToolCall(id, name);
  const call: ToolCallState = { ...known, name, status: isError ? 'error' : 'ok', partial: null, result, resultText: resultText(result), touchedPaths };

  return attach({ ...state, toolCalls: { ...state.toolCalls, [id]: call } }, id);
}

/** The turn is over: calls that never reported an end were interrupted. */
export function interruptRunningTools(state: ChatState): ChatState {
  const running = Object.values(state.toolCalls).filter((call) => call.status === 'running');
  if (running.length === 0) {
    return state;
  }

  const toolCalls = { ...state.toolCalls };
  for (const call of running) {
    toolCalls[call.id] = { ...call, status: 'error', partial: null, resultText: call.resultText ?? t('chat.tool.interrupted') };
  }

  return { ...state, toolCalls };
}
