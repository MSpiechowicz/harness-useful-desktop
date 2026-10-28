import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';
import type { NoteKind } from '$lib/ipc/bindings/NoteKind';

import { isJsonObject } from './tools';

/**
 * The harness-memory tool a call name is, whatever the harness calls it: Claude Code and Codex `mcp__harness-memory__note_search`,
 * OpenCode `harness-memory_note_search`, OMP `mcp__harness_memory_note_search`, a Claude plugin install
 * `mcp__plugin_<plugin>_harness-memory__note_search`. The server segment is anchored: after an optional `mcp__` and an optional
 * `plugin_<plugin>_` (a kebab-case plugin name) it is exactly `harness-memory` (or `harness_memory`), then the tool. Another server's `memory_search`, a bare
 * legacy `memory_save` and a look-alike server such as `mcp__evil_harness_memory__note_search` are not one. Null for those.
 */
export function memoryToolName(name: string): string | null {
  return /^(?:mcp__)?(?:plugin_[A-Za-z0-9-]+_)?harness[-_]memory(?:__|_)([a-z][a-z0-9_]*)$/.exec(name)?.[1] ?? null;
}

const NOTE_KINDS: readonly string[] = ['outcome', 'problem', 'fix', 'optimisation', 'location', 'decision', 'note'];

export interface FoundNote {
  type: 'note';
  id: number;
  kind: NoteKind;
  scope: 'repo' | 'global';
  /** `YYYY-MM-DD`, the day the note was last written. */
  date: string;
  title: string;
  /** The folder the note was written from (`from "…"` in its header), as the server shows it; null when it names none. */
  path: string | null;
}

export interface FoundScript {
  type: 'script';
  /** Set when the reply names the script by id (another script of the same name hides it from a name lookup). */
  id: number | null;
  name: string;
  description: string;
  language: string;
}

export type FoundRow = FoundNote | FoundScript;

export interface SearchResult {
  notes: FoundNote[];
  scripts: FoundScript[];
}

// `#<id> [<kind>, <scope>, <date>, from "…" via "…"] <title> {entities}`. The bracket holds no `]` (quoted fields have them
// replaced on the server), so the first `]` closes it, and the entities are an optional ` {a, b}` after the title.
const HIT = /^#(\d+) \[([a-z]+), (repo|global), (\d{4}-\d{2}-\d{2})([^\]]*)\] (.*?)(?: \{.*\})?$/;
/** The `from "<folder>"` part of a header's bracket. */
const FROM = /(?:^|, )from "([^"]*)"/;
const SCRIPT = /^- (.+) \(([a-z0-9_+-]+)\) → script_run (name|id)=(\S+)$/;
const SCRIPT_NAME = /^([a-z0-9][a-z0-9_-]*)(?:: (.*))?$/;
const SCRIPTS_OPEN = '<saved-scripts>';
const SCRIPTS_CLOSE = '</saved-scripts>';

/**
 * The notes and saved scripts a `note_search` reply lists. Note headers start their line (snippets are indented), saved
 * scripts are the lines of the `<saved-scripts>` block; the frame, a "(repository unknown …)" line, "no matching notes", a
 * "… (n more)" trailer and the saving notice are not rows. Anything unreadable is left out, never guessed.
 */
export function parseSearch(text: string | null): SearchResult {
  const result: SearchResult = { notes: [], scripts: [] };
  let inScripts = false;

  for (const line of (text ?? '').split('\n')) {
    if (line === SCRIPTS_OPEN) {
      inScripts = true;
    } else if (line === SCRIPTS_CLOSE) {
      inScripts = false;
    } else if (inScripts) {
      const script = parseScript(line);
      if (script) {
        result.scripts.push(script);
      }
    } else {
      const note = parseHit(line);
      if (note) {
        result.notes.push(note);
      }
    }
  }

  return result;
}

function parseHit(line: string): FoundNote | null {
  const match = HIT.exec(line);
  if (!match) {
    return null;
  }

  const id = Number(match[1]);
  if (!Number.isSafeInteger(id)) {
    return null;
  }

  const kind = NOTE_KINDS.includes(match[2]) ? (match[2] as NoteKind) : 'note';

  return { type: 'note', id, kind, scope: match[3] as 'repo' | 'global', date: match[4], title: match[6], path: pathOf(match[5]) };
}

/** The folder a header's bracket says the note was written from, when it says one. */
function pathOf(rest: string | undefined): string | null {
  return FROM.exec(rest ?? '')?.[1] || null;
}

function parseScript(line: string): FoundScript | null {
  const match = SCRIPT.exec(line);
  const head = match ? SCRIPT_NAME.exec(match[1]) : null;
  if (!match || !head) {
    return null;
  }

  const byId = match[3] === 'id';
  const id = byId ? Number(match[4]) : null;
  if (byId && !Number.isSafeInteger(id)) {
    return null;
  }

  return { type: 'script', id, name: head[1], description: head[2] ?? '', language: match[2] };
}

