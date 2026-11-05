//! The documents agents proposed through `knowledge_add`, waiting for the user's review in this
//! computer's queue (`knowledge_held_list`, `knowledge_held_get`, `knowledge_held_decide`,
//! `knowledge_changes_listen`), through `harness-knowledge held list|get|approve|reject`, and the
//! user's own search (`knowledge_search`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{KnowledgeAvailability, KnowledgeEmbedding, KnowledgeId, KnowledgeLink, lenient_name};

/// Where a proposal stands in the review queue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeHeldStatus {
    /// It waits for the user's review.
    #[default]
    Waiting,
    /// The user asked its agent for changes, with a comment: it waits for the revised proposal.
    ChangesRequested,
}

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
    /// The git `user.name` of the person the proposal is written for (its `addedBy`), none when no
    /// name was known and from a server that predates it.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub added_by: Option<String>,
    /// Whether it waits for review or for its agent to revise it (`waiting` when absent).
    #[serde(default)]
    #[ts(optional, as = "Option<KnowledgeHeldStatus>")]
    pub status: KnowledgeHeldStatus,
    /// What the user asked for, while the status is `changesRequested`.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub comment: Option<String>,
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
    /// The new version of a live document was stored as its pending version, which replaces the
    /// live one once it is approved.
    Pending,
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
        /// Team approval is on, so it was stored as a draft (a new document) and agents do not see
        /// it yet. Absent from a server without `docs-drafts`.
        #[ts(optional, as = "Option<bool>")]
        draft: bool,
    },
    /// It was thrown away.
    Rejected,
    /// It was marked `changesRequested` with the user's comment (the item stays in the queue).
    ChangesRequested,
    /// Requesting changes needs a comment (`commentRequired`).
    CommentRequired {
        /// The server's sentence (redacted).
        message: String,
    },
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
    /// The proposal links to a folder without a git remote, so it cannot go to the team place
    /// (`localPath`). It waits: unlink the folder or keep it on this computer.
    LocalPath {
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
    /// The remote database cannot be reached: the counts are this computer's, and writes to the
    /// remote are paused.
    #[serde(default)]
    pub offline: bool,
    /// Why it cannot be reached, as the server says it, when it says.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub offline_reason: Option<String>,
    /// How many documents wait for THIS person's approval in every place (the rail badge): the ones
    /// they did not write and did not ask for. `0` from a server that does not say who could
    /// approve (so nobody is badged for their own documents), none from a server without
    /// `docs-drafts`.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub waiting: Option<u32>,
    /// How many documents wait for approval in every place, whoever could approve them (the
    /// Waiting filter's total), none from a server without `docs-drafts`.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub waiting_all: Option<u32>,
    /// Whether team approval is on, none with no team database, while it cannot be reached and from
    /// a server without `docs-approval`.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub approval: Option<bool>,
    /// The database was moved to a newer schema by this computer's update: tell the user to restart
    /// their harnesses, once (`knowledge_restart_notice_dismiss` ends it).
    #[serde(default)]
    #[ts(optional = nullable)]
    pub restart_harnesses: Option<bool>,
    /// The database needs an upgrade a read-only role cannot make: it waits for a teammate's update.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub upgrade_waits: Option<bool>,
    /// The git `user.name` of the person at the terminal, as the server reads it: what the view
    /// compares authors with to tell their own documents. None from a server that predates it or
    /// while no name is set.
    #[serde(default, with = "lenient_name")]
    #[ts(optional = nullable, as = "Option<String>")]
    pub me: Option<String>,
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
