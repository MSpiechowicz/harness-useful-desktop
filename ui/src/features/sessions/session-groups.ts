import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';

export interface SessionGroups {
  /** Every session that is not a folded empty copy, in list order. */
  rows: SessionSummary[];
  /** The empty copies folded under a row, by the id of that row, in list order. */
  copiesByRoot: Map<string, SessionSummary[]>;
  /** Every empty copy of the list, folded or not. */
  emptyCopies: SessionSummary[];
}

/**
 * Folds the empty copies a restart leaves behind under the session they copy. A copy of a copy folds under the
 * session at the end of the chain; one whose original is not in the list stays a row of its own.
 */
export function groupSessions(sessions: SessionSummary[]): SessionGroups {
  const byId = new Map(sessions.map((summary) => [summary.ref.id, summary]));
  const emptyCopies = sessions.filter((summary) => summary.emptyCopy);
  const copiesByRoot = new Map<string, SessionSummary[]>();
  const folded = new Set<string>();

  for (const copy of emptyCopies) {
    const root = rootOf(copy, byId);
    if (!root) {
      continue;
    }

    folded.add(copy.ref.id);
    copiesByRoot.set(root.ref.id, [...(copiesByRoot.get(root.ref.id) ?? []), copy]);
  }

  return { rows: sessions.filter((summary) => !folded.has(summary.ref.id)), copiesByRoot, emptyCopies };
}

/** The session an empty copy ultimately copies: the first one in its chain of parents that is not itself an empty copy. */
function rootOf(copy: SessionSummary, byId: Map<string, SessionSummary>): SessionSummary | null {
  const seen = new Set<string>();
  let current = copy;

  while (current.emptyCopy && !seen.has(current.ref.id)) {
    seen.add(current.ref.id);

    const parent = current.parentSession === null ? undefined : byId.get(current.parentSession);
    if (!parent) {
      return null;
    }

    current = parent;
  }

  return current.emptyCopy ? null : current;
}

/** How many sessions were forked from session `id`; the empty copies a restart leaves behind are no forks. */
export function forkCount(sessions: SessionSummary[], id: string): number {
  return sessions.filter((summary) => summary.parentSession === id && !summary.emptyCopy).length;
}
