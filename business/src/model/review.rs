//! Diff review comments turned into an agent prompt (`review_compose`).
//!
//! The webview sends the composed `prompt` itself with `agent_send` (as a prompt or follow-up).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Maximum number of comments in one review.
pub const MAX_REVIEW_COMMENTS: usize = 100;

/// Maximum size of a comment body or the overall note.
pub const MAX_REVIEW_TEXT_BYTES: usize = 8 * 1024;

/// Maximum number of snippet lines per comment.
pub const MAX_REVIEW_SNIPPET_LINES: usize = 40;

/// Maximum size of a composed review prompt; larger reviews are rejected as invalid input.
pub const MAX_REVIEW_PROMPT_BYTES: usize = 512 * 1024;

/// Which side of a diff a comment refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CommentSide {
    /// The old file (removed lines; `startLine`/`endLine` are old line numbers).
    Old,
    /// The new file (added and context lines; `startLine`/`endLine` are new line numbers).
    New,
}

/// A comment on a line range of a file diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewComment {
    /// Project-relative path of the file.
    pub path: String,
    /// Which side the line numbers refer to.
    pub side: CommentSide,
    /// First commented line (1-based).
    pub start_line: u32,
    /// Last commented line (inclusive; equal to `startLine` for a single line).
    pub end_line: u32,
    /// The comment, at most [`MAX_REVIEW_TEXT_BYTES`].
    pub body: String,
    /// The commented lines, at most [`MAX_REVIEW_SNIPPET_LINES`].
    pub snippet: Vec<String>,
}

/// Arguments of `review_compose`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewRequest {
    /// Comments in display order, at most [`MAX_REVIEW_COMMENTS`].
    pub comments: Vec<ReviewComment>,
    /// Overall instruction placed before the comments, at most [`MAX_REVIEW_TEXT_BYTES`].
    pub note: Option<String>,
}

/// Result of `review_compose`: the deterministic Markdown prompt for the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComposedReview {
    /// The Markdown prompt, at most [`MAX_REVIEW_PROMPT_BYTES`].
    pub prompt: String,
    /// Size of `prompt` in UTF-8 bytes.
    pub bytes: u32,
    /// Number of distinct files commented on.
    pub file_count: u32,
    /// Number of comments.
    pub comment_count: u32,
}
