//! The Knowledge view's documents (`knowledge_docs_list`, `knowledge_doc_get`,
//! `knowledge_doc_rename`, `knowledge_doc_links_set`, `knowledge_docs_delete`), the repositories
//! they link to (`knowledge_repos_list`), the counts of the database (`knowledge_stats`) and what
//! the installed Harness Knowledge can do (`knowledge_status`).
//!
//! All of it is read through `harness-knowledge docs …`, `repos`, `stats` and `features`, never
//! from the database. Timestamps are RFC 3339 text as the server prints them.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{AuthorRef, DashboardSetup, KnowledgeProblem, ManualRemoval, lenient_author};

/// An id of a document or chunk as the server gave it: a plain number (the remote database, or the
/// only place in use) or a string `local:N` (this computer's store when both places are in use).
/// It round-trips exactly: give back what the server gave, never parse a local id into a number.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum KnowledgeId {
    /// A remote id.
    Remote(#[ts(type = "number")] i64),
    /// A local id (`local:12`, a chunk `local:340`, a composite `local:12:340`).
    Local(String),
}

impl KnowledgeId {
    /// True for a positive number or `local:N` / `local:N:M` with positive numbers.
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Remote(id) => *id >= 1,
            Self::Local(text) => text.strip_prefix("local:").is_some_and(|rest| {
                let parts: Vec<&str> = rest.split(':').collect();

                (1..=2).contains(&parts.len())
                    && parts.iter().all(|part| {
                        (1..=18).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit()) && *part != "0"
                    })
            }),
        }
    }
}

impl KnowledgeId {
    /// True for the id of a document: a positive number or `local:N`, never a chunk's.
    pub fn is_document(&self) -> bool {
        match self {
            Self::Remote(_) => self.is_valid(),
            Self::Local(text) => self.is_valid() && text.matches(':').count() == 1,
        }
    }
}

impl From<i64> for KnowledgeId {
    fn from(id: i64) -> Self {
        Self::Remote(id)
    }
}

impl std::fmt::Display for KnowledgeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Remote(id) => write!(f, "{id}"),
            Self::Local(text) => f.write_str(text),
        }
    }
}

/// Which place a document is kept in when both are in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgePlace {
    /// The team database.
    Remote,
    /// This computer's store.
    Local,
}

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
    /// The remote database is the only place in use and cannot be reached: there is nothing to
    /// show (the page says so, never an empty library).
    Unreachable,
    /// The server is upgrading its database: a neutral, passing state, not an error.
    Upgrading,
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
    /// `routing`: `[routing]` in the config, ids `local:N` and `place` fields.
    pub routing: bool,
    /// `doc-matches`: `docs matches`.
    pub doc_matches: bool,
    /// `offline-fallback`: a remote database that cannot be reached is answered from this computer
    /// (`offline` on lists and counts, `--place` on `docs list`).
    pub offline_fallback: bool,
    /// `docs-useful`: `shown`, `useful` and `lastUsefulAt` on documents, `--useful` and the two
    /// sorts by them.
    pub docs_useful: bool,
    /// `docs-similar`: `docs similar` and the import's `replacing`.
    pub docs_similar: bool,
    /// `docs-authors`: `addedBy` and `updatedBy` on documents.
    pub docs_authors: bool,
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
    pub id: KnowledgeId,
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
    /// Who first imported it. `None` when no name was known, and from a server without
    /// `docs-authors`.
    #[serde(default, with = "lenient_author")]
    #[ts(optional = nullable, as = "Option<AuthorRef>")]
    pub added_by: Option<AuthorRef>,
    /// When it last changed (RFC 3339).
    pub updated_at: String,
    /// Who last changed it. `None` as for `addedBy`.
    #[serde(default, with = "lenient_author")]
    #[ts(optional = nullable, as = "Option<AuthorRef>")]
    pub updated_by: Option<AuthorRef>,
    /// Where it came from.
    pub origin: KnowledgeOrigin,
    /// What the extraction noticed (a page without text).
    #[serde(default)]
    pub warnings: Vec<String>,
    /// The repositories it links to, none for a global document.
    #[serde(default)]
    pub links: Vec<KnowledgeLink>,
    /// Where it is kept, only when both places are in use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub place: Option<KnowledgePlace>,
    /// How often agents were shown the document. `None` from a server without `docs-useful`, and
    /// from a read-only role on a database no writer extended yet.
    #[serde(default)]
    #[ts(optional, type = "number | null")]
    pub shown: Option<i64>,
    /// How often an agent found the document useful. `None` as for `shown`.
    #[serde(default)]
    #[ts(optional, type = "number | null")]
    pub useful: Option<i64>,
    /// RFC 3339: when an agent last found the document useful. `None` when none did yet, and as
    /// `shown`.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub last_useful_at: Option<String>,
}

/// One entry of a document's outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeSection {
    /// The chunk's id.
    pub chunk_id: KnowledgeId,
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
    pub id: KnowledgeId,
    /// The document it belongs to.
    pub doc_id: KnowledgeId,
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
    /// The remote database cannot be reached, so the list holds this computer's documents only (or
    /// nothing, for a list narrowed to the remote place).
    #[serde(default)]
    pub offline: bool,
}

impl KnowledgePage {
    /// What a page holds when Harness Knowledge cannot be read.
    pub fn unavailable(availability: KnowledgeAvailability) -> Self {
        Self {
            availability,
            items: Vec::new(),
            total: 0,
            offline: false,
        }
    }

    /// What a list holds while the remote database cannot be reached and the list asked for it
    /// alone: nothing, said plainly.
    pub fn offline() -> Self {
        Self {
            availability: KnowledgeAvailability::Available,
            items: Vec::new(),
            total: 0,
            offline: true,
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
    /// The remote database cannot be reached, so the counts are this computer's.
    #[serde(default)]
    pub offline: bool,
}

impl KnowledgeReposPage {
    /// What a page holds when Harness Knowledge cannot be read.
    pub fn unavailable(availability: KnowledgeAvailability) -> Self {
        Self {
            availability,
            global: 0,
            repos: Vec::new(),
            offline: false,
        }
    }
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

/// The passages of one document that match a phrase (`knowledge_doc_matches`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeDocMatches {
    /// The matching chunks in document order, at most the limit asked.
    pub matches: Vec<KnowledgeDocMatch>,
    /// How many chunks match in all.
    pub total: u32,
}

/// One chunk that matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeDocMatch {
    /// The chunk's id.
    pub chunk_id: KnowledgeId,
    /// Its position in the document.
    pub ordinal: u32,
    /// The heading path.
    pub section: String,
    /// Up to 240 characters around the first word, on one line (untrusted).
    pub snippet: String,
}
