import type { KnowledgeAvailability } from '../bindings/KnowledgeAvailability';
import type { KnowledgeDecided } from '../bindings/KnowledgeDecided';
import type { KnowledgeDocument } from '../bindings/KnowledgeDocument';
import type { KnowledgeHeldItem } from '../bindings/KnowledgeHeldItem';
import type { KnowledgeHit } from '../bindings/KnowledgeHit';
import type { KnowledgeId } from '../bindings/KnowledgeId';
import type { KnowledgePulse } from '../bindings/KnowledgePulse';
import type { CommandArgs } from '../commands';
import { idKey } from '../knowledge-id';
import { IpcFailure } from '../transport';
import { bulkDocuments, chunkText, DESKTOP_REPO, documentOf, outlineOf, sampleDocuments, sampleHeld } from './knowledge-fixtures';
import { sortDocuments, visibleDocuments } from './knowledge-list';
import { numberOf, placed } from './knowledge-routing';
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
  | 'knowledge_changes_watch';

/**
 * Browser-preview switch `?knowledge=`: `empty` starts without documents and proposals, `outdated` is a Harness Knowledge that predates
 * the features the app needs, `missing` is a database that lacks pgvector, and `bulk` adds 1500 documents for the scale of a long list. Anything else shows the sample documents.
 */
export const KNOWLEDGE_SCENARIOS = ['ok', 'empty', 'outdated', 'bulk', 'missing'] as const;
export type KnowledgeScenario = (typeof KNOWLEDGE_SCENARIOS)[number];

export function knowledgeScenarioFromLocation(search: string = globalThis.location?.search ?? ''): KnowledgeScenario {
  const name = new URLSearchParams(search).get('knowledge');

  return KNOWLEDGE_SCENARIOS.find((scenario) => scenario === name) ?? 'ok';
}

const BULK_COUNT = 1500;
const DEFAULT_LIMIT = 50;

/** The documents and the review queue of Harness Knowledge, kept in memory. */
export class MockKnowledge {
  documents: KnowledgeDocument[];
  held: KnowledgeHeldItem[];
  generation = 1;

  private sink: ((pulse: KnowledgePulse) => void) | null = null;

  /** `split`: both places are in use, so every third document is kept on this computer and has a `local:N` id. */
  constructor(
    scenario: KnowledgeScenario,
    readonly split = false,
  ) {
    const all = scenario === 'empty' ? [] : [...sampleDocuments(), ...(scenario === 'bulk' ? bulkDocuments(BULK_COUNT) : [])];

    this.documents = split ? all.map((doc) => placed(doc, numberOf(doc.id) % 3 === 0)) : all;
    this.held = scenario === 'empty' ? [] : sampleHeld();
  }

