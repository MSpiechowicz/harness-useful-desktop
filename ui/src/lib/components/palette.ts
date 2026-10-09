import { fuzzyMatch } from '$lib/utils/fuzzy';

import type { IconComponent } from './icon';

export interface PaletteCommand {
  id: string;
  label: string;
  group: string;
  icon?: IconComponent;
  /** Extra searchable text that is not displayed. */
  keywords?: string;
  /** Combo shown as key caps, e.g. `mod+1`. */
  shortcut?: string;
  disabled?: boolean;
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
}

function matchCommand(query: string, command: PaletteCommand): PaletteEntry | null {
  const inLabel = fuzzyMatch(query, command.label);
  if (inLabel) {
    // Label hits outrank keyword-only hits.
    return { command, labelIndices: inLabel.indices, score: inLabel.score + 1000 };
  }

  const inKeywords = command.keywords ? fuzzyMatch(query, command.keywords) : null;
  if (inKeywords) {
    return { command, labelIndices: [], score: inKeywords.score };
  }

  return null;
}

/**
 * Filters commands by fuzzy query and groups them. Without a query the given
 * order is kept; with one, entries sort by score and groups by their best entry.
 */
export function searchCommands(commands: PaletteCommand[], query: string): PaletteGroup[] {
  const searching = query.trim() !== '';
  const entries = commands
    .map((command) => matchCommand(query, command))
    .filter((entry): entry is PaletteEntry => entry !== null);

  if (searching) {
    entries.sort((a, b) => b.score - a.score);
  }

  const groups = new Map<string, PaletteEntry[]>();
  for (const entry of entries) {
    const existing = groups.get(entry.command.group);

    if (existing) {
      existing.push(entry);
    } else {
      groups.set(entry.command.group, [entry]);
    }
  }

  return [...groups].map(([name, grouped]) => ({ name, entries: grouped }));
}
