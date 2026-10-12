import { tMaybe } from '$lib/i18n/i18n.svelte';

/** A failure cause in words: "edit_mismatch" reads "edit mismatch". A cause the app has no words for keeps the dashboard's name, with spaces for underscores. */
export function causeWords(cause: string): string {
  return tMaybe(`ecosystem.cause.${cause}`) ?? cause.replaceAll('_', ' ');
}

/** The causes the dashboard names; anything else (its own "other" included) folds into "Other". */
const KNOWN_CAUSES = ['exit_code', 'not_found', 'edit_mismatch', 'rejected', 'timeout', 'permission', 'stale_read', 'bad_input'];

/** At most this many causes get their own segment; the rest fold into "Other". */
export const FEATURED_CAUSES = 4;

export interface CauseShare {
  key: string;
  /** True for the one part that folds every remaining cause. */
  folded: boolean;
  count: number;
  share: number;
  /** The causes behind the segment: one, or every folded one. */
  causes: string[];
  tools: string[];
}

/**
 * The largest known causes, each as its own part, then everything else (the dashboard's own "other" included) as one
 * "Other" part, so the chart stays readable however many causes a range has.
 */
export function causeShares(causes: { cause: string; count: number; share: number; tools: string[] }[]): CauseShare[] {
  const ranked = [...causes].sort((a, b) => b.count - a.count);
  const featured = ranked.filter((cause) => KNOWN_CAUSES.includes(cause.cause)).slice(0, FEATURED_CAUSES);
  const folded = ranked.filter((cause) => !featured.includes(cause));

  const parts: CauseShare[] = featured.map((cause) => ({
    key: cause.cause,
    folded: false,
    count: cause.count,
    share: cause.share,
    causes: [cause.cause],
    tools: cause.tools,
  }));

  if (folded.length > 0) {
    parts.push({
      key: 'other',
      folded: true,
      count: folded.reduce((sum, cause) => sum + cause.count, 0),
      share: folded.reduce((sum, cause) => sum + cause.share, 0),
      causes: folded.map((cause) => cause.cause),
      tools: [...new Set(folded.flatMap((cause) => cause.tools))],
    });
  }

  return parts;
}
