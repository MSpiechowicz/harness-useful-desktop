import { formatDateTime, relativeTime } from '$lib/utils/format';

/** One labelled line of a facts card: the label on the left, the value on the right. */
export interface FactRow {
  label: string;
  value: string;
  mono?: boolean;
  /** The full text for the tooltip when the value shown is a shortened form of it. */
  title?: string;
  /** A switched-on state, drawn with a green dot. */
  on?: boolean;
  /** A value that asks for attention: amber for something to look at, red for something that went wrong. */
  tone?: 'warning' | 'danger';
  /** A value whose end says more than its start, such as a folder: when it does not fit, its start is cut. */
  cutStart?: boolean;
}

/** A moment as people read it ("2 days ago"), with the exact date and time in the tooltip. */
export function momentFact(label: string, iso: string): FactRow {
  return { label, value: relativeTime(iso), title: formatDateTime(iso) };
}
