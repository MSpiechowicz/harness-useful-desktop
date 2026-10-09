export interface BarDatum {
  key: string;
  label: string;
  /** Zero-based magnitude the bar encodes. */
  value: number;
  /** The exact value printed at the end of the bar. */
  valueLabel: string;
  /** Spoken and shown on hover/focus: every exact figure of the row. */
  description: string;
  color: string;
}
