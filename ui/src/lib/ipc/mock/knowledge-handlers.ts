import type { KnowledgeAvailability } from '../bindings/KnowledgeAvailability';
import type { KnowledgeDecided } from '../bindings/KnowledgeDecided';
import type { KnowledgeDocument } from '../bindings/KnowledgeDocument';
import type { KnowledgeHeldItem } from '../bindings/KnowledgeHeldItem';
import type { KnowledgeScope } from '../bindings/KnowledgeScope';
import type { SortOrder } from '../bindings/SortOrder';
import type { KnowledgeHit } from '../bindings/KnowledgeHit';
import type { KnowledgeId } from '../bindings/KnowledgeId';
import type { KnowledgeLink } from '../bindings/KnowledgeLink';
import type { KnowledgeOutcome } from '../bindings/KnowledgeOutcome';
import type { KnowledgePlaceStats } from '../bindings/KnowledgePlaceStats';
import type { KnowledgePulse } from '../bindings/KnowledgePulse';
import type { CommandArgs } from '../commands';
import { idKey } from '../knowledge-id';
import { IpcFailure } from '../transport';
import { bulkDocuments, chunkText, DESKTOP_REPO, documentOf, outlineOf, sampleDocuments, sampleHeld, withoutQuality } from './knowledge-fixtures';
import { sortDocuments, visibleDocuments } from './knowledge-list';
import { draftsApply, hasDrafts, ME, MockReview, pendingSummary, putPending, reviewCounts, seedReviewDocuments, seedRevisedProposal, waitingForYou } from './knowledge-review';
import { keptInTeam, localPathMessage, numberOf, placed } from './knowledge-routing';
import { ADA } from './authors';
import type { Handlers } from './types';

type KnowledgeCommands =
  | 'knowledge_status'
  | 'knowledge_docs_list'
  | 'knowledge_doc_get'
  | 'knowledge_doc_matches'
  | 'knowledge_doc_rename'
  | 'knowledge_doc_links_set'
  | 'knowledge_docs_delete'
  | 'knowledge_repos_list'
  | 'knowledge_stats'
  | 'knowledge_search'
  | 'knowledge_reindex'
  | 'knowledge_held_list'
  | 'knowledge_held_get'
  | 'knowledge_held_decide'
  | 'knowledge_changes_listen'
  | 'knowledge_changes_watch'
  | 'knowledge_restart_notice_dismiss';

/**
 * Browser-preview switch `?knowledge=`: `empty` starts without documents and proposals, `outdated` is a Harness Knowledge that predates
 * the features the app needs, `older` one that lacks `docs-useful`, `docs-similar` and `docs-authors` (no counts, no authors, no comparison), `missing` is a database that
 * lacks pgvector, `readonly` a team database the user may not change, and `bulk` adds 1500 documents for the scale of a long list. `drafts` adds documents in every
 * state (a draft, two that wait, one with changes requested, updates of live documents) with team approval on, `solo` the same with approval off, and `upgraded` shows
 * the one-time notice to restart harnesses. `older` has no drafts at all. Anything else shows the sample documents.
 */
export const KNOWLEDGE_SCENARIOS = ['ok', 'empty', 'outdated', 'older', 'bulk', 'missing', 'readonly', 'drafts', 'solo', 'upgraded'] as const;
export type KnowledgeScenario = (typeof KNOWLEDGE_SCENARIOS)[number];

export function knowledgeScenarioFromLocation(search: string = globalThis.location?.search ?? ''): KnowledgeScenario {
  const name = new URLSearchParams(search).get('knowledge');

  return KNOWLEDGE_SCENARIOS.find((scenario) => scenario === name) ?? 'ok';
}

const UNREACHABLE_PREFIX = 'harness-knowledge:';
const BULK_COUNT = 1500;
const DEFAULT_LIMIT = 50;

/** The documents and the review queue of Harness Knowledge, kept in memory. */
export class MockKnowledge {
  documents: KnowledgeDocument[];
  held: KnowledgeHeldItem[];
  generation = 1;
  /** Team approval, the history and the comments. */
  readonly review: MockReview;
  /** A team database is set up, so a document kept in the remote place is in it. Without one nothing is refused for being shared. */
  team = false;
  /** The database was upgraded and the user has not dismissed the notice to restart harnesses. */
  restartNotice: boolean;

  private sink: ((pulse: KnowledgePulse) => void) | null = null;

