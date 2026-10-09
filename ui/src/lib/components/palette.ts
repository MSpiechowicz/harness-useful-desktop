import { fuzzyMatch } from '$lib/utils/fuzzy';

import type { IconComponent } from './icon';

/** What a palette entry is; prefix modes narrow the list to some kinds. */
export type PaletteKind = 'nav' | 'action' | 'session' | 'file' | 'config' | 'extension' | 'slash';

export interface PaletteCommand {
  id: string;
  label: string;
  group: string;
  /** Defaults to `action`. */
  kind?: PaletteKind;
  icon?: IconComponent;
  /** Muted text after the label (a path, a date); also searchable. */
  detail?: string;
  /** Extra searchable text that is not displayed. */
  keywords?: string;
  /** Combo shown as key caps, e.g. `mod+1`. */
  shortcut?: string;
  disabled?: boolean;
  /** What Shift+Enter does instead, e.g. resume a session in chat. */
  secondary?: { label: string; run: () => void };
  run: () => void;
}

export interface PaletteEntry {
  command: PaletteCommand;
  /** Matched character positions within `command.label`, for highlighting. */
  labelIndices: number[];
  score: number;
}

export interface PaletteGroup {
  name: string;
  entries: PaletteEntry[];
  /** Matches left out of this group because the unfiltered list caps each group. */
  hidden: number;
}

export type PaletteMode = 'all' | 'action' | 'slash' | 'session' | 'file';

export const RECENT_GROUP = 'Recent';

/** Typing one of these first narrows the palette; shown in its footer. */
export const MODE_PREFIXES: readonly { prefix: string; mode: Exclude<PaletteMode, 'all'>; label: string; noun: string }[] = [
  { prefix: '>', mode: 'action', label: 'Actions', noun: 'actions' },
  { prefix: '/', mode: 'slash', label: 'Slash commands', noun: 'slash commands' },
  { prefix: '@', mode: 'session', label: 'Sessions', noun: 'sessions' },
  { prefix: '#', mode: 'file', label: 'Files', noun: 'changed files' },
];

const MODE_KINDS: Record<PaletteMode, PaletteKind[] | null> = {
  all: null,
  action: ['nav', 'action'],
  slash: ['slash'],
  session: ['session'],
  file: ['file'],
};

/** Kinds worth showing before the user has typed anything. */
const SUGGESTED_KINDS: PaletteKind[] = ['nav', 'action'];

const GROUP_LIMIT = 5;
const SUGGESTED_LIMIT = 8;
const MODE_LIMIT = 100;

export interface ParsedQuery {
  mode: PaletteMode;
  /** The query without its prefix. */
  text: string;
}

export function parseQuery(raw: string): ParsedQuery {
  const trimmed = raw.trimStart();
  const found = MODE_PREFIXES.find((entry) => trimmed.startsWith(entry.prefix));

  return found ? { mode: found.mode, text: trimmed.slice(1) } : { mode: 'all', text: trimmed };
}

/** Entries the user types abbreviations for; the rest (content of the project) match by whole words. */
const FUZZY_KINDS: PaletteKind[] = ['nav', 'action', 'slash'];

function matchFuzzy(query: string, command: PaletteCommand): PaletteEntry | null {
  const inLabel = fuzzyMatch(query, command.label);
  if (inLabel) {
    // Label hits outrank keyword-only hits.
    return { command, labelIndices: inLabel.indices, score: inLabel.score + 1000 };
  }

  // Descriptions and keywords are long; abbreviations would match them by accident, so only literal hits count.
  const hidden = [command.detail, command.keywords].filter(Boolean).join(' ').toLowerCase();
  const at = hidden.indexOf(query.trim().toLowerCase());

  return at === -1 ? null : { command, labelIndices: [], score: 100 - at };
}

/**
 * Sessions, files, configuration keys and extensions are long, so scattered letters would match
 * almost anything. Every word of the query must appear literally in the label or the detail instead.
 */
