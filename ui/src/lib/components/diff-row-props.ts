import type { PickHandlers } from '$lib/git/comment-range';

/** What both ways of laying out a diff line (unified, split) are given besides the line itself. */
export interface DiffRowShared {
  pick: PickHandlers;
  /** The normalized search query, and the flat index of the match being stepped through. */
  query?: string | null;
  currentMatch?: number;
}