  /** `split`: both places are in use, so every third document is kept on this computer and has a `local:N` id. */
  constructor(
    readonly scenario: KnowledgeScenario,
    readonly split = false,
    /** The remote database cannot be reached: only the documents kept on this computer can be read, and writes to the remote are refused. */
    public offline = false,
  ) {
    const all = scenario === 'empty' ? [] : [...sampleDocuments(), ...(scenario === 'bulk' ? bulkDocuments(BULK_COUNT) : []), ...(hasDrafts(scenario) ? seedReviewDocuments() : [])];

    this.documents = split ? all.map((doc) => placed(doc, numberOf(doc.pendingOf ?? doc.id) % 3 === 0)) : all;
    this.held = scenario === 'empty' ? [] : [...sampleHeld(), ...(hasDrafts(scenario) ? [seedRevisedProposal()] : [])];
    this.review = new MockReview(scenario);
    this.restartNotice = scenario === 'upgraded';
  }

  /** Whether this Harness Knowledge keeps drafts (`docs-drafts`): every one but an older one. */
  get drafts(): boolean {
    return this.scenario !== 'older';
  }

  pulse(): KnowledgePulse {
    const base = { documents: this.reachable().length, generation: this.generation, held: this.held.length, offline: this.offline };

    return this.drafts ? { ...base, waiting: waitingForYou(this.documents), waitingAll: this.documents.filter((doc) => doc.status === 'waiting').length, approval: this.review.approval, restartHarnesses: this.restartNotice ? true : null, me: ME } : base;
  }

  /** Tells the listener the database changed. */
  changed(): void {
    this.generation += 1;
    this.sink?.(this.pulse());
  }

  listen(sink: (pulse: KnowledgePulse) => void): void {
    this.sink = sink;
    sink(this.pulse());
  }

  nextId(): number {
    return this.documents.reduce((most, doc) => Math.max(most, numberOf(doc.id)), 0) + 1;
  }

  /** The id a new document gets in `place` (a plain number, or `local:N` when both places are in use and it is kept here). */
  newId(place: 'remote' | 'local' | undefined): KnowledgeId {
    return this.split && place === 'local' ? `local:${this.nextId()}` : this.nextId();
  }

  /** The documents that can be read now: all of them, or only this computer's while the remote is unreachable. A pending version is no row of a list of its own. */
  reachable(): KnowledgeDocument[] {
    const rows = this.documents.filter((doc) => doc.pendingOf == null);

    return this.offline ? rows.filter((doc) => doc.place === 'local') : rows;
  }

  /** The pending version of a live document, when it has one. */
  pendingOf(doc: KnowledgeDocument): KnowledgeDocument | undefined {
    return this.documents.find((row) => row.pendingOf != null && idKey(row.pendingOf) === idKey(doc.id));
  }

  /**
   * `doc` as the server shows it: a live document carries the summary of its pending version, and a row that is not live (and that summary) the names of its authors. The server
   * names them in `docs get` only, the mock in lists too.
   */
  view(doc: KnowledgeDocument): KnowledgeDocument {
    const pending = this.pendingOf(doc);

    if (doc.status === 'live') {
      return pending ? { ...doc, pending: pendingSummary(pending, this.review.authorsOf(pending)) } : doc;
    }

    return { ...doc, authors: this.review.authorsOf(doc) };
  }

  /** The document with this id. A remote one cannot be read while offline, or changed (`write`): the server's lines for each. */
  find(id: KnowledgeId, write = false): KnowledgeDocument {
    const doc = this.documents.find((candidate) => idKey(candidate.id) === idKey(id));

    if (!doc) {
      throw IpcFailure.from({ code: 'process', message: `error: no document with id ${id}` });
    }

    if (this.offline && doc.place !== 'local') {
      throw IpcFailure.from({ code: 'unreachable', message: `${UNREACHABLE_PREFIX} ${write ? 'the remote database is unreachable, so nothing was changed' : 'remote knowledge is unreachable right now'}` });
    }

    return doc;
  }
}

function hitsOf(docs: KnowledgeDocument[], query: string): KnowledgeHit[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);

  return docs
    .filter((doc) => words.some((word) => doc.title.toLowerCase().includes(word)))
    .flatMap((doc, index) => {
      // The first section, and the third when the document has one.
      const sections = outlineOf(doc);

      return [sections[0], sections[2]].filter((section) => section !== undefined).map((section, rank) => ({
        docId: doc.id,
        chunkId: section.chunkId,
        title: doc.title,
        section: section.headingPath,
        snippet: `This part of ${doc.title} mentions ${query.trim()} where it describes the ${section.headingPath.toLowerCase()} and the steps to follow.`,
        score: Math.round((0.92 - index * 0.07 - rank * 0.03) * 100) / 100,
      }));
    });
}

