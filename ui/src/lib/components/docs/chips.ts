/** How a chip field reads typed or pasted text: where one chip ends and what a chip looks like. */
export interface ChipRules {
  /** Separates one chip from the next. Without the `g` flag. */
  split: RegExp;
  /** The chip a piece of text makes: trimmed, lowercased, and so on. */
  normalize: (piece: string) => string;
}

/** The chips of a text, each as the rules make it, none empty. */
export function chipsOf(text: string, rules: ChipRules): string[] {
  return text
    .split(rules.split)
    .map(rules.normalize)
    .filter((piece) => piece !== '');
}

/** What makes two chips the same: any case, inner spaces collapsed. */
function chipKey(chip: string): string {
  return chip.replace(/\s+/g, ' ').toLowerCase();
}

/** `values` followed by the `added` ones that are not among them yet, each chip once. */
export function mergeChips(values: readonly string[], added: readonly string[]): string[] {
  const seen = new Set<string>();

  return [...values, ...added].filter((chip) => {
    const key = chipKey(chip);

    if (seen.has(key)) {
      return false;
    }

    seen.add(key);

    return true;
  });
}
