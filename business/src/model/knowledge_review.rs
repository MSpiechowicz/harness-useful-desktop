//! Drafts and team approval in the Knowledge view: the state a document is in, its pending version,
//! the transitions (`knowledge_doc_transition`), the review a teammate submits
//! (`knowledge_doc_review`), its comments, history, compare, download and the team approval
//! switch (`knowledge_approval_get`, `knowledge_approval_set`).
//!
//! All of it is read and written through `harness-knowledge docs ask|go-live|review|history|
//! compare|comments|comment|download` and `approval get|set`, which a server lists as `docs-drafts`
//! and `docs-approval`. A refusal the server explains with a code is an answer
//! ([`KnowledgeOutcome::Refused`]), not an error.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    AuthorRef, CommentSide, FileDiff, KnowledgeId, KnowledgePlace, lenient_author, lenient_name, lenient_names,
};

/// The state a document is in. Agents see live documents only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeDocStatus {
    /// Seen by agents. What every document is on a server without `docs-drafts`.
    #[default]
    Live,
    /// Kept to finish: not seen by agents.
    Draft,
    /// Asked for approval: not seen by agents.
    Waiting,
    /// A reviewer asked for changes: the author edits it and asks again.
    ChangesRequested,
}

/// The pending version of a live document: a replacement that is not live until it is approved
/// (or made live by its author when team approval is off).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgePending {
    /// The id of the pending row, to open it with `knowledge_doc_get`.
    pub id: KnowledgeId,
    /// Its state.
    pub status: KnowledgeDocStatus,
    /// Its revision, to give back to a transition.
    #[serde(default)]
    #[ts(type = "number")]
    pub revision: i64,
    /// Who last changed it.
    #[serde(default, with = "lenient_author")]
    #[ts(optional = nullable, as = "Option<AuthorRef>")]
    pub updated_by: Option<AuthorRef>,
    /// When it last changed (RFC 3339).
    #[serde(default)]
    pub updated_at: String,
    /// Everyone the approval and take-back rules count as an author of this version (as for
    /// [`KnowledgeDocument::authors`]). `None` from a server that does not name them.
    #[serde(default, with = "lenient_names")]
    #[ts(optional = nullable, as = "Option<Vec<String>>")]
    pub authors: Option<Vec<String>>,
}

/// The last time a reviewer asked for changes on a document that is not live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeLastChange {
    /// The reviewer's name, none when none was known.
    #[serde(default, with = "lenient_name")]
    #[ts(optional = nullable, as = "Option<String>")]
    pub by: Option<String>,
    /// When (RFC 3339).
    #[serde(default)]
    pub at: String,
    /// What they asked for.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub comment: Option<String>,
}

/// A transition of a document's review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeTransition {
    /// Draft or changes requested to waiting (`docs ask`). Needs team approval on.
    Ask,
    /// Waiting to changes requested, with a comment (`docs request-changes`). Needs approval on.
    RequestChanges,
    /// Waiting to live, by a teammate of the author (`docs approve`). Needs approval on.
    Approve,
    /// Draft, waiting or changes requested to live, by its author (`docs go-live`). Needs approval off.
    GoLive,
    /// Waiting back to draft, by its author (`docs take-back`): the ask is withdrawn. Works with
    /// approval on or off.
    TakeBack,
}

/// What a transition or a review came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeTransitioned {
    /// The document after it: the live id after a promote.
    pub id: KnowledgeId,
    /// Its state now.
    pub status: KnowledgeDocStatus,
    /// Its revision now.
    #[serde(default)]
    #[ts(type = "number")]
    pub revision: i64,
    /// A pending version replaced its live document.
    #[serde(default)]
    pub promoted: bool,
}

/// Why the server refused a review action. Every one leaves the document as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeRefusalCode {
    /// There is no such document or comment (any more).
    NotFound,
    /// The document moved on since it was shown (`revision` differs): reload it.
    Changed,
    /// The document is in a state the action does not apply to.
    WrongState,
    /// Team approval is on, so the author cannot make it live: ask for approval.
    ApprovalOn,
    /// Team approval is off, so nothing is asked or approved: make it live.
    ApprovalOff,
    /// The reviewer is the author (the same git name).
    SelfApproval,
    /// Only the one who wrote or asked for the document may take its ask back.
    NotAuthor,
    /// The reviewer has no git `user.name`.
    NoName,
    /// The role of the team database may not change it.
    ReadOnly,
    /// Requesting changes needs a comment.
    CommentRequired,
    /// The snapshot to compare with was not kept (the document was too large).
    SinceUnavailable,
    /// A team (remote) document cannot be linked to a folder without a git remote: only a `git:`
    /// project key leaves this computer.
    LocalPath,
    /// Any other refusal, with the server's sentence.
    Refused,
}

/// What a review action came to: it was carried out, or the server refused it with a code. A
/// refusal is an answer, not a failure. A remote database that cannot be reached is an
/// `unreachable` error instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum KnowledgeOutcome<T> {
    /// It was carried out.
    Done {
        /// What it answered.
        value: T,
    },
    /// The server refused it.
    Refused {
        /// Why.
        code: KnowledgeRefusalCode,
        /// The server's sentence (redacted).
        message: String,
    },
}

/// What a reviewer concludes with a review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeVerdict {
    /// Comments only: nothing changes state.
    Comment,
    /// Approves: the pending version goes live.
    Approve,
    /// Asks for changes: waiting goes to changes requested. Needs an overall comment.
    RequestChanges,
}

