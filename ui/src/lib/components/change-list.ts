import type { IconComponent } from './icon';

/** Plain data for `ChangeList` and the finish-review dialog: nothing in it knows where the changes come from (git, a document). */

export type ChangeTone = 'success' | 'warning' | 'danger' | 'info' | 'neutral';

/** One changed thing in a list, e.g. a file with its status letter and line counts. */
export interface ChangeItem {
  /** Unique within the whole list, even when the same thing sits under two sections. */
  id: string;
  /** The path or name shown, cut at its last "/" into a dimmed folder and the name. Also the tooltip. */
  label: string;
  /** A second, quieter line, e.g. "old/name → new/name". */
  sublabel?: string;
  /** The status as one letter (A, M, D…) and its spoken name. */
  letter: string;
  letterLabel: string;
  tone: ChangeTone;
  /** Added and removed lines. Both null (or absent) while unknown or for a binary change: nothing is shown then. */
  additions?: number | null;
  deletions?: number | null;
  /** Pending review comments on this item. */
  comments: number;
  /** Lines matching a search. Zero or absent shows no chip. */
  hits?: number;
}

/** A heading with the items under it. */
export interface ChangeSection {
  id: string;
  title: string;
  items: ChangeItem[];
}

/** One line comment as the finish-review dialog lists it. */
export interface ReviewLineComment {
  id: number;
  /** The file (or document) the comment is on, as the dialog groups by it. */
  path: string;
  side: 'old' | 'new';
  startLine: number;
  endLine: number;
  body: string;
}

/** One way to finish a review (or one place to send it): a row of a `ChoiceList`. */
export interface Choice {
  value: string;
  title: string;
  detail?: string;
  /** Lucide icon shown before the title. */
  icon?: IconComponent;
}
