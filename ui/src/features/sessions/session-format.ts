import type { SessionNode } from '$lib/ipc/bindings/SessionNode';
import type { SessionNodeKind } from '$lib/ipc/bindings/SessionNodeKind';
import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
import { formatDateTime } from '$lib/utils/format';

const TITLE_MAX = 80;

/** The session's title, else its first prompt, else "Untitled". */
export function sessionTitle(summary: SessionSummary): string {
  const text = (summary.title ?? summary.firstPrompt ?? '').replace(/\s+/g, ' ').trim();
  if (text === '') {
    return 'Untitled';
  }

  return text.length > TITLE_MAX ? `${text.slice(0, TITLE_MAX - 1)}…` : text;
}

/** Every whitespace-separated word of the query must appear in the title, first prompt, model or id. */
export function matchesQuery(summary: SessionSummary, query: string): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const haystack = [summary.title, summary.firstPrompt, summary.model, summary.ref.id].join('\n').toLowerCase();

  return words.every((word) => haystack.includes(word));
}

const KIND_LABELS: Record<SessionNodeKind, string> = {
  user: 'You',
  assistant: 'Assistant',
  toolResult: 'Tool result',
  bash: 'Command',
  compaction: 'Compaction',
  branchSummary: 'Branch summary',
  label: 'Label',
  modelChange: 'Model change',
  other: 'Entry',
};

export function kindLabel(kind: SessionNodeKind): string {
  return KIND_LABELS[kind];
}

/** The message text of a user or assistant entry on one line; empty for other kinds. */
export function previewText(node: SessionNode): string {
  return node.preview?.replace(/\s+/g, ' ').trim() ?? '';
}

/** Tooltip of a tree row: kind, time and the text. */
export function nodeTooltip(node: SessionNode): string {
  const time = node.timestamp ? formatDateTime(node.timestamp) : '';
  const when = time ? ` · ${time}` : '';

  return `${kindLabel(node.kind)}${when}\n${node.preview ?? ''}`.trim();
}
