import FilePlus from '@lucide/svelte/icons/file-plus';
import GitPullRequestDraft from '@lucide/svelte/icons/git-pull-request-draft';
import Circle from '@lucide/svelte/icons/circle';
import CircleCheck from '@lucide/svelte/icons/circle-check';
import MessageSquareWarning from '@lucide/svelte/icons/message-square-warning';
import Replace from '@lucide/svelte/icons/replace';
import Send from '@lucide/svelte/icons/send';
import Undo2 from '@lucide/svelte/icons/undo-2';

import type { IconComponent } from '$lib/components/icon';
import { type MessageKey, t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeEventKind } from '$lib/ipc/bindings/KnowledgeEventKind';
import type { KnowledgeHistoryEvent } from '$lib/ipc/bindings/KnowledgeHistoryEvent';
import { joinNames } from '$lib/utils/format';

/** The events of a live document and of the pending version that will replace it as one timeline, newest first. */
export function joinHistory(pending: readonly KnowledgeHistoryEvent[], live: readonly KnowledgeHistoryEvent[]): KnowledgeHistoryEvent[] {
  return [...pending, ...live].sort((left, right) => Date.parse(right.at) - Date.parse(left.at));
}

/** Events a few minutes apart by the same person read as one line: a file replaced and approval asked straight after are one step to the reader. */
const MERGE_MS = 5 * 60_000;

/** The kinds that prepare a version: a later one by the same person joins them. */
const PREPARING: readonly KnowledgeEventKind[] = ['drafted', 'updateDrafted', 'edited'];

/** What each kind may join: replacing a file joins what prepared the version, asking joins the same. */
const JOINS: Partial<Record<KnowledgeEventKind, readonly KnowledgeEventKind[]>> = { edited: PREPARING, asked: PREPARING };

const VERBS: Record<KnowledgeEventKind, MessageKey> = {
  drafted: 'knowledge.drafts.history.drafted',
  updateDrafted: 'knowledge.drafts.history.updateDrafted',
  edited: 'knowledge.drafts.history.edited',
  asked: 'knowledge.drafts.history.asked',
  changesRequested: 'knowledge.drafts.history.changesRequested',
  approved: 'knowledge.drafts.history.approved',
  wentLive: 'knowledge.drafts.history.wentLive',
  withdrawn: 'knowledge.drafts.history.withdrawn',
  other: 'knowledge.drafts.history.changed',
};

const ICONS: Record<KnowledgeEventKind, IconComponent> = {
  drafted: FilePlus,
  updateDrafted: GitPullRequestDraft,
  edited: Replace,
  asked: Send,
  changesRequested: MessageSquareWarning,
  approved: CircleCheck,
  wentLive: CircleCheck,
  withdrawn: Undo2,
  other: Circle,
};

/** The glyphs that carry colour: an approval is green, a request for changes amber. Every other glyph is neutral. */
export const GLYPH_TONES: Partial<Record<KnowledgeEventKind, string>> = { approved: 'text-success', wentLive: 'text-success', changesRequested: 'text-warning' };

export interface HistoryEntry {
  /** A key that no other entry of the list has: the place of the entry counted from the oldest, the time and the kinds (events that share a time and a kind are still two entries). */
  key: string;
  by: string | null;
  at: string;
  kinds: KnowledgeEventKind[];
  /** What the person wrote with it: the request for changes, or the note that came with the ask. */
  comment: string | null;
}

/** A name as the server compares them: trimmed, without regard to case. */
const normalized = (name: string | null | undefined): string | null => name?.trim().toLowerCase() ?? null;

/** The key of the entry at `index` (counted from the oldest, so a newer event does not rename the older entries). */
function entryKey(index: number, entry: Pick<HistoryEntry, 'at' | 'kinds'>): string {
  return `${index}:${entry.at}:${entry.kinds.join('+')}`;
}

/** The events of a document as entries, newest first: the ones that belong together are merged. The server prints them newest first. */
export function historyEntries(events: readonly KnowledgeHistoryEvent[]): HistoryEntry[] {
  const entries: HistoryEntry[] = [];

  for (const event of [...events].reverse()) {
    const last = entries.at(-1);
    const joins = JOINS[event.kind];
    const same = last !== undefined && normalized(last.by) === normalized(event.by);

    if (
      last &&
      joins &&
      same &&
      Date.parse(event.at) - Date.parse(last.at) <= MERGE_MS &&
      last.kinds.every((kind) => PREPARING.includes(kind)) &&
      joins.includes(last.kinds.at(-1) as KnowledgeEventKind)
    ) {
      last.kinds.push(event.kind);
      last.at = event.at;
      last.comment = event.comment ?? last.comment;
      last.key = entryKey(entries.length - 1, last);

      continue;
    }

    const entry: HistoryEntry = { key: '', by: event.by ?? null, at: event.at, kinds: [event.kind], comment: event.comment ?? null };

    entry.key = entryKey(entries.length, entry);
    entries.push(entry);
  }

  return entries.reverse();
}

/** What the entry did, as the end of the sentence: "added it as a draft and asked for approval". */
export function verbsText(entry: Pick<HistoryEntry, 'kinds'>): string {
  return joinNames(entry.kinds.map((kind) => t(VERBS[kind])));
}

/** The glyph of an entry: the one of its last step. */
export function glyphOf(entry: Pick<HistoryEntry, 'kinds'>): { icon: IconComponent; tone: string | undefined } {
  const kind = entry.kinds.at(-1) ?? 'other';

  return { icon: ICONS[kind], tone: GLYPH_TONES[kind] };
}
