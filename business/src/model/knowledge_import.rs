//! Importing documents (`knowledge_pick_files`, `knowledge_pick_folder`, `knowledge_drop_intake`,
//! `knowledge_import`, `knowledge_import_cancel`): what the user picked or dropped, and the events
//! `harness-knowledge docs import --ndjson` prints while it works.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{KnowledgeId, KnowledgeLink, KnowledgePlace};

/// A file or folder the user picked or dropped, as the shell checked it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeIntakeEntry {
    /// The name without its directory.
    pub name: String,
    /// The absolute path (only shown, the import names the intake by its token).
    pub path: String,
    /// The size of a file in bytes, none for a folder.
    #[ts(type = "number | null")]
    pub bytes: Option<u64>,
}

/// Why an item of an intake is left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeSkipReason {
    /// It does not exist (any more).
    Missing,
    /// It is a symbolic link.
    Symlink,
    /// It is neither a regular file nor a folder.
    NotFileOrFolder,
    /// It cannot be read.
    Unreadable,
    /// The intake already holds the most files an import takes.
    TooMany,
}

/// An item left out of an intake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeSkipped {
    /// The name without its directory.
    pub name: String,
    /// Why.
    pub reason: KnowledgeSkipReason,
}

/// The files and folders the user picked or dropped, ready to import. The shell keeps their paths:
/// `knowledge_import` takes the `token` and nothing else names a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeIntake {
    /// What to give `knowledge_import`.
    pub token: String,
    /// The files that will be imported.
    pub files: Vec<KnowledgeIntakeEntry>,
    /// The folders that will be walked (the server skips symlinks, hidden entries, `.git`,
    /// `node_modules` and `target`, and takes at most 5000 files in all).
    pub folders: Vec<KnowledgeIntakeEntry>,
    /// What was left out and why.
    pub skipped: Vec<KnowledgeSkipped>,
}

/// What became of one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeFileStatus {
    /// A new document.
    Added,
    /// An existing document with new content.
    Updated,
    /// Nothing changed, so nothing was embedded again.
    Unchanged,
    /// Not documentation, with a reason.
    Skipped,
    /// It could not be read, with a reason.
    Failed,
}

/// Whether the chunks of a file got their vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeEmbedding {
    /// They have vectors.
    Done,
    /// No embedder was available, so they wait for a reindex.
    Pending,
}

/// How many files ended each way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeImportCounts {
    /// New documents.
    pub added: u32,
    /// Changed documents.
    pub updated: u32,
    /// Documents that stayed as they were.
    pub unchanged: u32,
    /// Files that are no documentation.
    pub skipped: u32,
    /// Files that failed.
    pub failed: u32,
}

/// One event of an import, in the order the server prints them: `plan`, then a `file` and a
/// `progress` after each file, then `done`. `failed` ends an import that could not run at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum KnowledgeImportEvent {
    /// How many files the import looks at.
    Plan {
        /// The number of files.
        total: u32,
    },
    /// A file is done.
    File {
        /// The file's path as the server saw it.
        path: String,
        /// What became of it.
        status: KnowledgeFileStatus,
        /// Why it was skipped or failed.
        reason: Option<String>,
        /// Its document, when it has one.
        doc_id: Option<KnowledgeId>,
        /// How many chunks it has.
        chunks: Option<u32>,
        /// Whether its chunks got their vectors.
        embedding: Option<KnowledgeEmbedding>,
        /// What the extraction noticed.
        warnings: Vec<String>,
    },
    /// How far the import is.
    Progress {
        /// Files done.
        done: u32,
        /// Files in all.
        total: u32,
    },
    /// The import ended.
    Done {
        /// How many files ended each way.
        counts: KnowledgeImportCounts,
        /// True when it was cancelled: the files before the cancel are kept.
        cancelled: bool,
        /// What counted the tokens (`tokenizer` or `approximate`).
        counter: String,
        /// How many chunks have no vector yet.
        embedding_pending: u32,
    },
    /// The import could not run or ended without its closing event.
    Failed {
        /// Why (redacted).
        message: String,
    },
}

/// A file of the selection that replaces a stored document in place (`replacing`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeReplacement {
    /// The file's path, one of the selection's files as `knowledge_similar` listed it.
    pub path: String,
    /// The document it replaces, whose place decides the file's place.
    pub id: KnowledgeId,
}

/// What `knowledge_import` asks for beyond the intake: the repositories every imported document
/// links to, and the document the one file replaces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeImportOptions {
    /// The links of every imported document, besides the ones it already has (none: global).
    #[serde(default)]
    pub links: Vec<KnowledgeLink>,
    /// The document the one file of this import replaces in place, whatever its source.
    pub replaces: Option<KnowledgeId>,
    /// The documents several files of this import replace in place, each file one document. Needs
    /// `docs-similar`, and is not combined with `replaces`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replacing: Vec<KnowledgeReplacement>,
    /// Where the documents are kept when both places are in use (the routing's default for
    /// their kind when not given).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub place: Option<KnowledgePlace>,
}

/// How a finished import ended, for the command's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeImportResult {
    /// The counts of the closing event, none when the import ended without one.
    pub counts: Option<KnowledgeImportCounts>,
    /// True when it was cancelled.
    pub cancelled: bool,
}

/// What `knowledge_reindex` did: the chunks without a vector that got one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeReindexed {
    /// How many documents it looked at.
    pub documents: u32,
    /// How many chunks it embedded.
    pub embedded: u32,
    /// How many documents it could not embed.
    pub failed: u32,
    /// True when it was stopped before it finished.
    pub cancelled: bool,
    /// The model that embedded them.
    pub model: Option<String>,
}
