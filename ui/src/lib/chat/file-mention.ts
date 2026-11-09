import type { ProjectFile } from '$lib/ipc/bindings/ProjectFile';

import { isBashDraft } from './composer';

/** An `@path` token in the draft: `start..end` is the whole token with its `@`, `query` what was typed after the `@` up to the caret. */
export interface FileMention {
  start: number;
  end: number;
  query: string;
}

/** What an `@` may follow besides the start of the draft and whitespace. */
const OPENERS = '([{"\'';

/** What always ends the token after the caret besides whitespace, so picking inside `@foo,` leaves the punctuation alone. */
const STOPS = '"\',;`';

/** The opener each closing bracket belongs to. A closer ends the token only when the token holds no opener for it. */
const PAIRS: Record<string, string> = { ')': '(', ']': '[', '}': '{' };

const FENCE = /^ {0,3}(`{3,}|~{3,})/;

function isSpace(char: string): boolean {
  return /\s/.test(char);
}

/** True for a character that ends a token no matter what came before it: whitespace, a quote, a comma, a semicolon or a backtick. */
function stopsMention(char: string): boolean {
  return isSpace(char) || STOPS.includes(char);
}

/** True for a character that ends the text after a picked path, so the path is not glued to it. */
function endsMention(char: string): boolean {
  return stopsMention(char) || char in PAIRS;
}

/**
 * Where the token that holds `start..caret` ends. A closing bracket belongs to the token when the token holds its opener, so
 * `@src/ro|utes/[id]/page` is one path and `(@sr|c)` keeps its outer `)`.
 */
function mentionEnd(draft: string, start: number, caret: number): number {
  const open: Record<string, number> = { '(': 0, '[': 0, '{': 0 };

  for (let index = start; index < caret; index += 1) {
    const char = draft[index];

    if (char in open) {
      open[char] += 1;
    } else if (char in PAIRS && open[PAIRS[char]] > 0) {
      open[PAIRS[char]] -= 1;
    }
  }

  let end = caret;

  while (end < draft.length && !stopsMention(draft[end])) {
    const char = draft[end];

    if (char in open) {
      open[char] += 1;
    } else if (char in PAIRS) {
      if (open[PAIRS[char]] === 0) {
        break;
      }

      open[PAIRS[char]] -= 1;
    }

    end += 1;
  }

  return end;
}

/** True when `at` sits after the start of the draft, whitespace, or an opening bracket or quote, so `a@b.com` is left alone. */
function startsMention(draft: string, at: number): boolean {
  return at === 0 || isSpace(draft[at - 1]) || OPENERS.includes(draft[at - 1]);
}

/** True when `at` is inside a fenced code block that opens on an earlier line. */
function insideFence(draft: string, at: number): boolean {
  const lineStart = draft.lastIndexOf('\n', at - 1) + 1;
  let open: string | null = null;

  for (const line of draft.slice(0, lineStart).split('\n')) {
    const marker = FENCE.exec(line)?.[1];
    if (!marker) {
      continue;
    }

    if (open === null) {
      open = marker;
    } else if (marker[0] === open[0] && marker.length >= open.length) {
      open = null;
    }
  }

  return open !== null;
}

/** True when `at` follows an odd number of backticks on its own line, that is inside an inline code span. */
function insideCodeSpan(draft: string, at: number): boolean {
  const lineStart = draft.lastIndexOf('\n', at - 1) + 1;
  const ticks = draft.slice(lineStart, at).split('`').length - 1;

  return ticks % 2 === 1;
}

/**
 * The file mention the caret is in, or null. The token is the `@` and the non-space characters before the caret, and after it up to
 * whitespace, a quote, comma, semicolon or backtick, or a closing bracket that closes nothing in the token, so a caret in the middle of a path replaces the rest of it. Nothing opens for a selection, a shell command (`!`), or code.
 */
export function fileMentionQuery(draft: string, caret: number, selectionEnd: number = caret): FileMention | null {
  if (caret !== selectionEnd || caret < 0 || caret > draft.length || isBashDraft(draft)) {
    return null;
  }

  let runStart = caret;
  while (runStart > 0 && !isSpace(draft[runStart - 1])) {
    runStart -= 1;
  }

  // The first valid `@` of the run wins, so `@node_modules/@types` stays one mention.
  let start = -1;
  for (let index = runStart; index < caret; index += 1) {
    if (draft[index] === '@' && startsMention(draft, index)) {
      start = index;
      break;
    }
  }

  if (start === -1 || insideCodeSpan(draft, start) || insideFence(draft, start)) {
    return null;
  }

  return { start, end: mentionEnd(draft, start, caret), query: draft.slice(start + 1, caret) };
}

/** The draft with the mention replaced by the plain path (a folder keeps a trailing `/`), and where the caret goes. */
export function applyFileMention(draft: string, mention: FileMention, file: Pick<ProjectFile, 'path' | 'isDir'>): { text: string; caret: number } {
  // A name is the repository's to choose: one starting with `!` must not turn the draft into a shell command, and one starting with
  // `@` must not read as a mention to a harness that expands a leading `@`.
  const name = file.path.startsWith('!') || file.path.startsWith('@') ? `./${file.path}` : file.path;
  const path = file.isDir ? `${name}/` : name;
  const after = draft.slice(mention.end);
  const gap = after !== '' && endsMention(after[0]) ? '' : ' ';

  return {
    text: `${draft.slice(0, mention.start)}${path}${gap}${after}`,
    caret: mention.start + path.length + gap.length,
  };
}
