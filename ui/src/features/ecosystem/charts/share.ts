/** One part of a part-to-whole chart (waffle, donut) and its legend row. */
export interface ShareSegment {
  key: string;
  label: string;
  value: number;
  /** The exact figure, e.g. "231" or "$1,424.58". */
  valueLabel: string;
  /** The share, e.g. "42%". */
  shareLabel: string;
  /** A second line under the label in the legend, e.g. what was folded into "Other". */
  note?: string;
  /** Spoken and shown on hover: every exact figure of the segment. */
  description: string;
  color: string;
}