/** The start of a chunk's text around the first word of the search, on one line. */
function snippetOf(text: string, phrase: string): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  const at = Math.max(0, flat.toLowerCase().indexOf(phrase.split(' ')[0] ?? ''));

  return flat.slice(Math.max(0, at - 10), at + 30);
}

/** What one place holds, for the `places` of the counts. */
function placeStats(documents: KnowledgeDocument[], place: 'remote' | 'local', offline: boolean, generation: number): KnowledgePlaceStats {
  const kept = documents.filter((doc) => doc.pendingOf == null && doc.place === place);
  const chunks = kept.reduce((sum, doc) => sum + doc.chunkCount, 0);

  return offline && place === 'remote' ? { documents: 0, chunks: 0, embeddedChunks: 0, globalDocuments: 0, generation, offline: true } : { documents: kept.length, chunks, embeddedChunks: chunks, globalDocuments: kept.filter((doc) => doc.links.length === 0).length, generation, offline: false };
}

function heldNotFound(): KnowledgeDecided {
  return { type: 'notFound', message: 'error: that proposal was decided already' };
}

/**
 * The Knowledge commands over the in-memory database. `available` says whether the preview's Harness Knowledge is installed (the setup
 * mock installs it) and the scenario says whether it is current.
 */
export function knowledgeHandlers(store: MockKnowledge, scenario: () => KnowledgeScenario, installed: () => boolean): Pick<Handlers, KnowledgeCommands> {
  const availability = (): KnowledgeAvailability => {
    if (!installed()) {
      return { type: 'notInstalled' };
    }

    switch (scenario()) {
      case 'outdated':
        return { type: 'outdated' };
      case 'missing':
        return { type: 'missing', problem: 'noPgvector', message: 'the extension "vector" is not installed' };
      default:
        return { type: 'available' };
    }
  };
  const available = () => availability().type === 'available';
  // A Harness Knowledge without `docs-useful`, `docs-similar` and `docs-authors`: the app asks it for none of them, and its rows carry none.
  const older = () => scenario() === 'older';

  return {
    knowledge_status: () => {
      const on = available();

      const newer = on && !older();

      return { availability: availability(), features: { docs: on, import: on, embedder: on, search: on, recall: on, held: on, mcp: on, routing: on, docMatches: on, offlineFallback: on, docsUseful: newer, docsSimilar: newer, docsAuthors: newer, docsDrafts: newer, docsApproval: newer, docsSortPaths: newer } };
    },

    knowledge_docs_list: ({ filter, offset, limit, query }) => {
      if (!available()) {
        return { availability: availability(), items: [], total: 0, offline: false };
      }

      // The app drops the filter and the sorts a server without `docs-useful` or `docs-sort-paths` would refuse, and reads the default order instead.
      const asked = older() && filter ? { ...filter, useful: null, status: null, sort: filter.sort === 'useful' || filter.sort === 'lastUseful' || filter.sort === 'paths' ? null : filter.sort } : filter;
      const matching = sortDocuments(visibleDocuments(store.reachable(), asked, query, (doc) => store.pendingOf(doc)), asked);
      const size = limit ?? DEFAULT_LIMIT;
      const items = matching.slice(offset ?? 0, (offset ?? 0) + size).map((doc) => store.view(doc));

      return { availability: availability(), items: older() ? items.map(withoutQuality) : items, total: matching.length, offline: store.offline };
    },

    knowledge_doc_get: ({ id, chunkId }) => {
      const document = older() ? withoutQuality(store.find(id)) : store.view(store.find(id));
      const sections = outlineOf(document);
      const section = sections.find((candidate) => chunkId !== null && idKey(candidate.chunkId) === idKey(chunkId));

      return { document, sections, chunk: section ? { id: section.chunkId, docId: id, ordinal: section.ordinal, headingPath: section.headingPath, text: chunkText(document, section), tokens: section.tokens, charStart: section.charStart, charEnd: section.charEnd } : null };
    },

    knowledge_doc_matches: ({ id, query, limit }) => {
      const document = store.find(id);
      const phrase = query.trim().toLowerCase();
      const found = outlineOf(document)
        .map((section) => ({ section, text: chunkText(document, section) }))
        .filter(({ section, text }) => phrase !== '' && `${section.headingPath}\n${text}`.toLowerCase().includes(phrase));

      return {
        total: found.length,
        matches: found.slice(0, limit ?? 20).map(({ section, text }) => ({ chunkId: section.chunkId, ordinal: section.ordinal, section: section.headingPath, snippet: snippetOf(text, phrase) })),
      };
    },

    knowledge_doc_rename: ({ id, title }) => {
      const document = store.find(id, true);
      const trimmed = title.trim();

      if (trimmed === '' || trimmed.length > 300) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'a title has 1 to 300 characters' });
      }

      document.title = trimmed;

      // A rename of a row that is not live is a change of it: the person who made it is one of its authors, as for a live document the one of its pending version.
      const renamed = document.status === 'live' ? store.pendingOf(document) : document;

      if (renamed && store.drafts) {
        store.review.record(renamed.id, 'edited', ME, 'renamed');
      }

      store.changed();

      return trimmed;
    },

    knowledge_doc_links_set: ({ id, links }): KnowledgeOutcome<KnowledgeLink[]> => {
      const doc = store.find(id, true);
      const refusal = store.team && keptInTeam(doc.place) ? localPathMessage(links) : null;

      // Like the server, a team document cannot link to a folder without a git remote: the document stays as it was.
      if (refusal !== null) {
        return { type: 'refused', code: 'localPath', message: refusal };
      }

      doc.links = structuredClone(links);
      store.changed();

      return { type: 'done', value: links };
    },

    knowledge_docs_delete: ({ ids }) => {
      ids.forEach((id) => store.find(id, true));

      const before = store.reachable().length;
      const gone = new Set(ids.map(idKey));
      // Deleting a live document deletes its pending version, deleting a pending version only discards the update.
      store.documents = store.documents.filter((doc) => !gone.has(idKey(doc.id)) && !(doc.pendingOf != null && gone.has(idKey(doc.pendingOf))));
      store.changed();

      return { deleted: before - store.reachable().length };
    },

    knowledge_repos_list: () => ({
      availability: availability(),
      global: store.reachable().filter((doc) => doc.links.length === 0).length,
      repos: [DESKTOP_REPO, 'git:github.com/useful/website'].map((key) => ({ key, docs: store.reachable().filter((doc) => doc.links.some((link) => link.repo === key)).length })).filter((repo) => repo.docs > 0),
      offline: store.offline,
    }),

    knowledge_stats: () => {
      // With both places in use the top level is the remote database's alone (this computer's while it cannot be reached), `places` has each.
      const kept = store.split && !store.offline ? store.documents.filter((doc) => doc.pendingOf == null && doc.place === 'remote') : store.reachable();
      const chunks = kept.reduce((sum, doc) => sum + doc.chunkCount, 0);

      return {
        documents: kept.length,
        chunks,
        embeddedChunks: chunks,
        bytes: kept.reduce((sum, doc) => sum + doc.bytes, 0),
        globalDocuments: kept.filter((doc) => doc.links.length === 0).length,
        repositories: new Set(kept.flatMap((doc) => doc.links.map((link) => link.repo))).size,
        generation: store.generation,
        schema: 1,
        model: chunks > 0 ? 'multilingual-e5-small' : null,
        offline: store.offline,
        // The remote place's answer: unknown while it cannot be reached, absent from a server that does not say.
        ...(older() ? {} : { remoteWritable: store.offline ? null : scenario() !== 'readonly', ...reviewCounts(store.documents, kept, store.review.approval), me: { name: ME } }),
        ...(store.split ? { places: { remote: placeStats(store.documents, 'remote', store.offline, store.generation), local: placeStats(store.documents, 'local', false, store.generation) } } : {}),
      };
    },

    knowledge_search: ({ query, limit }) => hitsOf(store.documents.filter((doc) => doc.pendingOf == null), query).slice(0, limit ?? 10),

    knowledge_reindex: ({ ids }) => ({ documents: ids?.length ?? store.documents.length, embedded: 0, failed: 0, cancelled: false, model: 'multilingual-e5-small' }),

    knowledge_held_list: ({ offset, limit, query, scope, sort, order }) => {
      const needle = (query ?? '').toLowerCase();
      const matching = store.held
        .filter((item) => item.title.toLowerCase().includes(needle) || item.name.toLowerCase().includes(needle))
        .filter((item) => heldInScope(item, scope ?? { kind: 'all' }));
      const size = limit ?? DEFAULT_LIMIT;

      return { availability: availability(), items: sortHeld(matching, sort, order).slice(offset ?? 0, (offset ?? 0) + size), total: store.held.length };
    },

    knowledge_held_get: ({ id }) => {
      const item = store.held.find((candidate) => candidate.id === id);

      if (!item) {
        throw IpcFailure.from({ code: 'notFound', message: 'error: no such proposal' });
      }

      return item;
    },

    knowledge_held_decide: (args: CommandArgs<'knowledge_held_decide'>): KnowledgeDecided => {
      const item = store.held.find((candidate) => candidate.id === args.id);

      if (!item) {
        return heldNotFound();
      }

      if (args.revision !== null && args.revision !== item.revision) {
        return { type: 'changed', message: 'error: the proposal changed since it was shown' };
      }

      // Approving into the team database a proposal that links to a folder without a git remote is refused, and the proposal stays in the queue.
      const refusal = args.decision === 'approve' && store.team && keptInTeam(args.place) ? localPathMessage(item.links) : null;

      if (refusal !== null) {
        return { type: 'localPath', message: refusal };
      }

      store.held = store.held.filter((candidate) => candidate !== item);

      if (args.decision === 'reject') {
        store.changed();

        return { type: 'rejected' };
      }

      const replaced = item.replaces === null ? null : store.documents.find((doc) => idKey(doc.id) === idKey(item.replaces as KnowledgeId));
      // With team approval on, an approval stores a draft (a new document) or the pending version of the live document it replaces, and agents do not see either yet.
      const draft = draftsApply(store, args.place);
      const docId = replaced?.id ?? store.newId(args.place);

      if (replaced && draft) {
        const pending = putPending(store, replaced, ADA, false);
        store.changed();

        return { type: 'approved', docId: pending.id, status: 'pending', title: item.title, chunks: 4, embedding: 'done', draft: true };
      }

      if (replaced) {
        replaced.updatedAt = new Date().toISOString();
      } else {
        const doc = documentOf({ id: docId, title: item.title, name: item.name, format: item.format, kb: Math.max(1, Math.ceil(item.bytes / 1024)), chunks: 4, age: 0, origin: 'agentApproved', links: item.links, status: draft ? 'draft' : 'live', revision: draft ? 1 : 0 });

        store.documents.push(store.split ? { ...doc, place: args.place ?? 'remote' } : doc);

        if (draft) {
          store.review.record(docId, 'drafted', ADA.name);
        }
      }

      store.changed();

      return { type: 'approved', docId, status: replaced ? 'updated' : 'added', title: item.title, chunks: 4, embedding: 'done', draft: !replaced && draft };
    },

    knowledge_changes_listen: ({ onEvent }) => {
      store.listen(onEvent);

      return null;
    },

    knowledge_changes_watch: () => null,

    knowledge_restart_notice_dismiss: () => {
      store.restartNotice = false;
      store.changed();

      return null;
    },
  };
}

