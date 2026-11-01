//! The Knowledge view's documents (`knowledge_docs_list`, `knowledge_doc_get`,
//! `knowledge_doc_rename`, `knowledge_doc_links_set`, `knowledge_docs_delete`), the repositories
//! they link to (`knowledge_repos_list`), the counts of the database (`knowledge_stats`) and what
//! the installed Harness Knowledge can do (`knowledge_status`).
//!
//! All of it is read through `harness-knowledge docs …`, `repos`, `stats` and `features`, never
//! from the database. Timestamps are RFC 3339 text as the server prints them.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{DashboardSetup, KnowledgeProblem, ManualRemoval, SortOrder};

/// Whether Harness Knowledge could be read, for the page of a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum KnowledgeAvailability {
    /// It answered.
    Available,
    /// Harness Knowledge is not installed.
    NotInstalled,
    /// The installed server predates the command (update it).
    Outdated,
    /// The database lacks a capability Harness Knowledge needs (exit 3), with what and the
    /// server's sentence (redacted): the settings test names it the same way.
    Missing {
        /// What is missing.
        problem: KnowledgeProblem,
        /// What the server said.
        message: String,
    },
    /// The server failed or could not be asked, with its line (redacted).
    Failed {
        /// What it said.
        message: String,
    },
}

/// What the installed Harness Knowledge lists in `harness-knowledge features`. Everything the app
/// offers for it waits for the matching feature.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeFeatures {
    /// `docs`: `docs list|get|links|rename|delete`, `repos`, `stats`, `check`.
    pub docs: bool,
    /// `import-ndjson`: `docs import --ndjson`.
    pub import: bool,
    /// `embedder`: its own local embedder (`embedder run|enable|disable|status|remove-model`).
    pub embedder: bool,
    /// `search`: `search --json`.
    pub search: bool,
    /// `recall`: `recall --hook`.
    pub recall: bool,
    /// `held`: the review queue of documents agents propose (`held list|get|approve|reject`).
    pub held: bool,
    /// `mcp`: the MCP server the harnesses run.
    pub mcp: bool,
}

/// What `knowledge_status` says: whether the server is there and what it can do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeStatus {
    /// Whether the server could be asked and how (`available` once it listed its features).
    pub availability: KnowledgeAvailability,
    /// What it lists; all false unless `available`.
    pub features: KnowledgeFeatures,
}

/// Where a document came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeOrigin {
    /// Imported by the user.
    User,
    /// Proposed by an agent and approved by the user.
    AgentApproved,
}

/// A repository a document links to, and the paths it is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeLink {
    /// The repository key (`git:github.com/acme/widget`, `path:/work/widget`).
    pub repo: String,
    /// Path globs inside it (`src/api/**`), empty for the whole repository.
    #[serde(default)]
    pub globs: Vec<String>,
}

/// One document, as the lists and the detail show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeDocument {
    /// The document's id.
    #[ts(type = "number")]
    pub id: i64,
    /// Its title.
    pub title: String,
    /// The name of the file it came from.
    pub source_name: String,
    /// What identifies it across imports (a path, a Confluence page id).
    pub source_key: String,
    /// `markdown`, `text`, `html`, `openapi`, `asciidoc`, `rst` or `pdf`.
    pub format: String,
    /// The hash of its extracted text.
    pub content_sha256: String,
    /// The hash of the file as imported.
    pub file_sha256: String,
    /// The size of its text in bytes.
    #[ts(type = "number")]
    pub bytes: u64,
    /// How many chunks it has.
    pub chunk_count: u32,
    /// When it was first imported (RFC 3339).
    pub created_at: String,
    /// When it last changed (RFC 3339).
    pub updated_at: String,
    /// Where it came from.
    pub origin: KnowledgeOrigin,
    /// What the extraction noticed (a page without text).
    #[serde(default)]
    pub warnings: Vec<String>,
    /// The repositories it links to, none for a global document.
    #[serde(default)]
    pub links: Vec<KnowledgeLink>,
}

/// One entry of a document's outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeSection {
    /// The chunk's id.
    #[ts(type = "number")]
    pub chunk_id: i64,
    /// Its position in the document, from 0.
    pub ordinal: u32,
    /// The headings above it and its own on one line (`Guide > Install`), empty before the first
    /// heading.
    pub heading_path: String,
    /// Its size in tokens.
    pub tokens: u32,
    /// Where it starts in the document's text, in characters.
    pub char_start: u32,
    /// Where it ends.
    pub char_end: u32,
}

