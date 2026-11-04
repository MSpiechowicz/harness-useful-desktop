import type { SortOrder } from './bindings/SortOrder';
import type { KnowledgeScope } from './bindings/KnowledgeScope';
import type { KnowledgeCheck } from './bindings/KnowledgeCheck';
import type { KnowledgeChangesRequested } from './bindings/KnowledgeChangesRequested';
import type { KnowledgeComment } from './bindings/KnowledgeComment';
import type { KnowledgeCommentChanged } from './bindings/KnowledgeCommentChanged';
import type { KnowledgeCommentsDeleted } from './bindings/KnowledgeCommentsDeleted';
import type { KnowledgeCompare } from './bindings/KnowledgeCompare';
import type { KnowledgeDownloaded } from './bindings/KnowledgeDownloaded';
import type { KnowledgeHistory } from './bindings/KnowledgeHistory';
import type { KnowledgeOutcome } from './bindings/KnowledgeOutcome';
import type { KnowledgeReviewSubmit } from './bindings/KnowledgeReviewSubmit';
import type { KnowledgeTeamApproval } from './bindings/KnowledgeTeamApproval';
import type { KnowledgeTransition } from './bindings/KnowledgeTransition';
import type { KnowledgeTransitioned } from './bindings/KnowledgeTransitioned';
import type { KnowledgeDecided } from './bindings/KnowledgeDecided';
import type { KnowledgeDecision } from './bindings/KnowledgeDecision';
import type { KnowledgeDeleted } from './bindings/KnowledgeDeleted';
import type { KnowledgeDetail } from './bindings/KnowledgeDetail';
import type { KnowledgeDocMatches } from './bindings/KnowledgeDocMatches';
import type { KnowledgeHeldItem } from './bindings/KnowledgeHeldItem';
import type { KnowledgeHeldPage } from './bindings/KnowledgeHeldPage';
import type { KnowledgeHit } from './bindings/KnowledgeHit';
import type { KnowledgeId } from './bindings/KnowledgeId';
import type { KnowledgeImportEvent } from './bindings/KnowledgeImportEvent';
import type { KnowledgeImportOptions } from './bindings/KnowledgeImportOptions';
import type { KnowledgeImportResult } from './bindings/KnowledgeImportResult';
import type { KnowledgeIntake } from './bindings/KnowledgeIntake';
import type { KnowledgeLink } from './bindings/KnowledgeLink';
import type { KnowledgeListFilter } from './bindings/KnowledgeListFilter';
import type { KnowledgePage } from './bindings/KnowledgePage';
import type { KnowledgePlace } from './bindings/KnowledgePlace';
import type { KnowledgePulse } from './bindings/KnowledgePulse';
import type { KnowledgeReindexed } from './bindings/KnowledgeReindexed';
import type { KnowledgeRemote } from './bindings/KnowledgeRemote';
import type { KnowledgeReposPage } from './bindings/KnowledgeReposPage';
import type { KnowledgeSearchScope } from './bindings/KnowledgeSearchScope';
import type { KnowledgeSimilar } from './bindings/KnowledgeSimilar';
import type { KnowledgeStats } from './bindings/KnowledgeStats';
import type { KnowledgeStatus } from './bindings/KnowledgeStatus';
import type { KnowledgeSwitchResult } from './bindings/KnowledgeSwitchResult';
import type { MemoryEmbedderState } from './bindings/MemoryEmbedderState';
import type { MemoryNotesRouting } from './bindings/MemoryNotesRouting';
import type { MemoryRemoteKind } from './bindings/MemoryRemoteKind';
import type { MemoryVectorSwitchResult } from './bindings/MemoryVectorSwitchResult';
import { cmd } from './command-spec';
import type { EventSink } from './transport';