/** As the app sorts the queue: by a column, newest or Z first unless asked, ties by the time. */
function sortHeld(items: KnowledgeHeldItem[], sort: string | null, order: SortOrder | null): KnowledgeHeldItem[] {
  const direction = order === 'asc' ? 1 : -1;

  // By the person a proposal is written for: names A to Z with case ignored, proposals with no name last in either direction.
  if (sort === 'addedBy') {
    const name = (item: KnowledgeHeldItem) => item.addedBy?.trim().toLowerCase() || null;

    return [...items].sort((left, right) => {
      const [leftName, rightName] = [name(left), name(right)];

      if (leftName === null || rightName === null) {
        return Number(leftName === null) - Number(rightName === null) || right.createdAt.localeCompare(left.createdAt);
      }

      return leftName.localeCompare(rightName) * direction || right.createdAt.localeCompare(left.createdAt);
    });
  }

  const folder = (item: KnowledgeHeldItem) => (item.projectPath ?? '').split(/[\\/]/).filter(Boolean).at(-1)?.toLowerCase() ?? '';
  const key = (item: KnowledgeHeldItem): string => {
    switch (sort) {
      case 'title':
        return item.title.toLowerCase();
      case 'client':
        return item.client.toLowerCase();
      case 'project':
        return folder(item);
      default:
        return '';
    }
  };

  return [...items].sort((left, right) => (key(left).localeCompare(key(right)) || left.createdAt.localeCompare(right.createdAt)) * direction);
}

/** As the app scopes the queue: global when it links to no repository, else its repositories and the one its agent worked in. */
function heldInScope(item: KnowledgeHeldItem, scope: KnowledgeScope): boolean {
  if (scope.kind === 'all') {
    return true;
  }

  if (scope.kind === 'global') {
    return item.links.length === 0;
  }

  return item.links.some((link) => link.repo === scope.key) || item.repoKey === scope.key;
}