/// One chunk of a document with its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeChunk {
    /// The chunk's id.
    #[ts(type = "number")]
    pub id: i64,
    /// The document it belongs to.
    #[ts(type = "number")]
    pub doc_id: i64,
    /// Its position in the document, from 0.
    pub ordinal: u32,
    /// The headings above it and its own on one line (`Guide > Install`), empty before the first
    /// heading.
    pub heading_path: String,
    /// Its text.
    pub text: String,
    /// Its size in tokens.
    pub tokens: u32,
    /// Where it starts in the document's text, in characters.
    pub char_start: u32,
    /// Where it ends.
    pub char_end: u32,
}

/// A document with its outline (and one chunk when asked): the slide-over's data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeDetail {
    /// The document.
    pub document: KnowledgeDocument,
    /// Its outline, in document order.
    pub sections: Vec<KnowledgeSection>,
    /// The chunk asked for with `chunkId`.
    pub chunk: Option<KnowledgeChunk>,
}

/// Which documents a list shows by their repository links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeScope {
    /// Every document.
    All,
    /// The documents linked to no repository.
    Global,
    /// The documents linked to the repository with this key.
    Repo {
        /// The repository key (see [`KnowledgeLink::repo`]).
        key: String,
    },
}

/// How a list is sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeSort {
    /// By title, A to Z unless reversed.
    Title,
    /// By the last change, newest first unless reversed.
    Updated,
    /// By size, largest first unless reversed.
    Size,
}

/// What a list of documents is asked with: the filters and the page (see
/// [`SortOrder`] for the direction).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeListFilter {
    /// By repository (every document unless given).
    pub scope: Option<KnowledgeScope>,
    /// By format (`markdown`, `pdf`, ...).
    pub format: Option<String>,
    /// The sort (`updated` unless given).
    pub sort: Option<KnowledgeSort>,
    /// The direction (the sort's own unless given).
    pub order: Option<SortOrder>,
}

/// One page of documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgePage {
    /// Whether Harness Knowledge could be read (see [`KnowledgeAvailability`]).
    pub availability: KnowledgeAvailability,
    /// The documents of this page, empty unless `available`.
    pub items: Vec<KnowledgeDocument>,
    /// How many documents match the filters, over every page.
    pub total: u32,
}

impl KnowledgePage {
    /// What a page holds when Harness Knowledge cannot be read.
    pub fn unavailable(availability: KnowledgeAvailability) -> Self {
        Self {
            availability,
            items: Vec::new(),
            total: 0,
        }
    }
}

/// A repository with documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeRepoCount {
    /// The repository key, to give back as [`KnowledgeScope::Repo`].
    pub key: String,
    /// How many documents link to it.
    pub docs: u32,
}

/// The repositories that have documents (`harness-knowledge repos --json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeReposPage {
    /// Whether Harness Knowledge could be read.
    pub availability: KnowledgeAvailability,
    /// How many documents link to no repository.
    pub global: u32,
    /// The repositories, most documents first.
    pub repos: Vec<KnowledgeRepoCount>,
}

impl KnowledgeReposPage {
    /// What a page holds when Harness Knowledge cannot be read.
    pub fn unavailable(availability: KnowledgeAvailability) -> Self {
        Self {
            availability,
            global: 0,
            repos: Vec::new(),
        }
    }
}

/// What the database holds (`harness-knowledge stats --json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeStats {
    /// How many documents.
    pub documents: u32,
    /// How many chunks.
    pub chunks: u32,
    /// How many chunks have a vector.
    pub embedded_chunks: u32,
    /// The size of all their text in bytes.
    #[ts(type = "number")]
    pub bytes: u64,
    /// How many documents link to no repository.
    pub global_documents: u32,
    /// How many repositories have documents.
    pub repositories: u32,
    /// Changes whenever the database changes (the live view reloads on it).
    #[ts(type = "number")]
    pub generation: i64,
    /// The schema version of the database.
    pub schema: u32,
    /// The model that embedded the chunks, once any were.
    pub model: Option<String>,
    /// How many proposals wait in the review queue. Absent from a server that predates it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub held: Option<u32>,
}

/// The answer of `knowledge_docs_delete`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeDeleted {
    /// How many documents were deleted (ids that did not exist count nothing).
    pub deleted: u32,
}

/// Result of the "Project knowledge" switch (`knowledge_set_enabled`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeSwitchResult {
    /// The stored `knowledgeEnabled` pref.
    pub knowledge_enabled: bool,
    /// The `harness-knowledge` binary after the switch. While it is not installed nothing is
    /// registered, and the UI offers its installer (`setup_install_knowledge`).
    pub server: DashboardSetup,
    /// When switched off: the app's own entries left in files it cannot rewrite (they have
    /// comments), which the user removes by hand. Empty otherwise.
    pub manual_removals: Vec<ManualRemoval>,
}
