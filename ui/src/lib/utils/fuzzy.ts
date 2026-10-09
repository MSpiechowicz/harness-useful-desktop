export interface FuzzyMatch {
  score: number;
  /** Indices in the haystack that matched, ascending. */
  indices: number[];
}

const WORD_BOUNDARY = /[\s\-_/.:]/;

function isWordStart(text: string, index: number): boolean {
  return index === 0 || WORD_BOUNDARY.test(text[index - 1]);
}

function contiguousMatch(needle: string, haystack: string): FuzzyMatch | null {
  const start = haystack.indexOf(needle);
  if (start === -1) {
    return null;
  }

  const indices = Array.from({ length: needle.length }, (_, offset) => start + offset);
  const boundaryBonus = isWordStart(haystack, start) ? 40 : 0;
  const prefixBonus = start === 0 ? 20 : 0;

  return { score: 100 + boundaryBonus + prefixBonus - start, indices };
}

function subsequenceMatch(needle: string, haystack: string): FuzzyMatch | null {
  const indices: number[] = [];
  let score = 0;
  let cursor = 0;

  for (const char of needle) {
    const found = haystack.indexOf(char, cursor);
    if (found === -1) {
      return null;
    }

    const previous = indices[indices.length - 1];
    if (previous !== undefined && found === previous + 1) {
      score += 8;
    }
    if (isWordStart(haystack, found)) {
      score += 6;
    }

    score += 1 - (found - cursor) * 0.1;
    indices.push(found);
    cursor = found + 1;
  }

  return { score, indices };
}

/**
 * Case-insensitive fuzzy match. A contiguous substring always beats a scattered
 * subsequence; an empty query matches everything with score 0.
 */
export function fuzzyMatch(query: string, text: string): FuzzyMatch | null {
  const needle = query.trim().toLowerCase();
  if (needle === '') {
    return { score: 0, indices: [] };
  }

  const haystack = text.toLowerCase();

  return contiguousMatch(needle, haystack) ?? subsequenceMatch(needle, haystack);
}

export interface TextSegment {
  text: string;
  match: boolean;
}

/** Splits text into alternating plain/matched runs so it can be rendered without `{@html}`. */
export function highlightSegments(text: string, indices: number[]): TextSegment[] {
  const matched = new Set(indices);
  const segments: TextSegment[] = [];

  for (let index = 0; index < text.length; index += 1) {
    const match = matched.has(index);
    const last = segments[segments.length - 1];

    if (last && last.match === match) {
      last.text += text[index];
    } else {
      segments.push({ text: text[index], match });
    }
  }

  return segments;
}
