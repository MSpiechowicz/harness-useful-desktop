import { displayText } from '$lib/utils/display-text';

import { harnessToolName } from './harness-tools';
import type { ToolCallState } from './types';

/** A tool of Harness Code the Status report card reads. */
export type CodeLookup = 'map' | 'symbol' | 'impact' | 'path';

export interface CodeFile {
  path: string;
  /** Which lookups named it, in the order they ran. */
  kinds: CodeLookup[];
}

export interface CodeGraphUse {
  /** The lookups that named at least one file. */
  lookups: number;
  files: CodeFile[];
}

/** The most files one reply lists: a bound on what a hostile reply makes the card draw. */
const MAX_FILES = 200;
const MAX_PATH_CHARS = 400;

const TOOLS: Record<string, CodeLookup> = { code_map: 'map', code_symbol: 'symbol', code_impact: 'impact', code_path: 'path' };

const LOCATED = /^\s*\S.*?\s{2,}([^\s()]+?)(?::\d+(?:-\d+)?)?$/;
const CALL_SITE = /^\s+at\s/;
const TESTS = /^\s+tests: (.*)$/;
const TESTED = /^(\S+) \(\d+\)$/;
const IMPACTED = /^\s+d\d+ (\S+)(?:\s{2}\[.*)?$/;
const SITE = /^\s{4}(\S+?):\d+/;

/** A word of a reply that names a file, not a symbol or a sentence. */
function isPath(word: string): boolean {
  return word.includes('/') || word.includes('.');
}

function testedFiles(list: string): string[] {
  return list.split(', ').flatMap((part) => TESTED.exec(part)?.[1] ?? []);
}

/** `code_map`: a file on a line of its own, its definitions indented under it. */
function mapFiles(lines: string[]): string[] {
  return lines.filter((line) => /^\S+$/.test(line));
}

/** `code_impact`: `  d1 path  [caller → callee]` for each file, then the tests. */
function impactFiles(lines: string[]): string[] {
  return lines.flatMap((line) => {
    const tests = TESTS.exec(line);

    return tests ? testedFiles(tests[1]) : (IMPACTED.exec(line)?.[1] ?? []);
  });
}

/** `code_path`: `  → name  path:line` for each step. The `at path:line` lines under a step are call sites, not steps. */
function pathFiles(lines: string[]): string[] {
  return lines.flatMap((line) => (/^\s/.test(line) && !CALL_SITE.test(line) ? (LOCATED.exec(line)?.[1] ?? []) : []));
}

/** `code_symbol`: where the name is defined and the files under `used by`. What it `uses` is what it depends on, not who needs it. */
function symbolFiles(lines: string[]): string[] {
  const files: string[] = [];
  let usedBy = false;

  for (const line of lines) {
    if (/^\S/.test(line)) {
      usedBy = false;

      const defined = LOCATED.exec(line)?.[1];

      if (defined) {
        files.push(defined);
      }
    } else if (/^\s+uses:/.test(line)) {
      usedBy = false;
    } else if (/^\s+used by /.test(line)) {
      usedBy = true;
    } else if (usedBy) {
      const tests = TESTS.exec(line);

      files.push(...(tests ? testedFiles(tests[1]) : (SITE.exec(line)?.slice(1, 2) ?? [])));
    }
  }

  return files;
}

const READERS: Record<CodeLookup, (lines: string[]) => string[]> = { map: mapFiles, symbol: symbolFiles, impact: impactFiles, path: pathFiles };

/** The files one Harness Code reply names. Empty for a reply in another shape, so the card falls back on what the agent wrote. */
export function codeReplyFiles(kind: CodeLookup, text: string | null): string[] {
  const files = READERS[kind]((text ?? '').split('\n'))
    .filter(isPath)
    .map((path) => displayText(path, MAX_PATH_CHARS));

  return [...new Set(files)];
}

/** What the Harness Code calls of a reply found, in the order they ran. A failed call counts for nothing. */
export function readCodeGraph(calls: ToolCallState[]): CodeGraphUse {
  const files = new Map<string, CodeFile>();
  let lookups = 0;

  for (const call of calls) {
    const kind = TOOLS[harnessToolName('code', call.name) ?? ''];

    if (!kind || call.status !== 'ok') {
      continue;
    }

    const named = codeReplyFiles(kind, call.resultText);

    if (named.length > 0) {
      lookups += 1;
    }

    for (const path of named) {
      const known = files.get(path);

      if (known) {
        known.kinds = known.kinds.includes(kind) ? known.kinds : [...known.kinds, kind];
      } else if (files.size < MAX_FILES) {
        files.set(path, { path, kinds: [kind] });
      }
    }
  }

  return { lookups, files: [...files.values()] };
}
