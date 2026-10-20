/** The texts of a menu's search field: its accessible name, its hint and the line shown when nothing matches. */
export interface MenuSearchTexts {
  label: string;
  placeholder: string;
  empty: string;
}

/** The options whose label or hint has what was typed (in any case); all of them while nothing is typed. */
export function matchOptions<O extends { label: string; hint?: string }>(options: O[], needle: string): O[] {
  const typed = needle.trim().toLowerCase();

  if (typed === '') {
    return options;
  }

  return options.filter((option) => option.label.toLowerCase().includes(typed) || (option.hint?.toLowerCase().includes(typed) ?? false));
}
