import { expandWrapped } from './wrapped-calls';
import { reasonArg } from './harness-tools';
import { isSteer, recallCallId } from './recall-rows';
import type { ChatState, MessageItem, ToolCallState } from './types';

/** How the rest of the reply used a row a search found. `useful` is the agent's own word, `opened` and `ran` are what it did. */
export type RowUse = { kind: 'useful'; reason: string | null } | { kind: 'opened' } | { kind: 'ran' };

/** What the later calls of the same reply say about the rows of one search call, whatever the rows are (`K` is how one is named). */
export interface BaseUse<K> {
  /** The reply that made the search is over: another message of the user follows it, or the chat is not streaming. */
  ended: boolean;
  /** Rows the agent rated useful, with its reason when it gave one. */
  rated: Map<K, string | null>;
  /** Rows it opened. */
  read: Set<K>;
}

/** The use of every search call of a chat, by call id. */
export interface UseIndex<K, Hit, Use extends BaseUse<K>> extends ReadonlyMap<string, Use> {
  /** The rows each feedback call rated that the same reply found (search) or opened (get) before it, by call id and row id. A row not in it is one nothing is known of. */
  readonly rated?: ReadonlyMap<string, ReadonlyMap<K, Hit>>;
}

/** What a row of a card is shown with: what the reply did with it, and whether it reads quieter. */
export interface RowMarks {
  /** What the reply did with the row; null for a row nothing is known of (or that is not marked yet). */
  use: RowUse | null;
  /** A row the reply did not use: its title reads a step quieter. */
  dim?: boolean;
}

/** What one family of tools (memory notes, knowledge documents) tells the index about its calls and its replies. */
export interface UseAdapter<K, Hit extends { id: K }, Use extends BaseUse<K>> {
  /** The tool a call name is of this family (`note_search`), else null. */
  tool(name: string): string | null;
  /** The names of the three tools the index reads: the search, the open of one row and the rating. */
  names: { search: string; get: string; feedback: string };
  /** The family, as it names the search the app makes for a message (`recall:<message>:<family>`). */
  family: 'memory' | 'knowledge';
  blank(): Use;
  /** The rows the app found for a message of the user (the recall block of its hook context), as a search at the start of the reply; null when it found none. */
  recalled?(message: MessageItem): Hit[] | null;
  /** The rows a search reply lists, and the ones an open reply shows. */
  searched(text: string | null): Hit[];
  opened(text: string | null): Hit[];
  /** The rows a rating call rated (its reply, else its arguments) and an open call opened. */
  rated(call: ToolCallState): K[];
  read(call: ToolCallState): K[];
  /** What else a call of the reply says about a search before it (memory: a script was run). */
  other?(tool: string, call: ToolCallState, use: Use): void;
  /** Whether the two uses differ in what the adapter adds to the base. */
  sameExtra?(a: Use, b: Use): boolean;
  /** Whether a row is shown the same, so a card of it need not render again. */
  sameHit(a: Hit, b: Hit): boolean;
}

/** The index as `buildUseIndex` fills it. */
type Built<K, Hit, Use extends BaseUse<K>> = Map<string, Use> & { rated: Map<string, Map<K, Hit>> };

/** What the reply so far has shown of its rows, and the searches that later calls are held against. */
interface Reply<K, Hit, Use> {
  searches: Use[];
  known: Map<K, Hit>;
  /** The reply started with the app's own search (a recall block in the prompt). */
  recalled?: boolean;
}

/** The part of a chat the index reads. */
export type UseChat = Pick<ChatState, 'items' | 'toolCalls' | 'phase'>;

/**
 * Reads what a reply did with the rows its searches found. A reply starts at a message of the user and takes
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
 * A family reads only the calls of its own tools, so the searches of two families in one reply are judged independently.
 *
 * `previous` is the index of the last build. A search whose use did not change keeps its object from there (and the whole
 * index is `previous` when nothing changed), so the cards of the unchanged searches do not render again on every delta.
 */
export function buildUseIndex<K, Hit extends { id: K }, Use extends BaseUse<K>>(
  chat: UseChat,
  adapter: UseAdapter<K, Hit, Use>,
  previous?: UseIndex<K, Hit, Use>,
): UseIndex<K, Hit, Use> {
  const rated = new Map<string, Map<K, Hit>>();
  const index = Object.assign(new Map<string, Use>(), { rated });

  // A reply with no call of the family (a run of a script included) has nothing to say about the rows of a search, the app's own included.
  if (!Object.values(chat.toolCalls).some((call) => expandWrapped(call).some((one) => adapter.tool(one.name) !== null))) {
    return index;
  }

  let reply: Reply<K, Hit, Use> = { searches: [], known: new Map() };

  for (const item of chat.items) {
    if (item.kind === 'message' && item.role === 'user') {
      // A steer the agent took mid-reply carries no hook context (the app adds it to prompts only, and a hook follow-up is
      // told apart): the reply the app's own search started goes on, so its rows keep their marks.
      if (reply.recalled && isSteer(item)) {
        continue;
      }

      // The reply before this message is over, whatever the chat does now.
      for (const search of reply.searches) {
        search.ended = true;
      }

      reply = { searches: [], known: new Map() };
      startWithRecall(adapter, item, reply, index);
      continue;
    }

    const ids = item.kind === 'tool' ? [item.toolCallId] : item.kind === 'message' && item.role === 'assistant' ? item.blocks.flatMap((block) => (block.type === 'tool' ? [block.toolCallId] : [])) : [];

    for (const id of ids) {
      const call = chat.toolCalls[id];
      if (call) {
        expandWrapped(call).forEach((one) => absorb(adapter, one, reply, index, rated));
      }
    }
  }

  const over = chat.phase !== 'streaming';
  for (const search of reply.searches) {
    search.ended = over;
  }

  return previous ? reuse(adapter, index, previous) : index;
}

