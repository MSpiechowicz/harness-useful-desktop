import { expandWrapped } from './wrapped-calls';
import { docUse, type KnowledgeUseIndex } from './knowledge-use';
import { type FoundDoc, knowledgeToolName, parseDocs } from './knowledge-tools';
import { type FoundNote, memoryToolName, parseSearch } from './memory-tools';
import { type MemoryUseIndex, noteUse, type SearchUse } from './memory-use';
import { isSteer, messageRecall, recallCallId } from './recall-rows';
import { fullPrimaryArg } from './tool-meta';
import type { DocUse } from './knowledge-use';
import type { RowUse } from './use-index';
import type { MessageItem, ToolCallState, TranscriptItem } from './types';

/** The part of a reply the Status report card reads. */
export interface ReplyParts {
  /** The message of the user the reply answers; null for a transcript page that starts after it. */
  message: MessageItem | null;
  /** The tool calls of the reply up to and including `key`, in the order of the screen. */
  calls: ToolCallState[];
}

/** What one search of a reply found. */
export interface CheckedGroup<Row> {
  key: string;
  /** The words it searched for, when they are known. */
  query: string | null;
  /** What the app found for the message itself, not a search of the agent. */
  recalled: boolean;
  rows: { row: Row; use: RowUse | null }[];
}

function callIdsOf(item: TranscriptItem): string[] {
  if (item.kind === 'tool') {
    return [item.toolCallId];
  }

  if (item.kind === 'message' && item.role === 'assistant') {
    return item.blocks.flatMap((block) => (block.type === 'tool' ? [block.toolCallId] : []));
  }

  return [];
}

/**
 * Walks back from the message `key` to the message of the user that started its reply, so a long chat costs only the length of the
 * reply. A steer ends the reply, unless the reply began with what the app found for a message: then it goes on, as in `buildUseIndex`.
 */
export function replyOf(items: TranscriptItem[], key: string, toolCalls: Record<string, ToolCallState>): ReplyParts {
  let calls: ToolCallState[] = [];
  let nearest: ReplyParts | null = null;
  let reached = false;

  for (let index = items.length - 1; index >= 0; index -= 1) {
    const item = items[index];

    reached ||= item.key === key;

    if (!reached) {
      continue;
    }

    if (item.kind !== 'message' || item.role !== 'user') {
      calls = [...callIdsOf(item).flatMap((id) => (toolCalls[id] ? expandWrapped(toolCalls[id]) : [])), ...calls];
      continue;
    }

    nearest ??= { message: item, calls };

    if (!isSteer(item)) {
      const recall = messageRecall(item);

      return nearest.message === item || recall.memory || recall.knowledge ? { message: item, calls } : nearest;
    }
  }

  return nearest ?? { message: null, calls };
}

/** A row is listed under the first search that found it, which is the one the later calls of the reply are held against. */
function grouped<Row extends { id: K }, K>(found: { key: string; query: string | null; recalled?: boolean; rows: Row[]; useOf: (id: K) => RowUse | null }[]): CheckedGroup<Row>[] {
  const seen = new Set<K>();
  const groups: CheckedGroup<Row>[] = [];

  for (const { key, query, recalled = false, rows, useOf } of found) {
    const fresh = rows.filter((row) => !seen.has(row.id));

    fresh.forEach((row) => seen.add(row.id));

    if (fresh.length > 0) {
      groups.push({ key, query, recalled, rows: fresh.map((row) => ({ row, use: useOf(row.id) })) });
    }
  }

  return groups;
}

/** The notes a reply checked: what the app found for the message, then each `note_search`. */
export function checkedNotes({ message, calls }: ReplyParts, index: MemoryUseIndex | undefined): CheckedGroup<FoundNote>[] {
  const useOf = (id: string) => {
    const use: SearchUse | undefined = index?.get(id);

    return (note: number) => noteUse(use, note);
  };
  const found = [];
  const recalled = message && messageRecall(message).memory;

  if (message && recalled) {
    const key = recallCallId(message.key, 'memory');

    found.push({ key, query: null, recalled: true, rows: recalled.notes, useOf: useOf(key) });
  }

  for (const call of calls) {
    if (memoryToolName(call.name) === 'note_search' && call.status === 'ok') {
      found.push({ key: call.id, query: fullPrimaryArg(call.args), rows: parseSearch(call.resultText).notes, useOf: useOf(call.id) });
    }
  }

  return grouped<FoundNote, number>(found);
}

/** The documents a reply checked: what the app found for the message, then each `knowledge_search`. */
export function checkedDocs({ message, calls }: ReplyParts, index: KnowledgeUseIndex | undefined): CheckedGroup<FoundDoc>[] {
  const useOf = (id: string) => {
    const use: DocUse | undefined = index?.get(id);

    return (doc: string) => docUse(use, doc);
  };
  const found = [];
  const recalled = message && messageRecall(message).knowledge;

  if (message && recalled) {
    const key = recallCallId(message.key, 'knowledge');

    found.push({ key, query: null, recalled: true, rows: recalled.docs, useOf: useOf(key) });
  }

  for (const call of calls) {
    if (knowledgeToolName(call.name) === 'knowledge_search' && call.status === 'ok') {
      found.push({ key: call.id, query: fullPrimaryArg(call.args), rows: parseDocs(call.resultText), useOf: useOf(call.id) });
    }
  }

  return grouped<FoundDoc, string>(found);
}

/** How many rows the groups list, and how many of them the reply used. */
export function checkedCounts(groups: CheckedGroup<unknown>[]): { checked: number; used: number } {
  const rows = groups.flatMap((group) => group.rows);

  return { checked: rows.length, used: rows.filter((row) => row.use !== null).length };
}
