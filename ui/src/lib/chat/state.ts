import type { ChatState, TranscriptItem } from './types';

export function initialChatState(key: string): ChatState {
  return {
    key,
    lastSeq: 0,
    gaps: 0,
    phase: 'starting',
    settledCount: 0,
    readyCount: 0,
    items: [],
    toolCalls: {},
    subagents: {},
    openRequests: [],
    queue: { steering: [], followUp: [], liveSteered: 0 },
    statuses: {},
    widgets: {},
    title: null,
    session: null,
    commands: null,
    promptError: null,
    crash: null,
    stderrTail: [],
    editorText: null,
    protocolVersion: null,
    lastUserText: null,
    historyCursor: null,
    historyTotal: null,
    nextOrder: 0,
  };
}

export function findLastIndex<T>(list: readonly T[], matches: (item: T) => boolean): number {
  for (let index = list.length - 1; index >= 0; index -= 1) {
    if (matches(list[index])) {
      return index;
    }
  }

  return -1;
}

/** Adds an item at the end; `make` receives the key and creation order the state hands out. */
export function appendItem(state: ChatState, make: (key: string, order: number) => TranscriptItem, key?: string): ChatState {
  const order = state.nextOrder;

  return { ...state, items: [...state.items, make(key ?? `i${order}`, order)], nextOrder: order + 1 };
}

/** Replaces the item with this key (searching from the end, where live updates happen). */
export function updateItem(state: ChatState, key: string, update: (item: TranscriptItem) => TranscriptItem): ChatState {
  const index = findLastIndex(state.items, (item) => item.key === key);
  if (index === -1) {
    return state;
  }

  const items = state.items.slice();
  items[index] = update(items[index]);

  return { ...state, items };
}

export function messageKey(messageId: string): string {
  return `m:${messageId}`;
}
