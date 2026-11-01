import type { KnowledgeCheck } from './bindings/KnowledgeCheck';
import type { KnowledgeDecided } from './bindings/KnowledgeDecided';
import type { KnowledgeDecision } from './bindings/KnowledgeDecision';
import type { KnowledgeDeleted } from './bindings/KnowledgeDeleted';
import type { KnowledgeDetail } from './bindings/KnowledgeDetail';
import type { KnowledgeHeldItem } from './bindings/KnowledgeHeldItem';
import type { KnowledgeHeldPage } from './bindings/KnowledgeHeldPage';
import type { KnowledgeHit } from './bindings/KnowledgeHit';
import type { KnowledgeImportEvent } from './bindings/KnowledgeImportEvent';
import type { KnowledgeImportOptions } from './bindings/KnowledgeImportOptions';
import type { KnowledgeImportResult } from './bindings/KnowledgeImportResult';
import type { KnowledgeIntake } from './bindings/KnowledgeIntake';
import type { KnowledgeLink } from './bindings/KnowledgeLink';
import type { KnowledgeListFilter } from './bindings/KnowledgeListFilter';
import type { KnowledgePage } from './bindings/KnowledgePage';
import type { KnowledgePulse } from './bindings/KnowledgePulse';
import type { KnowledgeReindexed } from './bindings/KnowledgeReindexed';
import type { KnowledgeRemote } from './bindings/KnowledgeRemote';
import type { KnowledgeReposPage } from './bindings/KnowledgeReposPage';
import type { KnowledgeSearchScope } from './bindings/KnowledgeSearchScope';
import type { KnowledgeStats } from './bindings/KnowledgeStats';
import type { KnowledgeStatus } from './bindings/KnowledgeStatus';
import type { KnowledgeSwitchResult } from './bindings/KnowledgeSwitchResult';
import type { MemoryEmbedderState } from './bindings/MemoryEmbedderState';
import type { MemoryRemoteKind } from './bindings/MemoryRemoteKind';
import type { MemoryVectorSwitchResult } from './bindings/MemoryVectorSwitchResult';
import { cmd } from './command-spec';
import type { EventSink } from './transport';