function matchWords(query: string, command: PaletteCommand): PaletteEntry | null {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const label = command.label.toLowerCase();
  const haystack = [label, command.detail, command.keywords].filter(Boolean).join(' ').toLowerCase();

  if (!words.every((word) => haystack.includes(word))) {
    return null;
  }

  const indices = words.flatMap((word) => {
    const start = label.indexOf(word);

    return start === -1 ? [] : Array.from({ length: word.length }, (_, offset) => start + offset);
  });
  const inLabel = words.some((word) => label.includes(word));

  return { command, labelIndices: [...new Set(indices)].sort((x, y) => x - y), score: (inLabel ? 1000 : 0) - label.length };
}

function matchCommand(query: string, command: PaletteCommand): PaletteEntry | null {
  if (query.trim() === '') {
    return { command, labelIndices: [], score: 0 };
  }

  return FUZZY_KINDS.includes(command.kind ?? 'action') ? matchFuzzy(query, command) : matchWords(query, command);
}

function groupEntries(entries: PaletteEntry[], limit: number): PaletteGroup[] {
  const groups = new Map<string, PaletteEntry[]>();
  for (const entry of entries) {
    const existing = groups.get(entry.command.group);

    if (existing) {
      existing.push(entry);
    } else {
      groups.set(entry.command.group, [entry]);
    }
  }

  return [...groups].map(([name, grouped]) => ({ name, entries: grouped.slice(0, limit), hidden: Math.max(0, grouped.length - limit) }));
}

export interface SearchOptions {
  /** Ids of recently run entries, newest first; shown above the rest when nothing is typed. */
  recents?: readonly string[];
}

export interface PaletteResults {
  mode: PaletteMode;
  text: string;
  groups: PaletteGroup[];
}

/**
 * Filters and groups palette entries.
 *  - Nothing typed: recents, then navigation and actions in their given order.
 *  - Text typed: every kind, fuzzy-ranked; each group is capped, and groups rank by their best hit.
 *  - A prefix (`>` `/` `@` `#`): only that kind, uncapped up to a sane maximum.
 */
export function searchPalette(commands: PaletteCommand[], raw: string, { recents = [] }: SearchOptions = {}): PaletteResults {
  const { mode, text } = parseQuery(raw);
  const allowed = MODE_KINDS[mode];
  const searching = text.trim() !== '';
  const pool = commands.filter((command) => (allowed ? allowed.includes(command.kind ?? 'action') : searching || SUGGESTED_KINDS.includes(command.kind ?? 'action')));

  const entries = pool.map((command) => matchCommand(text, command)).filter((entry): entry is PaletteEntry => entry !== null);

  if (searching) {
    entries.sort((a, b) => b.score - a.score);
  }

  const limit = mode === 'all' ? GROUP_LIMIT : MODE_LIMIT;
  const groups = groupEntries(entries, mode === 'all' && !searching ? SUGGESTED_LIMIT : limit);

  if (searching || mode !== 'all') {
    return { mode, text, groups };
  }

  return { mode, text, groups: withRecents(groups, recents) };
}

/** Pulls recently run entries out of their groups into a leading "Recent" group. */
function withRecents(groups: PaletteGroup[], recents: readonly string[]): PaletteGroup[] {
  const byId = new Map(groups.flatMap((group) => group.entries).map((entry) => [entry.command.id, entry]));
  const recent = recents.map((id) => byId.get(id)).filter((entry): entry is PaletteEntry => entry !== undefined && !entry.command.disabled);

  if (recent.length === 0) {
    return groups;
  }

  const taken = new Set(recent.map((entry) => entry.command.id));
  const rest = groups
    .map((group) => ({ ...group, entries: group.entries.filter((entry) => !taken.has(entry.command.id)) }))
    .filter((group) => group.entries.length > 0);

  return [{ name: RECENT_GROUP, entries: recent, hidden: 0 }, ...rest];
}
