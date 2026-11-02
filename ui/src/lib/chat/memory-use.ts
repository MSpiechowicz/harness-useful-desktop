import type { FoundNote, FoundScript } from './memory-tools';
import { idsOf, memoryToolName, parseOpenedNotes, parseSearch, ratedNotes, readNotes, textArg } from './memory-tools';
import { isJsonObject } from './tools';
import type { ToolCallState } from './types';
import { type BaseUse, buildUseIndex, rowUse, type RowUse, sameSet, type UseAdapter, type UseChat, type UseIndex } from './use-index';

export { type RowUse, sharedReason } from './use-index';

/** What the later calls of the same reply say about the rows of one `note_search` call: the notes it rated or read, and the scripts it ran (`script_run`). */
export interface SearchUse extends BaseUse<number> {
  /** Script names and ids it ran. */
  ranNames: Set<string>;
  ranIds: Set<number>;
}

/** The use of every `note_search` call of a chat, by call id, and the notes each `note_feedback` call rated that the same reply found or read before it. */
export type MemoryUseIndex = UseIndex<number, FoundNote, SearchUse>;

const adapter: UseAdapter<number, FoundNote, SearchUse> = {
  tool: memoryToolName,
  names: { search: 'note_search', get: 'note_get', feedback: 'note_feedback' },
  blank: () => ({ ended: false, rated: new Map(), read: new Set(), ranNames: new Set(), ranIds: new Set() }),
  searched: (text) => parseSearch(text).notes,
  opened: parseOpenedNotes,
  rated: (call) => ratedNotes(call.args, call.resultText),
  read: (call) => readNotes(call.args, call.resultText),
  other: (tool, call, use) => {
    if (tool === 'script_run') {
      ran(use, call);
    }
  },
  sameExtra: (a, b) => sameSet(a.ranNames, b.ranNames) && sameSet(a.ranIds, b.ranIds),
  sameHit: (a, b) => a.title === b.title && a.kind === b.kind && a.scope === b.scope && a.date === b.date,
};

/** What a chat's replies did with the notes and scripts their searches found: see `buildUseIndex`. */
export function buildMemoryUse(chat: UseChat, previous?: MemoryUseIndex): MemoryUseIndex {
  return buildUseIndex(chat, adapter, previous);
}

function ran(search: SearchUse, call: ToolCallState): void {
  const name = textArg(call.args, 'name');
  if (name !== null) {
    search.ranNames.add(name);
  }

  // `script_run` takes one `id`; the ids a note call lists are not a script's.
  if (isJsonObject(call.args)) {
    idsOf({ id: call.args.id ?? null }).forEach((id) => search.ranIds.add(id));
  }
}

/** What the reply did with note `id`: rated it useful, else read it. Null when nothing is known of it. */
export function noteUse(use: SearchUse | undefined, id: number): RowUse | null {
  return rowUse(use, id);
}

/**
 * What the reply did with a script a search listed: ran it. A row listed with an id is a script another one of the same name
 * hides from a name lookup, so only a run by that id is its run; a row without an id is found by its name.
 */
export function scriptUse(use: SearchUse | undefined, script: Pick<FoundScript, 'id' | 'name'>): RowUse | null {
  if (!use) {
    return null;
  }

  const ranIt = script.id === null ? use.ranNames.has(script.name) : use.ranIds.has(script.id);

  return ranIt ? { kind: 'ran' } : null;
}
