import { splitFileNote } from './attachments';
import { splitHooks } from './hook-context';
import { splitRecall } from './memory-recall';
import { appendItem } from './state';
import type { ChatState, DisplayBlock, MessageItem, TranscriptItem } from './types';

/** The row key of a message shown before its echo: the echo keeps it as the list key (`SendTrace.id`), so the row stays put. */
export function pendingKey(id: string): string {
  return `p:${id}`;
}

/**
 * The words the user typed in a prompt as sent or echoed: what remains after the app's memory block, hook context and file
 * list are taken off. Null for a message a hook sent, which carries no words of the user's to match.
 */
export function ownWords(text: string): string | null {
  const hooks = splitHooks(splitRecall(text).text);

  if (hooks.followUp.length > 0) {
    return null;
  }

  return splitFileNote(hooks.text).text.trim();
}

function isSending(item: TranscriptItem): item is MessageItem & { send: NonNullable<MessageItem['send']> } {
  return item.kind === 'message' && item.role === 'user' && item.send?.status === 'sending';
}

/** Shows the user's message at once, before the harness has echoed it. `message` is the text as sent, `imageTypes` the types of its images. */
export function startSend(state: ChatState, id: string, message: string, imageTypes: string[]): ChatState {
  const blocks: DisplayBlock[] = [{ type: 'text', text: message }, ...imageTypes.map((mimeType): DisplayBlock => ({ type: 'image', mimeType }))];
  const words = ownWords(message) ?? '';

  return appendItem(
    state,
    (key, order) => ({ kind: 'message', key, order, role: 'user', blocks, streaming: false, slots: {}, meta: null, send: { id, status: 'sending', words } }),
    pendingKey(id),
  );
}

/** True while a message waits for its echo, so a user message that starts is not shown empty before its words are known. */
export function hasPendingSend(state: ChatState): boolean {
  return state.items.some(isSending);
}

/** The position of the oldest message still waiting for the echo that says `text`, or -1. First in, first matched. */
export function pendingIndexFor(state: ChatState, text: string): number {
  const words = ownWords(text);

  if (words === null) {
    return -1;
  }

  return state.items.findIndex((item) => isSending(item) && item.send.words === words);
}

/** Takes the message back out (the harness refused it, or the chat starts over). */
export function dropSend(state: ChatState, id: string): ChatState {
  const key = pendingKey(id);
  const items = state.items.filter((item) => !(item.key === key && isSending(item)));

  return items.length === state.items.length ? state : { ...state, items };
}

/**
 * The harness will not echo the messages still waiting (its prompt failed, the turn ended without them, the process exited).
 * Each stays in the transcript as the message the user sent, no longer waiting, and the newest one is what Retry sends again.
 */
export function settleSends(state: ChatState, scope: 'all' | 'started' = 'all'): ChatState {
  if (!hasPendingSend(state)) {
    return state;
  }

  let lastUserText = state.lastUserText;
  const items = state.items.map((item): TranscriptItem => {
    // A plain settle may be a late one from before the send (OMP settles after its turn ends): it only settles a message
    // whose own turn started. An error or an exit settles every waiting message.
    if (!isSending(item) || (scope === 'started' && !item.send.turnStarted)) {
      return item;
    }

    const sent = item.blocks.find((block) => block.type === 'text');
    lastUserText = sent?.type === 'text' ? sent.text : lastUserText;

    return { ...item, send: { ...item.send, status: 'sent' } };
  });

  return { ...state, items, lastUserText };
}

/** A turn started: every waiting message's echo is now part of it, so the settle after it belongs to them. */
export function markTurnStarted(state: ChatState): ChatState {
  if (!hasPendingSend(state)) {
    return state;
  }

  return {
    ...state,
    items: state.items.map((item): TranscriptItem => (isSending(item) && !item.send.turnStarted ? { ...item, send: { ...item.send, turnStarted: true } } : item)),
  };
}
