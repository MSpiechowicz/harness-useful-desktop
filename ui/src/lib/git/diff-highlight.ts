import type { FileDiff } from '$lib/ipc/bindings/FileDiff';
import { highlight, type HighlightedLine } from '$lib/markdown/highlight';
import { grammarFor } from '$lib/markdown/languages';

/** Diffs longer than this stay plain; tokenizing them would delay the first paint for little gain. */
const MAX_HIGHLIGHTED_LINES = 4000;

/** Fence label (the file extension) for a path, or '' when it has none. */
export function languageOf(path: string): string {
  const name = path.split('/').at(-1) ?? path;
  const dot = name.lastIndexOf('.');

  return dot > 0 ? name.slice(dot + 1) : '';
}

/**
 * Syntax colors for the lines of a diff, keyed by flat line index. Each hunk is highlighted as the
 * old text and the new text it stands for, so tokens keep their surrounding context; context lines
 * take the new side. Returns an empty map for unknown languages and oversized diffs.
 */
export async function highlightDiff(diff: FileDiff): Promise<Map<number, HighlightedLine>> {
  const lang = languageOf(diff.path);
  const result = new Map<number, HighlightedLine>();
  const total = diff.hunks.reduce((sum, hunk) => sum + hunk.lines.length, 0);

  if (grammarFor(lang) === null || total > MAX_HIGHLIGHTED_LINES) {
    return result;
  }

  let index = 0;

  for (const hunk of diff.hunks) {
    const oldSide: { index: number; text: string }[] = [];
    const newSide: { index: number; text: string }[] = [];

    for (const line of hunk.lines) {
      if (line.kind === 'remove') {
        oldSide.push({ index, text: line.text });
      } else if (line.kind === 'add' || line.kind === 'context') {
        newSide.push({ index, text: line.text });
      }
      index += 1;
    }

    for (const side of [oldSide, newSide]) {
      const tokens = await highlight(side.map((entry) => entry.text).join('\n'), lang);
      tokens?.forEach((line, position) => {
        const entry = side[position];
        if (entry) {
          result.set(entry.index, line);
        }
      });
    }
  }

  return result;
}
