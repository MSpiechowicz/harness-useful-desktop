/** One part that differs between the version the user approved and the one waiting. `from` is null when there was no approved version, or the part was empty. */
export interface DocChange<F extends string> {
  field: F;
  from: string | null;
  to: string | null;
}

/**
 * What differs between a waiting document and the version the user approved, part by part in `order`. Without an approved
 * version (an agent added the document) every part it sets is listed as new, so the review shows what it would do before it ever runs.
 */
export function diffFields<F extends string>(order: readonly F[], waiting: Record<F, string | null>, approved: Record<F, string | null> | null): DocChange<F>[] {
  return order.flatMap((field) => {
    const before = approved?.[field] ?? null;
    const after = waiting[field];

    if (approved === null ? after === null : before === after) {
      return [];
    }

    return [{ field, from: before, to: after }];
  });
}

/** One part that differs, with its values already written as a person reads them. */
export interface ReviewChange {
  key: string;
  /** The name of the part: "Name", "Prompt". */
  label: string;
  /** A text part is shown as a line diff, any other as the old value struck out above the new one. */
  text: boolean;
  from: string | null;
  to: string | null;
  /** A value that is code or a path, set in the mono face. */
  mono?: boolean;
}

/** How a feature words the parts of its document in a review. */
export interface ReviewWords<F extends string> {
  /** The name of a part. */
  label: (field: F) => string;
  /** A value as a person reads it: a switch as on or off, a path with its hidden characters spelled out. */
  value: (field: F, value: string | null) => string | null;
  /** The part that is text, shown as a line diff. */
  text: F;
  /** The parts that are code or a path, set in the mono face. */
  mono: ReadonlySet<F>;
}

/** The changes of a review, worded for the reader. */
export function reviewChanges<F extends string>(changes: DocChange<F>[], words: ReviewWords<F>): ReviewChange[] {
  return changes.map((change) => ({
    key: change.field,
    label: words.label(change.field),
    text: change.field === words.text,
    from: words.value(change.field, change.from),
    to: words.value(change.field, change.to),
    mono: words.mono.has(change.field),
  }));
}

/** What a review needs from its view: whether a change is being written, and what approving and rejecting do. */
export interface DocReviewHandlers {
  busy: boolean;
  onapprove: () => void;
  /** Left out for a team document, which the user can only approve or leave: the team's copy is not theirs to undo. */
  onreject?: () => void;
}
