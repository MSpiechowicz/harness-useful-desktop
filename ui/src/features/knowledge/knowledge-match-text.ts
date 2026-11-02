import { type MessageKey, t, tn } from '$lib/i18n/i18n.svelte';
import type { KnowledgeMatchReason } from '$lib/ipc/bindings/KnowledgeMatchReason';
import type { KnowledgePlace } from '$lib/ipc/bindings/KnowledgePlace';
import { type Landing, type MatchChoices, type MatchRow, summaryOf } from '$lib/utils/knowledge-match';

/** Why a stored document counts as a version of a file, the strongest first. */
export const REASON_LABELS = {
  sourceKey: 'knowledge.match.reason.sourceKey',
  sameContent: 'knowledge.match.reason.sameContent',
  sharedChunks: 'knowledge.match.reason.sharedChunks',
  title: 'knowledge.match.reason.title',
} as const satisfies Record<KnowledgeMatchReason, MessageKey>;

/** Where a matching document is kept, in full: the team's database or this computer. */
export function placeLabelOf(place: KnowledgePlace): string {
  return t(place === 'local' ? 'settings.memoryRemote.target.local' : 'knowledge.settings.remote.title');
}

/** "4 will update, 1 will be added": a part that is zero is left out, and nothing is said when no file can be compared. */
export function summaryText(rows: readonly MatchRow[], choices: MatchChoices, landing: Landing): string {
  const { update, add } = summaryOf(rows, choices, landing);

  return [...(update > 0 ? [tn('knowledge.match.update.count', update)] : []), ...(add > 0 ? [tn('knowledge.match.add.count', add)] : [])].join(', ');
}