/// A comment on lines of one section's diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeLineComment {
    /// The section's heading path (a compare section's `path`).
    pub section: String,
    /// Which of the sections that share that heading path it is, counted from 1 (a compare
    /// section's `occurrence`). Left out, it is the first.
    #[serde(default = "first_occurrence")]
    #[ts(optional, type = "number")]
    pub occurrence: u32,
    /// Which side of the section's diff the lines are on.
    pub side: CommentSide,
    /// First line inside the section's diff (from 1).
    pub from: u32,
    /// Last line (inclusive).
    pub to: u32,
    /// The comment.
    pub body: String,
}

/// The first of the sections that share a heading path.
fn first_occurrence() -> u32 {
    1
}

/// One review: an overall comment and the comments on lines, with the verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeReviewSubmit {
    /// What the reviewer concludes.
    pub verdict: KnowledgeVerdict,
    /// The overall comment.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub body: Option<String>,
    /// The comments on lines of the diff.
    #[serde(default)]
    pub comments: Vec<KnowledgeLineComment>,
}

/// What happened to a document, in its history. Read from the server's word: `sentBack` (a server
/// before the rename) is `changesRequested`, a word this app does not know is `other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeEventKind {
    /// A draft was stored.
    Drafted,
    /// A draft or waiting document was changed.
    Edited,
    /// A pending version of a live document was stored.
    UpdateDrafted,
    /// Approval was asked.
    Asked,
    /// A reviewer asked for changes.
    ChangesRequested,
    /// A teammate approved it.
    Approved,
    /// Its author made it live.
    WentLive,
    /// Its author took the ask for approval back.
    Withdrawn,
    /// Anything a newer server records.
    Other,
}

impl<'de> Deserialize<'de> for KnowledgeEventKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let word = String::deserialize(deserializer)?;

        Ok(match word.as_str() {
            "drafted" => Self::Drafted,
            "edited" => Self::Edited,
            "updateDrafted" => Self::UpdateDrafted,
            "asked" => Self::Asked,
            "changesRequested" | "sentBack" => Self::ChangesRequested,
            "approved" => Self::Approved,
            "wentLive" => Self::WentLive,
            "withdrawn" => Self::Withdrawn,
            _ => Self::Other,
        })
    }
}

/// One event of a document's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeHistoryEvent {
    /// What happened.
    pub kind: KnowledgeEventKind,
    /// Who did it, none when no name was known.
    #[serde(default, with = "lenient_name")]
    #[ts(optional = nullable, as = "Option<String>")]
    pub by: Option<String>,
    /// When (RFC 3339).
    #[serde(default)]
    pub at: String,
    /// The comment that came with it (what was asked for).
    #[serde(default)]
    #[ts(optional = nullable)]
    pub comment: Option<String>,
}

/// A document's history, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeHistory {
    /// The newest events, at most 200.
    pub events: Vec<KnowledgeHistoryEvent>,
    /// How many events the document has in all.
    pub total: u32,
}

/// One changed section of a compare: its [`FileDiff`] (`path` is the heading path) and which of the
/// sections that share that path it is. `(path, occurrence)` is what a line comment names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeSectionDiff {
    /// The diff of the section.
    #[serde(flatten)]
    pub diff: FileDiff,
    /// Counted from 1 among the sections of the version that share `path`, the unchanged ones
    /// included.
    #[serde(default = "first_occurrence")]
    #[ts(type = "number")]
    pub occurrence: u32,
}

/// A pending version compared with its live document (or with the version that was asked): one
/// [`FileDiff`] per changed section, as the Changes view shows files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeCompare {
    /// The live document, none for a new document (every line is added).
    #[serde(default)]
    #[ts(optional = nullable)]
    pub live: Option<KnowledgeId>,
    /// The pending (or draft) row.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub pending: Option<KnowledgeId>,
    /// The revision of the row that is reviewed (`pending`), to give back as the `expect` of a
    /// review or a transition. None from a server that does not print it.
    #[serde(default)]
    #[ts(optional, type = "number | null")]
    pub revision: Option<i64>,
    /// The sections that differ, in the pending version's order. `path` is the heading path.
    pub sections: Vec<KnowledgeSectionDiff>,
    /// The texts were cut at the server's size limit.
    #[serde(default)]
    pub truncated: bool,
}

/// Where a document was saved (`knowledge_doc_download`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeDownloaded {
    /// The file that was written.
    pub path: String,
    /// True for the file as it was imported, false for the stored text written as Markdown or text.
    pub original: bool,
}

/// The team approval switch (`approval get|set`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeTeamApproval {
    /// Whether every import to the team database is a draft a teammate approves.
    pub approval: bool,
    /// Where the setting is kept: the team database, or this computer (which is always off).
    pub place: KnowledgePlace,
    /// Whether the user may change it: `false` for a read-only role, none while unknown.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub writable: Option<bool>,
    /// Who last switched it (a git `user.name`), none while the team database does not say.
    #[serde(default, with = "lenient_name")]
    #[ts(optional = nullable, as = "Option<String>")]
    pub changed_by: Option<String>,
    /// When it was last switched (RFC 3339), none while the team database does not say.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub changed_at: Option<String>,
}

/// What `upgrade` did to the database (`harness-knowledge upgrade --json`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeUpgraded {
    /// A database was moved to the newer schema: running harnesses must be restarted.
    #[serde(default)]
    pub upgraded: bool,
    /// A database could not be upgraded by this role (read-only): it waits for a teammate's update.
    #[serde(default)]
    pub waiting: bool,
}

/// What `knowledge_held_request_changes` did: the proposal was marked, and whether the comment
/// reached the chat of the agent that proposed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeChangesRequested {
    /// What the server answered (`changesRequested`, or its refusal).
    pub decided: super::KnowledgeDecided,
    /// The comment was sent to the live desktop chat of the proposal's session as a follow-up.
    pub delivered: bool,
}
