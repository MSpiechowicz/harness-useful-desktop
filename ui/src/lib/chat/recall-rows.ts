import { splitHooks } from './hook-context';
import { type FoundDoc, parseRecallDocs } from './knowledge-tools';
import { splitRecall } from './memory-recall';
import { type FoundNote, type FoundScript, parseRecallScripts, parseSearch } from './memory-tools';
import { splitStepRecall } from './step-recall';
import type { MessageItem } from './types';

/** The notes and saved scripts the app found for a message (the Harness Memory step), and the block as the agent got it. */
export interface MemoryRecallRows {
  notes: FoundNote[];
  scripts: FoundScript[];
  text: string;
}

/** The documents the app found for a message (the Harness Knowledge step), and the block as the agent got it. */
export interface KnowledgeRecallRows {
  docs: FoundDoc[];
  text: string;
}

/** What the app found for one message of the user. A family with nothing readable in its block is null: the hook card then shows the block as it is. */
export interface MessageRecall {
  memory: MemoryRecallRows | null;
  knowledge: KnowledgeRecallRows | null;
}

const NONE: MessageRecall = { memory: null, knowledge: null };
const cache = new WeakMap<MessageItem, MessageRecall>();

/** A message of the user that is neither a prompt the app added hook context to nor a hook follow-up: a steer the agent took mid-reply (or a prompt sent with every hook off). */
export function isSteer(item: MessageItem): boolean {
  const text = item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n');
  const { context, followUp } = splitHooks(splitRecall(text).text);

  return context.length === 0 && followUp.length === 0;
}

/** The id the use index gives the search the app made for a message, which its card is looked up by. */
export function recallCallId(messageKey: string, family: 'memory' | 'knowledge'): string {
  return `recall:${messageKey}:${family}`;
}

/** The rows a Harness Memory recall block lists, as a search would list them. Null when it lists none. */
export function memoryRecallRows(body: string): MemoryRecallRows | null {
  const notes = parseSearch(body).notes;
  const scripts = parseRecallScripts(body);

  return notes.length + scripts.length > 0 ? { notes, scripts, text: body } : null;
}

/** The rows a Harness Knowledge recall block lists. Null when it lists none. */
export function knowledgeRecallRows(body: string): KnowledgeRecallRows | null {
  const docs = parseRecallDocs(body);

  return docs.length > 0 ? { docs, text: body } : null;
}

/**
 * What the app found for a message of the user, read from the Harness Memory and Harness Knowledge steps of the hook context
 * in front of it (a step is told from its block, not its name). The first block of each family counts. A message is not edited
 * once it is in the list, so it is read once.
 */
export function messageRecall(item: MessageItem): MessageRecall {
  if (item.role !== 'user') {
    return NONE;
  }

  const known = cache.get(item);
  if (known) {
    return known;
  }

  const text = item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n');
  const found: MessageRecall = { memory: null, knowledge: null };

  if (text.includes('<harness-hook-context>')) {
    for (const hook of splitHooks(splitRecall(text).text).context) {
      const { recall } = splitStepRecall(hook.text);

      if (recall?.kind === 'memory' && !found.memory) {
        found.memory = memoryRecallRows(recall.body);
      } else if (recall?.kind === 'knowledge' && !found.knowledge) {
        found.knowledge = knowledgeRecallRows(recall.body);
      }
    }
  }

  const result = found.memory || found.knowledge ? found : NONE;
  cache.set(item, result);

  return result;
}