/** The app looks up notes and documents itself for a message: that is a search at the start of its reply, which the calls after it are held against like any other. */
function startWithRecall<K, Hit extends { id: K }, Use extends BaseUse<K>>(
  adapter: UseAdapter<K, Hit, Use>,
  message: MessageItem,
  reply: Reply<K, Hit, Use>,
  index: Map<string, Use>,
): void {
  const hits = adapter.recalled?.(message);
  if (!hits) {
    return;
  }

  const use = adapter.blank();
  index.set(recallCallId(message.key, adapter.family), use);
  reply.searches.push(use);
  reply.recalled = true;
  hits.forEach((hit) => reply.known.set(hit.id, hit));
}

/** `next` with the object of `previous` for every search and rating whose use is the same; `previous` itself when all of them are. */
function reuse<K, Hit extends { id: K }, Use extends BaseUse<K>>(
  adapter: UseAdapter<K, Hit, Use>,
  next: Built<K, Hit, Use>,
  previous: UseIndex<K, Hit, Use>,
): UseIndex<K, Hit, Use> {
  const ratings = next.rated;
  let changed = next.size !== previous.size || ratings.size !== (previous.rated?.size ?? 0);

  for (const [id, use] of next) {
    const before = previous.get(id);

    if (before && sameUse(adapter, before, use)) {
      next.set(id, before);
    } else {
      changed = true;
    }
  }

  for (const [id, hits] of ratings) {
    const before = previous.rated?.get(id);

    if (before && sameHits(adapter, before, hits)) {
      ratings.set(id, before as Map<K, Hit>);
    } else {
      changed = true;
    }
  }

  return changed ? next : previous;
}

function sameHits<K, Hit extends { id: K }, Use extends BaseUse<K>>(adapter: UseAdapter<K, Hit, Use>, a: ReadonlyMap<K, Hit>, b: ReadonlyMap<K, Hit>): boolean {
  return a.size === b.size && [...b].every(([id, hit]) => {
    const other = a.get(id);

    return other !== undefined && adapter.sameHit(other, hit);
  });
}

function sameUse<K, Hit extends { id: K }, Use extends BaseUse<K>>(adapter: UseAdapter<K, Hit, Use>, a: Use, b: Use): boolean {
  return (
    a.ended === b.ended &&
    a.rated.size === b.rated.size &&
    [...b.rated].every(([id, reason]) => a.rated.has(id) && a.rated.get(id) === reason) &&
    sameSet(a.read, b.read) &&
    (adapter.sameExtra?.(a, b) ?? true)
  );
}

/** Whether two sets hold the same values. */
export function sameSet<T>(a: ReadonlySet<T>, b: ReadonlySet<T>): boolean {
  return a.size === b.size && [...b].every((value) => a.has(value));
}

/**
 * A search starts a record and its rows become known to the reply. Any other call of the reply is held against the searches
 * before it; an open also makes its rows known, and a rating takes the titles of the rows it rated from them.
 */
function absorb<K, Hit extends { id: K }, Use extends BaseUse<K>>(
  adapter: UseAdapter<K, Hit, Use>,
  call: ToolCallState,
  reply: Reply<K, Hit, Use>,
  index: Map<string, Use>,
  rated: Map<string, Map<K, Hit>>,
): void {
  const tool = adapter.tool(call.name);
  const { searches, known } = reply;
  const { search, get, feedback } = adapter.names;

  if (tool === null) {
    return;
  }

  if (tool === search) {
    if (!index.has(call.id)) {
      const use = adapter.blank();
      index.set(call.id, use);
      searches.push(use);
    }

    if (call.status === 'ok') {
      adapter.searched(call.resultText).forEach((hit) => known.set(hit.id, hit));
    }

    return;
  }

  if (call.status !== 'ok') {
    return;
  }

  if (tool === get) {
    adapter.opened(call.resultText).forEach((hit) => known.set(hit.id, hit));
  }

  if (tool === feedback) {
    rated.set(call.id, new Map(adapter.rated(call).flatMap((id): [K, Hit][] => {
      const hit = known.get(id);

      return hit ? [[id, hit]] : [];
    })));
  }

  for (const use of searches) {
    if (tool === feedback) {
      rate(adapter, use, call);
    } else if (tool === get) {
      adapter.read(call).forEach((id) => use.read.add(id));
    } else {
      adapter.other?.(tool, call, use);
    }
  }
}

function rate<K, Hit extends { id: K }, Use extends BaseUse<K>>(adapter: UseAdapter<K, Hit, Use>, use: Use, call: ToolCallState): void {
  const reason = reasonArg(call.args);

  for (const id of adapter.rated(call)) {
    // The first word of the agent stands, unless a later one adds the reason the first left out.
    if (!use.rated.has(id) || (use.rated.get(id) === null && reason !== null)) {
      use.rated.set(id, reason);
    }
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

/** What the reply did with row `id` of a search: rated it useful, else opened it. Null when nothing is known of it. */
export function rowUse<K>(use: BaseUse<K> | undefined, id: K): RowUse | null {
  if (!use) {
    return null;
  }

  if (use.rated.has(id)) {
    return { kind: 'useful', reason: use.rated.get(id) ?? null };
  }

  return use.read.has(id) ? { kind: 'opened' } : null;
}