  pulse(): KnowledgePulse {
    return { documents: this.documents.length, generation: this.generation, held: this.held.length };
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

  find(id: KnowledgeId): KnowledgeDocument {
    const doc = this.documents.find((candidate) => idKey(candidate.id) === idKey(id));

    if (!doc) {
      throw IpcFailure.from({ code: 'process', message: `error: no document with id ${id}` });
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
function placeStats(documents: KnowledgeDocument[], place: 'remote' | 'local'): { documents: number; chunks: number; embeddedChunks: number; globalDocuments: number } {
  const kept = documents.filter((doc) => doc.place === place);
  const chunks = kept.reduce((sum, doc) => sum + doc.chunkCount, 0);

  return { documents: kept.length, chunks, embeddedChunks: chunks, globalDocuments: kept.filter((doc) => doc.links.length === 0).length };
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

  return {
    knowledge_status: () => {
      const on = available();

      return { availability: availability(), features: { docs: on, import: on, embedder: on, search: on, recall: on, held: on, mcp: on, routing: on, docMatches: on } };
    },

    knowledge_docs_list: ({ filter, offset, limit, query }) => {
      if (!available()) {
        return { availability: availability(), items: [], total: 0 };
      }

      const matching = sortDocuments(visibleDocuments(store.documents, filter, query), filter);
      const size = limit ?? DEFAULT_LIMIT;

      return { availability: availability(), items: matching.slice(offset ?? 0, (offset ?? 0) + size), total: matching.length };
    },

    knowledge_doc_get: ({ id, chunkId }) => {
      const document = store.find(id);
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
      const document = store.find(id);
      const trimmed = title.trim();

      if (trimmed === '' || trimmed.length > 300) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'a title has 1 to 300 characters' });
      }

      document.title = trimmed;
      store.changed();

      return trimmed;
    },

    knowledge_doc_links_set: ({ id, links }) => {
      store.find(id).links = structuredClone(links);
      store.changed();

      return links;
    },

    knowledge_docs_delete: ({ ids }) => {
      const before = store.documents.length;
      const gone = new Set(ids.map(idKey));
      store.documents = store.documents.filter((doc) => !gone.has(idKey(doc.id)));
      store.changed();

      return { deleted: before - store.documents.length };
    },

    knowledge_repos_list: () => ({
      availability: availability(),
      global: store.documents.filter((doc) => doc.links.length === 0).length,
      repos: [DESKTOP_REPO, 'git:github.com/useful/website'].map((key) => ({ key, docs: store.documents.filter((doc) => doc.links.some((link) => link.repo === key)).length })).filter((repo) => repo.docs > 0),
    }),

    knowledge_stats: () => {
      const chunks = store.documents.reduce((sum, doc) => sum + doc.chunkCount, 0);

      return {
        documents: store.documents.length,
        chunks,
        embeddedChunks: chunks,
        bytes: store.documents.reduce((sum, doc) => sum + doc.bytes, 0),
        globalDocuments: store.documents.filter((doc) => doc.links.length === 0).length,
        repositories: new Set(store.documents.flatMap((doc) => doc.links.map((link) => link.repo))).size,
        generation: store.generation,
        schema: 1,
        model: chunks > 0 ? 'multilingual-e5-small' : null,
        ...(store.split ? { places: { remote: placeStats(store.documents, 'remote'), local: placeStats(store.documents, 'local') } } : {}),
      };
    },

    knowledge_search: ({ query, limit }) => hitsOf(store.documents, query).slice(0, limit ?? 10),

    knowledge_reindex: ({ ids }) => ({ documents: ids?.length ?? store.documents.length, embedded: 0, failed: 0, cancelled: false, model: 'multilingual-e5-small' }),

    knowledge_held_list: ({ offset, limit, query }) => {
      const needle = (query ?? '').toLowerCase();
      const matching = store.held.filter((item) => item.title.toLowerCase().includes(needle) || item.name.toLowerCase().includes(needle));
      const size = limit ?? DEFAULT_LIMIT;

      return { availability: availability(), items: matching.slice(offset ?? 0, (offset ?? 0) + size), total: store.held.length };
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

      store.held = store.held.filter((candidate) => candidate !== item);

      if (args.decision === 'reject') {
        store.changed();

        return { type: 'rejected' };
      }

      const replaced = item.replaces === null ? null : store.documents.find((doc) => idKey(doc.id) === idKey(item.replaces as KnowledgeId));
      const docId = replaced?.id ?? store.newId(args.place);

      if (replaced) {
        replaced.updatedAt = new Date().toISOString();
      } else {
        const doc = documentOf({ id: docId, title: item.title, name: item.name, format: item.format, kb: Math.max(1, Math.ceil(item.bytes / 1024)), chunks: 4, age: 0, origin: 'agentApproved', links: item.links });

        store.documents.push(store.split ? { ...doc, place: args.place ?? 'remote' } : doc);
      }

      store.changed();

      return { type: 'approved', docId, status: replaced ? 'updated' : 'added', title: item.title, chunks: 4, embedding: 'done' };
    },

    knowledge_changes_listen: ({ onEvent }) => {
      store.listen(onEvent);

      return null;
    },

    knowledge_changes_watch: () => null,
  };
}