/** The `knowledge_*` commands, spread into `commands`. */
export const knowledgeCommands = {
  /**
   * Whether Harness Knowledge is installed and what its `features` line lists (`availability`, `features`). Never rejects for a missing or
   * old server: the answer says so.
   */
  knowledge_status: cmd<undefined, KnowledgeStatus>(),
  /**
   * The "Project knowledge" switch: stores `knowledgeEnabled` and registers or removes the harness-knowledge MCP entry.
   * `server.installed` false means nothing was registered: offer `setup_install_knowledge`.
   */
  knowledge_set_enabled: cmd<{ enabled: boolean }, KnowledgeSwitchResult>(),
  /** The team database in Harness Knowledge's config.toml: the URL redacted, the token as `hasToken`, and any `problem` with the file. */
  knowledge_remote_get: cmd<undefined, KnowledgeRemote>(),
  /**
   * Saves the team database (`enabled`) or removes it (the file is deleted). A null `url`/`token` keeps the saved one, `token: ''` clears it,
   * the token is stored for libSQL only. `invalidInput` for a URL or token the server would refuse, or a file the app must not overwrite.
   */
  knowledge_remote_set: cmd<{ enabled: boolean; kind: MemoryRemoteKind | null; url: string | null; token: string | null }, KnowledgeRemote>(),
  /** Runs `harness-knowledge check` with the given URL/token, or the saved ones where null. Never rejects for a failed check: see `ok` and `problem`. */
  knowledge_remote_test: cmd<{ kind: MemoryRemoteKind; url: string | null; token: string | null }, KnowledgeCheck>(),
  /**
   * The "Vector search" switch: stores `knowledgeVectorSearch`, then starts or stops the embedder. The answer holds the stored pref and the
   * embedder's state right then: follow it with `knowledge_embedder_watch`. `unsupported` for a server without `embedder`, `notFound` when it is not installed.
   */
  knowledge_set_vector_search: cmd<{ enabled: boolean }, MemoryVectorSwitchResult>(),
  /** Sends the embedder's state to `onEvent` now and on every change, replacing the previous listener. */
  knowledge_embedder_watch: cmd<{ onEvent: EventSink<MemoryEmbedderState> }, null>(),
  /** Stops the embedder, deletes the model files and the vectors and answers its state (`off`). Leaves `knowledgeVectorSearch` as it is. */
  knowledge_embedder_remove_model: cmd<undefined, MemoryEmbedderState>(),
  /** Embeds the chunks that have no vector yet (all documents, or the given ones). */
  knowledge_reindex: cmd<{ ids: number[] | null }, KnowledgeReindexed>(),
  /**
   * One page of documents. `offset` is a multiple of `limit` (1 to 200, 50 when null). Never rejects for a missing or old server: see
   * `availability` (`items` is empty unless `available`). `invalidInput` for a bad limit, offset, format or repository key.
   */
  knowledge_docs_list: cmd<{ filter: KnowledgeListFilter | null; offset: number | null; limit: number | null; query: string | null }, KnowledgePage>(),
  /** A document with its outline, and the chunk `chunkId` of it with its text when given. */
  knowledge_doc_get: cmd<{ id: number; chunkId: number | null }, KnowledgeDetail>(),
  /** Renames a document (1 to 300 characters) and answers the title as stored. */
  knowledge_doc_rename: cmd<{ id: number; title: string }, string>(),
  /** Replaces the repositories and path globs a document links to (none: global) and answers them as stored. */
  knowledge_doc_links_set: cmd<{ id: number; links: KnowledgeLink[] }, KnowledgeLink[]>(),
  /** Deletes documents with their chunks. Ids that do not exist count nothing. */
  knowledge_docs_delete: cmd<{ ids: number[] }, KnowledgeDeleted>(),
  /** The repositories that have documents, most documents first, and how many documents are global. */
  knowledge_repos_list: cmd<undefined, KnowledgeReposPage>(),
  /** What the database holds: documents, chunks, vectors, size. */
  knowledge_stats: cmd<undefined, KnowledgeStats>(),
  /** Hybrid search over the documents: the passages that match `query`, best first. */
  knowledge_search: cmd<{ query: string; project: string | null; scope: KnowledgeSearchScope | null; limit: number | null }, KnowledgeHit[]>(),
  /** One page of the review queue of documents agents proposed, newest first. Never rejects for a missing or old server: see `availability`. */
  knowledge_held_list: cmd<{ offset: number | null; limit: number | null; query: string | null }, KnowledgeHeldPage>(),
  /** One proposal with all of its text, for the review. */
  knowledge_held_get: cmd<{ id: string }, KnowledgeHeldItem>(),
  /**
   * Approves (imports) or rejects (throws away) a proposal, only the version whose `revision` is given. A refusal is an answer
   * (`notFound`, `changed`, `refused`), not an error.
   */
  knowledge_held_decide: cmd<{ id: string; decision: KnowledgeDecision; revision: string | null }, KnowledgeDecided>(),
  /** Sends the database's counts to `onEvent` now and whenever one changes, replacing the previous listener: the live list and the review badge. */
  knowledge_changes_listen: cmd<{ onEvent: EventSink<KnowledgePulse> }, null>(),
  /** Tells that the Knowledge view is open or closed: the counts are read every 4 s while it is, every 30 s otherwise. */
  knowledge_changes_watch: cmd<{ watching: boolean }, null>(),
  /** Lets the user choose documents in a native dialog and checks them. Null when cancelled. */
  knowledge_pick_files: cmd<undefined, KnowledgeIntake | null>(),
  /** Lets the user choose a folder of documents in a native dialog and checks it. Null when cancelled. */
  knowledge_pick_folder: cmd<undefined, KnowledgeIntake | null>(),
  /** The files and folders of the last drop on the window, checked, once. Null when nothing was dropped since. */
  knowledge_drop_intake: cmd<undefined, KnowledgeIntake | null>(),
  /**
   * Imports the files and folders of the intake `token`, linking every document to `options.links` (none: global). Each event goes to `onEvent`
   * as it happens. `invalidInput` while another import runs, `notFound` for a token that is gone, `unsupported` for a server without `import-ndjson`.
   */
  knowledge_import: cmd<{ token: string; options: KnowledgeImportOptions | null; onEvent: EventSink<KnowledgeImportEvent> }, KnowledgeImportResult>(),
  /** Asks the running import to stop between two documents. False when no import runs. */
  knowledge_import_cancel: cmd<undefined, boolean>(),
};
