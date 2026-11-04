//! The stored comments of a review (`knowledge_doc_comments`), what answers a reply, a resolve or a
//! delete, and the lines a comment quotes. Read through `harness-knowledge docs comments|comment`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{CommentSide, KnowledgeId, lenient_name};

/// A stored comment: an overall one of a review (no section), one on lines of a section, or a reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeComment {
    /// The comment's id, to reply to it or resolve it.
    pub id: KnowledgeId,
    /// The review it came with, none for a reply.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub review_id: Option<KnowledgeId>,
    /// The section it is on, none for an overall comment or a reply.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub section: Option<String>,
    /// Which of the sections that share the heading path `section` it is on (from 1), none for an
    /// overall comment or a reply (and for a comment an older server stored).
    #[serde(default)]
    #[ts(optional, type = "number | null")]
    pub occurrence: Option<u32>,
    /// The side of the diff it is on.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub side: Option<CommentSide>,
    /// The first line it is on.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub from: Option<u32>,
    /// The last line it is on.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub to: Option<u32>,
    /// The comment's text (untrusted: shown as plain text).
    pub body: String,
    /// Who wrote it.
    #[serde(default, with = "lenient_name")]
    #[ts(optional = nullable, as = "Option<String>")]
    pub by: Option<String>,
    /// When (RFC 3339).
    #[serde(default)]
    pub at: String,
    /// The comment it answers, none for a comment that starts a thread.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub parent_id: Option<KnowledgeId>,
    /// When it was resolved (RFC 3339), none while open.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub resolved_at: Option<String>,
    /// Who resolved it.
    #[serde(default, with = "lenient_name")]
    #[ts(optional = nullable, as = "Option<String>")]
    pub resolved_by: Option<String>,
    /// The revision of the document it was written on.
    #[serde(default)]
    #[ts(type = "number")]
    pub revision: i64,
    /// The document changed since: the lines it names may not be the same any more.
    #[serde(default)]
    pub outdated: bool,
    /// The lines it names as they were when it was written. None for an overall comment, a reply,
    /// a comment older than the server's quotes and a server that does not print them.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub quote: Option<Vec<KnowledgeQuoteLine>>,
}

/// What a quoted line was when a comment was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeQuoteKind {
    /// Added by the version.
    Added,
    /// Removed by the version.
    Removed,
    /// Left as it was.
    Same,
}

/// One line a comment quotes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeQuoteLine {
    /// Its number on the side of the comment.
    #[ts(type = "number")]
    pub line: u32,
    /// What it was.
    pub kind: KnowledgeQuoteKind,
    /// Its text (untrusted: shown as plain text).
    pub text: String,
}

/// What a delete answered: the comments that are gone, the one asked for and, when it started a
/// thread, its replies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeCommentsDeleted {
    /// Their ids, as the server printed them.
    pub deleted: Vec<KnowledgeId>,
}

/// What a reply, a resolve or an unresolve answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeCommentChanged {
    /// The comment as it is now, when the server printed it.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub comment: Option<KnowledgeComment>,
}
