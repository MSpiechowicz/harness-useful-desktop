import type { FoundNote, FoundScript } from './memory-tools';
import { idsOf, memoryToolName, parseOpenedNotes, parseSearch, ratedNotes, readNotes, textArg } from './memory-tools';
import { isJsonObject } from './tools';
import type { ChatState, ToolCallState } from './types';

/** How the rest of the reply used a row a search found. `useful` is the agent's own word, `opened` and `ran` are what it did. */
export type RowUse = { kind: 'useful'; reason: string | null } | { kind: 'opened' } | { kind: 'ran' };

/** What the later calls of the same reply say about the rows of one `note_search` call. */
export interface SearchUse {
  /** The reply that made the search is over: another message of the user follows it, or the chat is not streaming. */
  ended: boolean;
  /** Note ids the agent rated useful (`note_feedback`), with its reason when it gave one. */
  rated: Map<number, string | null>;
  /** Note ids it read (`note_get`). */
  read: Set<number>;
  /** Script names and ids it ran (`script_run`). */
  ranNames: Set<string>;
  ranIds: Set<number>;
}

/** The use of every `note_search` call of a chat, by call id. */
export interface MemoryUseIndex extends ReadonlyMap<string, SearchUse> {
  /** The notes each `note_feedback` call rated that the same reply found (`note_search`) or read (`note_get`) before it, by call id and note id. A note not in it is one nothing is known of. */
  readonly rated?: ReadonlyMap<string, ReadonlyMap<number, FoundNote>>;
}

/** The index as `buildMemoryUse` fills it. */
type Built = Map<string, SearchUse> & { rated: Map<string, Map<number, FoundNote>> };

/** What the reply so far has shown of its notes, and the searches that later calls are held against. */
interface Reply {
  searches: SearchUse[];
  known: Map<number, FoundNote>;
}

type Chat = Pick<ChatState, 'items' | 'toolCalls' | 'phase'>;

function blank(): SearchUse {
  return { ended: false, rated: new Map(), read: new Set(), ranNames: new Set(), ranIds: new Set() };
}

/**
 * Reads what a reply did with the notes and scripts its searches found. A reply starts at a message of the user and takes
 * every tool call of the assistant messages and loose tool cards after it, up to the next message of the user, in the order
 * of `items` (the order of the screen, history included: never sorted by the call ids). Only calls after a search count for
 * it, only in the same reply, and a call that failed counts for nothing.
 *
 * Every message of the user ends the reply, a steering message the agent took mid-turn too: the items keep no mark of a turn
 * (the reducer drops `turnStart`, `turnEnd` and `agentEnd` from the list, and a steer is told from a new prompt, or from a
 * follow-up, by nothing on the item), so there is no telling them apart here. The calls after a steer are then held against
 * the searches after it only. A transcript read in pages may begin with the tail of a reply: its searches are read as they
 * stand, with no use of the calls on the page before.
 *
 * `previous` is the index of the last build. A search whose use did not change keeps its object from there (and the whole
 * index is `previous` when nothing changed), so the cards of the unchanged searches do not render again on every delta.
 */
export function buildMemoryUse(chat: Chat, previous?: MemoryUseIndex): MemoryUseIndex {
  const rated = new Map<string, Map<number, FoundNote>>();
  const index = Object.assign(new Map<string, SearchUse>(), { rated });

  if (!Object.values(chat.toolCalls).some((call) => ['note_search', 'note_feedback'].includes(memoryToolName(call.name) ?? ''))) {
    return index;
  }

  let reply: Reply = { searches: [], known: new Map() };

  for (const item of chat.items) {
    if (item.kind === 'message' && item.role === 'user') {
      // The reply before this message is over, whatever the chat does now.
      for (const search of reply.searches) {
        search.ended = true;
      }

      reply = { searches: [], known: new Map() };
      continue;
    }

    const ids = item.kind === 'tool' ? [item.toolCallId] : item.kind === 'message' && item.role === 'assistant' ? item.blocks.flatMap((block) => (block.type === 'tool' ? [block.toolCallId] : [])) : [];

    for (const id of ids) {
      const call = chat.toolCalls[id];
      if (call) {
        absorb(call, reply, index, rated);
      }
    }
  }

  const over = chat.phase !== 'streaming';
  for (const search of reply.searches) {
    search.ended = over;
  }

  return previous ? reuse(index, previous) : index;
}