/** The titles of the notes a `note_get` reply shows, in order: its headers are `#<id> [<kind>, <scope>, revision …] <title>`. */
export function parseNoteTitles(text: string | null): string[] {
  const titles: string[] = [];

  for (const line of (text ?? '').split('\n')) {
    const title = /^#\d+ \[[^\]]*\] (.*)$/.exec(line)?.[1];
    if (title !== undefined) {
      titles.push(title);
    }
  }

  return titles;
}

// `#<id> [<kind>, <scope> "<repo>", revision <n>, updated <date> (…), from "…" via "…"] <title>`: the header of a block of `note_get`.
const BLOCK = /^#(\d+) \[([a-z]+), (repo|global)(?: "[^"]*")?, revision \d+, updated (\d{4}-\d{2}-\d{2})([^\]]*)\] (.*)$/;

/** The notes a `note_get` reply shows, as a search lists them. A block whose header is not read in full is left out, never guessed. */
export function parseOpenedNotes(text: string | null): FoundNote[] {
  return (text ?? '').split('\n').flatMap((line): FoundNote[] => {
    const match = BLOCK.exec(line);
    const id = Number(match?.[1]);
    if (!match || !Number.isSafeInteger(id)) {
      return [];
    }

    const kind = NOTE_KINDS.includes(match[2]) ? (match[2] as NoteKind) : 'note';

    return [{ type: 'note', id, kind, scope: match[3] as 'repo' | 'global', date: match[4], title: match[6], path: pathOf(match[5]) }];
  });
}

/** A note an id names and nothing else is known of: a card shows `#<id>` and the way to open it. */
export interface UnknownNote {
  type: 'unknown';
  id: number;
}

const MARKED = /^marked (?:no notes|(\d+) notes?) useful(?:: (#\d+(?:, #\d+)*))?$/;
const NOTE_HEADER = /^#(\d+) \[/;

/**
 * The note ids a `note_feedback` reply says it marked useful: `marked 2 notes useful: #12, #40`, `marked 1 note useful: #12` or
 * `marked no notes useful`, then optional `not found: …` and `not checked: …` lines for ids that do not count. Null when the reply
 * is not that (an old server, an error, text cut short), so a caller falls back to what the call asked for.
 */
export function parseMarked(text: string | null): number[] | null {
  const first = (text ?? '').split('\n', 1)[0].trim();
  const match = MARKED.exec(first);
  if (!match) {
    return null;
  }

  if (match[2] === undefined) {
    return match[1] === undefined ? [] : null;
  }

  const ids = [...match[2].matchAll(/#(\d+)/g)].map((id) => Number(id[1]));
  if (match[1] === undefined || Number(match[1]) !== ids.length || !ids.every(Number.isSafeInteger)) {
    return null;
  }

  return [...new Set(ids)];
}

/**
 * The note ids a `note_get` reply shows: a block starts with `#<id> [`, and the ids it could not find are named on a
 * `not found:` line. Null when no block shows, so a caller falls back to what the call asked for.
 */
export function parseOpened(text: string | null): number[] | null {
  const ids = (text ?? '').split('\n').flatMap((line) => {
    const id = Number(NOTE_HEADER.exec(line)?.[1]);

    return Number.isSafeInteger(id) ? [id] : [];
  });

  return ids.length > 0 ? [...new Set(ids)] : null;
}

/** The notes a `note_feedback` call rated: the ones its reply confirms, else the ones it asked for. */
export function ratedNotes(args: JsonValue | null, resultText: string | null): number[] {
  return parseMarked(resultText) ?? [...new Set(idsOf(args))];
}

/** The notes a `note_get` call read: the ones its reply shows, else the ones it asked for. */
export function readNotes(args: JsonValue | null, resultText: string | null): number[] {
  return parseOpened(resultText) ?? [...new Set(idsOf(args))];
}

/** The ids a call names in `ids` (numbers, or digits as text) and the scalar `id`. */
export function idsOf(args: JsonValue | null): number[] {
  if (!isJsonObject(args)) {
    return [];
  }

  const named = [...(Array.isArray(args.ids) ? args.ids : []), args.id];

  return named.flatMap((value) => {
    const id = typeof value === 'string' && /^\d+$/.test(value) ? Number(value) : value;

    return typeof id === 'number' && Number.isSafeInteger(id) ? [id] : [];
  });
}

/** A text argument of a call, trimmed; null when it is missing or blank. */
export function textArg(args: JsonValue | null, name: string): string | null {
  const value = isJsonObject(args) ? args[name] : undefined;

  return typeof value === 'string' && value.trim() !== '' ? value.trim() : null;
}
