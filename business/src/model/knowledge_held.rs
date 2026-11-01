//! The documents agents proposed through `knowledge_add`, waiting for the user's review in this
//! computer's queue (`knowledge_held_list`, `knowledge_held_get`, `knowledge_held_decide`,
//! `knowledge_changes_listen`), through `harness-knowledge held list|get|approve|reject`, and the
//! user's own search (`knowledge_search`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{KnowledgeAvailability, KnowledgeEmbedding, KnowledgeId, KnowledgeLink};

/// One proposed document of the review queue, as `held list` and `held get` print it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeHeldItem {
    /// The proposal's id (a lowercase UUID), to approve or reject it.
    pub id: String,
    /// The proposed title.
    pub title: String,
    /// `markdown`, `text`, `html`, `asciidoc`, `rst` or `openapi`.
    pub format: String,
    /// The file name it is imported under (`install-notes.md`).
    pub name: String,
    /// The size of its text in bytes.
    #[ts(type = "number")]
    pub bytes: u64,
    /// The first 300 characters of its text in a list, all of it from `knowledge_held_get`.
    pub text: String,
    /// The repositories the agent proposed to link it to.
    pub links: Vec<KnowledgeLink>,
    /// The harness that proposed it (the name its MCP client gave, `unknown` without one).
    pub client: String,
    /// The session of the server that took the proposal, when it has one.
    pub session: Option<String>,
    /// The folder the proposal was about.
    pub project_path: Option<String>,
    /// The repository key of that folder, when it is in one.
    pub repo_key: Option<String>,
    /// The file the text was read from, relative to the project, when the agent named one.
    pub source_path: Option<String>,
    /// The document an approval replaces in place.
    pub replaces: Option<KnowledgeId>,
    /// What identifies this version of the proposal: give it back to a decision to decide only the
    /// version that was shown.
    pub revision: String,
    /// When it was proposed (RFC 3339).
    pub created_at: String,
}

/// One page of the review queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeHeldPage {
    /// Whether the queue could be read.
    pub availability: KnowledgeAvailability,
    /// The proposals of this page, newest first, empty unless `available`.
    pub items: Vec<KnowledgeHeldItem>,
    /// How many proposals wait in all (the badge).
    pub total: u32,
}

impl KnowledgeHeldPage {
    /// What a page holds when the queue cannot be read.
    pub fn unavailable(availability: KnowledgeAvailability) -> Self {
        Self {
            availability,
            items: Vec::new(),
            total: 0,
        }
    }
}

/// What the user decides about a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeDecision {
    /// Import it as a document.
    Approve,
    /// Throw it away.
    Reject,
}

/// What an approval did to the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeApproval {
    /// A new document.
    Added,
    /// The document it replaced, with the proposal's text.
    Updated,
    /// The same text was imported already: nothing changed but the links.
    Unchanged,
}

/// What a decision came to: it was carried out, or the server refused it. A refusal is an answer,
/// not a failure: the proposal is still in the queue unless another decision took it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum KnowledgeDecided {
    /// It was imported as a document.
    Approved {
        /// The document it became.
        doc_id: KnowledgeId,
        /// What the import did.
        status: KnowledgeApproval,
        /// The title it was imported under.
        title: String,
        /// How many chunks it was cut into, none for `unchanged` (nothing was cut again).
        chunks: Option<u32>,
        /// Whether its chunks got their vectors, none for `unchanged`.
        embedding: Option<KnowledgeEmbedding>,
    },
    /// It was thrown away.
    Rejected,
    /// There is no such proposal, or another decision took it (`notFound`).
    NotFound {
        /// The server's sentence (redacted).
        message: String,
    },
    /// The proposal is not the version that was shown (`changed`): reload it.
    Changed {
        /// The server's sentence (redacted).
        message: String,
    },
    /// The import failed and the proposal waits for another try (`refused`).
    Refused {
        /// Why (redacted).
        message: String,
    },
}

/// What `knowledge_changes_listen` sends: the database's counts now and on every change, so the
/// view reloads its list when `generation` moves and shows `held` on the review badge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgePulse {
    /// How many documents the database holds.
    pub documents: u32,
    /// Changes whenever the database changes.
    #[ts(type = "number")]
    pub generation: i64,
    /// How many proposals wait for review, none while the server has no review queue.
    pub held: Option<u32>,
}

/// Which documents a search sees besides the global ones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeSearchScope {
    /// The project's repository (`--scope repo`), the global documents only without a project.
    #[default]
    Repo,
    /// The documents of every repository (`--scope all`).
    All,
}

/// One passage a search found, as `search --json` prints it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeHit {
    /// The document it is in.
    pub doc_id: KnowledgeId,
    /// The passage's chunk, to open it with `knowledge_doc_get`.
    pub chunk_id: KnowledgeId,
    /// The document's title.
    pub title: String,
    /// The headings above the passage (`Guide > Install`), empty at the top of a document.
    pub section: String,
    /// The start of the passage around the first word that matched, on one line.
    pub snippet: String,
    /// How well it matched, the server's rounded score (higher is better).
    #[ts(type = "number")]
    pub score: i64,
}
