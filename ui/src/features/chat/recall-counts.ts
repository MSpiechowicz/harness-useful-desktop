import { tn } from '$lib/i18n/i18n.svelte';
import type { KnowledgeRecallRows, MemoryRecallRows } from '$lib/chat/recall-rows';

/** "2 notes", "1 script" or "2 notes · 1 script": a part with nothing in it is left out. */
export function memoryCounts(rows: Pick<MemoryRecallRows, 'notes' | 'scripts'>): string {
  return [
    rows.notes.length > 0 ? tn('chat.user.recallCount', rows.notes.length) : '',
    rows.scripts.length > 0 ? tn('chat.user.recallScriptCount', rows.scripts.length) : '',
  ]
    .filter(Boolean)
    .join(' · ');
}

/** "2 documents". */
export function knowledgeCounts(rows: Pick<KnowledgeRecallRows, 'docs'>): string {
  return tn('hooks.chip.recall.pointers', rows.docs.length);
}
