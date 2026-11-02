import { type FoundDoc, knowledgeToolName, parseDocs, parseOpenedDocs, ratedDocs, readDocs } from './knowledge-tools';
import { type BaseUse, buildUseIndex, rowUse, type RowUse, type UseAdapter, type UseChat, type UseIndex } from './use-index';

/** What the later calls of the same reply say about the documents of one `knowledge_search` call: the ones it rated or opened. */
export type DocUse = BaseUse<string>;

/** The use of every `knowledge_search` call of a chat, by call id, and the documents each `knowledge_feedback` call rated that the same reply found or opened before it. */
export type KnowledgeUseIndex = UseIndex<string, FoundDoc, DocUse>;

const adapter: UseAdapter<string, FoundDoc, DocUse> = {
  tool: knowledgeToolName,
  names: { search: 'knowledge_search', get: 'knowledge_get', feedback: 'knowledge_feedback' },
  blank: () => ({ ended: false, rated: new Map(), read: new Set() }),
  searched: parseDocs,
  opened: parseOpenedDocs,
  rated: (call) => ratedDocs(call.args, call.resultText),
  read: (call) => readDocs(call.args, call.resultText),
  sameHit: (a, b) => a.title === b.title && a.section === b.section && a.format === b.format,
};

/** What a chat's replies did with the documents their searches found: see `buildUseIndex`. */
export function buildKnowledgeUse(chat: UseChat, previous?: KnowledgeUseIndex): KnowledgeUseIndex {
  return buildUseIndex(chat, adapter, previous);
}

/** What the reply did with document `id`: rated it useful, else opened it. Null when nothing is known of it. */
export function docUse(use: DocUse | undefined, id: string): RowUse | null {
  return rowUse(use, id);
}
