import { splitFileNote } from './attachments';
import { splitHooks } from './hook-context';
import { splitRecall } from './memory-recall';
import type { ChatState } from './types';

/** What the chat was opened with: the user's first message without the hooks, recall and file notes the shell added. Empty while older messages are not loaded. */
export function firstUserPrompt(chat: ChatState): string {
  if (chat.historyCursor !== null) {
    return '';
  }

  const first = chat.items.find((item) => item.kind === 'message' && item.role === 'user');

  if (first?.kind !== 'message') {
    return '';
  }

  const sent = first.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n');

  return splitFileNote(splitHooks(splitRecall(sent).text).text).text;
}