/** `next` with the object of `previous` for every search and rating whose use is the same; `previous` itself when all of them are. */
function reuse(next: Built, previous: MemoryUseIndex): MemoryUseIndex {
  const ratings = next.rated;
  let changed = next.size !== previous.size || ratings.size !== (previous.rated?.size ?? 0);

  for (const [id, use] of next) {
    const before = previous.get(id);

    if (before && sameUse(before, use)) {
      next.set(id, before);
    } else {
      changed = true;
    }
  }

  for (const [id, notes] of ratings) {
    const before = previous.rated?.get(id);

    if (before && sameNotes(before, notes)) {
      ratings.set(id, before as Map<number, FoundNote>);
    } else {
      changed = true;
    }
  }

  return changed ? next : previous;
}

function sameNotes(a: ReadonlyMap<number, FoundNote>, b: ReadonlyMap<number, FoundNote>): boolean {
  return a.size === b.size && [...b].every(([id, note]) => {
    const other = a.get(id);

    return other !== undefined && other.title === note.title && other.kind === note.kind && other.scope === note.scope && other.date === note.date;
  });
}

function sameUse(a: SearchUse, b: SearchUse): boolean {
  return (
    a.ended === b.ended &&
    a.rated.size === b.rated.size &&
    [...b.rated].every(([id, reason]) => a.rated.has(id) && a.rated.get(id) === reason) &&
    sameSet(a.read, b.read) &&
    sameSet(a.ranNames, b.ranNames) &&
    sameSet(a.ranIds, b.ranIds)
  );
}

function sameSet<T>(a: ReadonlySet<T>, b: ReadonlySet<T>): boolean {
  return a.size === b.size && [...b].every((value) => a.has(value));
}

/**
 * A search starts a record and its notes become known to the reply. Any other call of the reply is held against the searches
 * before it; a `note_get` also makes its notes known, and a `note_feedback` takes the titles of the notes it rated from them.
 */
function absorb(call: ToolCallState, reply: Reply, index: Map<string, SearchUse>, rated: Map<string, Map<number, FoundNote>>): void {
  const tool = memoryToolName(call.name);
  const { searches, known } = reply;

  if (tool === 'note_search') {
    if (!index.has(call.id)) {
      const use = blank();
      index.set(call.id, use);
      searches.push(use);
    }

    if (call.status === 'ok') {
      parseSearch(call.resultText).notes.forEach((note) => known.set(note.id, note));
    }

    return;
  }

  if (call.status !== 'ok') {
    return;
  }

  if (tool === 'note_get') {
    parseOpenedNotes(call.resultText).forEach((note) => known.set(note.id, note));
  }

  if (tool === 'note_feedback') {
    rated.set(call.id, new Map(ratedNotes(call.args, call.resultText).flatMap((id): [number, FoundNote][] => {
      const note = known.get(id);

      return note ? [[id, note]] : [];
    })));
  }

  for (const search of searches) {
    if (tool === 'note_feedback') {
      rate(search, call);
    } else if (tool === 'note_get') {
      readNotes(call.args, call.resultText).forEach((id) => search.read.add(id));
    } else if (tool === 'script_run') {
      ran(search, call);
    }
  }
}

function rate(search: SearchUse, call: ToolCallState): void {
  const reason = textArg(call.args, 'reason');

  for (const id of ratedNotes(call.args, call.resultText)) {
    // The first word of the agent stands, unless a later one adds the reason the first left out.
    if (!search.rated.has(id) || (search.rated.get(id) === null && reason !== null)) {
      search.rated.set(id, reason);
    }
  }
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

/**
 * The one reason every row of a list shares: the rows are at least two, all marked useful, and all with the same reason. It is
 * then said once for the list. Null when the rows differ (each says its own) or there is nothing to share.
 */
export function sharedReason(uses: readonly (RowUse | null)[]): string | null {
  const reasons = uses.map((use) => (use?.kind === 'useful' ? use.reason : null));
  const first = reasons[0] ?? null;

  return reasons.length > 1 && first !== null && reasons.every((reason) => reason === first) ? first : null;
}

/** What the reply did with note `id`: rated it useful, else read it. Null when nothing is known of it. */
export function noteUse(use: SearchUse | undefined, id: number): RowUse | null {
  if (!use) {
    return null;
  }

  if (use.rated.has(id)) {
    return { kind: 'useful', reason: use.rated.get(id) ?? null };
  }

  return use.read.has(id) ? { kind: 'opened' } : null;
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
