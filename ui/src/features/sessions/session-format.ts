import type { SessionNode } from '$lib/ipc/bindings/SessionNode';
import type { SessionNodeKind } from '$lib/ipc/bindings/SessionNodeKind';
import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import { formatDateTime, perLocale } from '$lib/utils/format';

const TITLE_MAX = 80;

/** The session's title, else its first prompt, else "Untitled". */
export function sessionTitle(summary: SessionSummary): string {
  const text = (summary.title ?? summary.firstPrompt ?? '').replace(/\s+/g, ' ').trim();
  if (text === '') {
    return t('sessions.untitled');
  }

  return text.length > TITLE_MAX ? `${text.slice(0, TITLE_MAX - 1)}…` : text;
}

/** Every whitespace-separated word of the query must appear in the title, first prompt, model or id. */
export function matchesQuery(summary: SessionSummary, query: string): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const haystack = [summary.title, summary.firstPrompt, summary.model, summary.ref.id].join('\n').toLowerCase();

  return words.every((word) => haystack.includes(word));
}

const KIND_KEYS = {
  user: 'sessions.kind.user',
  assistant: 'sessions.kind.assistant',
  toolResult: 'sessions.kind.toolResult',
  bash: 'sessions.kind.bash',
  compaction: 'sessions.kind.compaction',
  branchSummary: 'sessions.kind.branchSummary',
  label: 'sessions.kind.label',
  modelChange: 'sessions.kind.modelChange',
  other: 'sessions.kind.entry',
} as const satisfies Record<SessionNodeKind, MessageKey>;

export function kindLabel(kind: SessionNodeKind): string {
  return t(KIND_KEYS[kind]);
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

const TOKENS = perLocale((l) => new Intl.NumberFormat(l, { notation: 'compact', maximumFractionDigits: 1 }));
const MINUTES = perLocale((l) => new Intl.NumberFormat(l, { style: 'unit', unit: 'minute', unitDisplay: 'short' }));
const HOURS = perLocale((l) => new Intl.NumberFormat(l, { style: 'unit', unit: 'hour', unitDisplay: 'short' }));

/** A token count in the short form of the language: `14.5K`. */
export function formatTokens(tokens: number): string {
  return TOKENS().format(tokens);
}

/** A span of time in whole minutes and hours: `44 min`, `1 hr 5 min`; under a minute it reads `< 1 min`. */
export function formatDuration(ms: number): string {
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 1) {
    return `< ${MINUTES().format(1)}`;
  }

  const hours = Math.floor(minutes / 60);
  if (hours < 1) {
    return MINUTES().format(minutes);
  }

  return minutes % 60 === 0 ? HOURS().format(hours) : `${HOURS().format(hours)} ${MINUTES().format(minutes % 60)}`;
}