/** What `knowledge_remote_set` takes. */
export interface KnowledgeRemoteSetArgs {
  enabled: boolean;
  kind: MemoryRemoteKind | null;
  url: string | null;
  token: string | null;
  repositoryDocuments?: MemoryNotesRouting;
  globalDocuments?: MemoryNotesRouting;
}

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
   * the token is stored for libSQL only. `repositoryDocuments` and `globalDocuments` say where documents are kept (omitted keeps the saved
   * ones, `unsupported` while the server lacks `routing`). `invalidInput` for a URL or token the server would refuse, or a file the app must not overwrite.
   */
  knowledge_remote_set: cmd<KnowledgeRemoteSetArgs, KnowledgeRemote>(),
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
  knowledge_reindex: cmd<{ ids: KnowledgeId[] | null }, KnowledgeReindexed>(),
  /**
   * One page of documents. `offset` is a multiple of `limit` (1 to 200, 50 when null). Never rejects for a missing or old server: see
   * `availability` (`items` is empty unless `available`). `invalidInput` for a bad limit, offset, format or repository key.
   */
  knowledge_docs_list: cmd<{ filter: KnowledgeListFilter | null; offset: number | null; limit: number | null; query: string | null }, KnowledgePage>(),
  /** A document with its outline, and the chunk `chunkId` of it with its text when given. */
  knowledge_doc_get: cmd<{ id: KnowledgeId; chunkId: KnowledgeId | null }, KnowledgeDetail>(),
  /** The chunks of a document that match `query` (a phrase, in heading and text), in document order, with their total. Needs `docMatches`. */
  knowledge_doc_matches: cmd<{ id: KnowledgeId; query: string; limit: number | null }, KnowledgeDocMatches>(),
  /** Renames a document (1 to 300 characters) and answers the title as stored. */
  knowledge_doc_rename: cmd<{ id: KnowledgeId; title: string }, string>(),
  /** Replaces the repositories and path globs a document links to (none: global) and answers them as stored. */
  knowledge_doc_links_set: cmd<{ id: KnowledgeId; links: KnowledgeLink[] }, KnowledgeOutcome<KnowledgeLink[]>>(),
  /** Deletes documents with their chunks. Ids that do not exist count nothing. */
  knowledge_docs_delete: cmd<{ ids: KnowledgeId[] }, KnowledgeDeleted>(),
  /** The repositories that have documents, most documents first, and how many documents are global. */
  knowledge_repos_list: cmd<undefined, KnowledgeReposPage>(),
  /** What the database holds: documents, chunks, vectors, size. */
  knowledge_stats: cmd<undefined, KnowledgeStats>(),
  /** Hybrid search over the documents: the passages that match `query`, best first. */
  knowledge_search: cmd<{ query: string; project: string | null; scope: KnowledgeSearchScope | null; limit: number | null }, KnowledgeHit[]>(),
  /** One page of the review queue of documents agents proposed, newest first. Never rejects for a missing or old server: see `availability`. */
  knowledge_held_list: cmd<{ offset: number | null; limit: number | null; query: string | null; scope: KnowledgeScope | null; sort: string | null; order: SortOrder | null }, KnowledgeHeldPage>(),
  /** One proposal with all of its text, for the review. */
  knowledge_held_get: cmd<{ id: string }, KnowledgeHeldItem>(),
  /**
   * Approves (imports) or rejects (throws away) a proposal, only the version whose `revision` is given. A rejection may carry a `comment` for the
   * agent (1 to 500 characters, dropped for a server without `docsDrafts`). An approval may be `pending` or a `draft` (team approval on). A refusal is
   * an answer (`notFound`, `changed`, `refused`), not an error.
   */
  knowledge_held_decide: cmd<{ id: string; decision: KnowledgeDecision; revision: string | null; place?: KnowledgePlace; comment?: string | null }, KnowledgeDecided>(),
  /**
   * Moves a document one step, only while its `revision` is the one shown: `ask` (draft or changes requested to waiting), `requestChanges` (waiting to
   * changes requested, needs a `comment` of 1 to 500 characters), `approve` (waiting to live, by a teammate of the author) or `goLive` (by its author,
   * with approval off). The first three need `docsApproval`, `goLive` needs `docsDrafts`. Answers the state it is in (the live id after a promote) or the
   * server's refusal (`changed`, `wrongState`, `approvalOn`, `approvalOff`, `selfApproval`, `noName`, `readOnly`, `commentRequired`). Rejects with
   * `unreachable` while the team database cannot be reached.
   */
  knowledge_doc_transition: cmd<{ id: KnowledgeId; action: KnowledgeTransition; revision: number; comment: string | null }, KnowledgeOutcome<KnowledgeTransitioned>>(),
  /**
   * Submits a review of a pending version: the overall `body`, the comments on lines of the compare (at most 200, 1 to 4000 characters each) and the
   * verdict (`comment` changes nothing, `approve` makes it live, `requestChanges` needs an overall comment). Only while `revision` is the one reviewed.
   * Answers like `knowledge_doc_transition`.
   */
  knowledge_doc_review: cmd<{ id: KnowledgeId; revision: number; review: KnowledgeReviewSubmit }, KnowledgeOutcome<KnowledgeTransitioned>>(),
  /** The events of a document, newest first (at most 200), with the rounds of its promoted pending versions. Needs `docsDrafts`. */
  knowledge_doc_history: cmd<{ id: KnowledgeId }, KnowledgeHistory>(),
  /** The comments of a document (the pending row, or a live row that has a pending version), replies and resolved ones included. Needs `docsDrafts`. */
  knowledge_doc_comments: cmd<{ id: KnowledgeId }, KnowledgeComment[]>(),
  /** Answers a comment (1 to 4000 characters). Anybody may reply, the author included. */
  knowledge_doc_comment_reply: cmd<{ id: KnowledgeId; commentId: KnowledgeId; body: string }, KnowledgeOutcome<KnowledgeCommentChanged>>(),
  /** Resolves a comment (`resolved: true`) or opens it again. */
  knowledge_doc_comment_resolve: cmd<{ id: KnowledgeId; commentId: KnowledgeId; resolved: boolean }, KnowledgeOutcome<KnowledgeCommentChanged>>(),
  /** Deletes a comment the user wrote, with its replies when it starts a thread. Refuses `notAuthor` for another person's comment. History is untouched. */
  knowledge_doc_comment_delete: cmd<{ id: KnowledgeId; commentId: KnowledgeId }, KnowledgeOutcome<KnowledgeCommentsDeleted>>(),
  /**
   * Compares a pending version (the pending row, or a live row that has one) with its live document, or with the version asked at revision `since`:
   * the sections that differ as the Changes view shows files (`path` is the heading path). A new document compares with nothing, every line is added.
   * Refuses with `sinceUnavailable` when that version was not kept.
   */
  knowledge_doc_compare: cmd<{ id: KnowledgeId; since: number | null }, KnowledgeOutcome<KnowledgeCompare>>(),
  /**
   * Lets the user choose where to save a document in a native dialog (suggesting `name`, a file name) and writes its original file there, or its stored
   * text as Markdown or text when the original was not kept (`original` says which). `live` takes the live document's file instead of its pending
   * version's. Null when the user cancelled.
   */
  knowledge_doc_download: cmd<{ id: KnowledgeId; name: string | null; live: boolean | null }, KnowledgeOutcome<KnowledgeDownloaded> | null>(),
  /**
   * Opens the file manager with a file this app just saved with `knowledge_doc_download` selected. `path` must be exactly the path the save dialog
   * returned (`KnowledgeDownloaded.path`) for a download of this run: any other path is `forbidden`, a saved file that is gone `notFound`.
   */
  knowledge_reveal_download: cmd<{ path: string }, null>(),
  /** Whether team approval is on, where it is kept and whether the user may change it. Needs `docsApproval`. */
  knowledge_approval_get: cmd<undefined, KnowledgeTeamApproval>(),
  /** Turns team approval on or off for the team database (applied at once). Refuses with `readOnly` for a read-only role, `refused` with no team database. */
  knowledge_approval_set: cmd<{ on: boolean }, KnowledgeOutcome<KnowledgeTeamApproval>>(),
  /**
   * Asks the agent of a proposal for changes: the proposal stays in the queue as `changesRequested` with the `comment` (1 to 500 characters), only while
   * `revision` is the one shown. When the chat of that agent is still open in this app the comment is also sent to it as a follow-up message and
   * `delivered` says so.
   */
  knowledge_held_request_changes: cmd<{ id: string; revision: string | null; comment: string }, KnowledgeChangesRequested>(),
  /**
   * Whether the `session` a proposal records belongs to a chat that is open in this app and works in `projectPath`, the folder the proposal was about
   * (`projectPath` of the proposal): false without either, and for a chat of another project, so "Request changes" can tell its agent.
   */
  knowledge_held_follow_up_available: cmd<{ session: string | null; projectPath?: string | null }, boolean>(),
  /** The user dismissed the notice to restart running harnesses after the database was upgraded: the pulse stops carrying `restartHarnesses`. */
  knowledge_restart_notice_dismiss: cmd<undefined, null>(),
  /** Sends the database's counts to `onEvent` now and whenever one changes, replacing the previous listener: the live list and the review badge. */
  knowledge_changes_listen: cmd<{ onEvent: EventSink<KnowledgePulse> }, null>(),
  /** Tells that the Knowledge view is open or closed: the counts are read every 4 s while it is, every 30 s otherwise. */
  knowledge_changes_watch: cmd<{ watching: boolean }, null>(),
  /** Lets the user choose documents in a native dialog and checks them. Null when cancelled. `multiple: false` lets the dialog take one file only. */
  knowledge_pick_files: cmd<{ multiple?: boolean } | undefined, KnowledgeIntake | null>(),
  /** Lets the user choose a folder of documents in a native dialog and checks it. Null when cancelled. */
  knowledge_pick_folder: cmd<undefined, KnowledgeIntake | null>(),
  /** The files and folders of the last drop on the window, checked, once. Null when nothing was dropped since. */
  knowledge_drop_intake: cmd<undefined, KnowledgeIntake | null>(),
  /**
   * Imports the files and folders of the intake `token`, linking every document to `options.links` (none: global). Each event goes to `onEvent`
   * as it happens, replacing the document of `options.replacing` for each of the files it names (needs `docsSimilar`). `invalidInput` while another import runs, `notFound` for a token that is gone, `unsupported` for a server without `import-ndjson`.
   */
  knowledge_import: cmd<{ token: string; options: KnowledgeImportOptions | null; onEvent: EventSink<KnowledgeImportEvent> }, KnowledgeImportResult>(),
  /** Asks the running import to stop between two documents. False when no import runs. */
  knowledge_import_cancel: cmd<undefined, boolean>(),
  /**
   * The stored documents the files of the intake `token` are probably versions of, one entry per file (a folder gives its files). A title only matches
   * inside the scope of `links` (the first link's repository, else global). `notFound` for a token that is gone, `unsupported` for a server without `docs-similar`.
   */
  knowledge_similar: cmd<{ token: string; links: KnowledgeLink[] | null }, KnowledgeSimilar>(),
};
