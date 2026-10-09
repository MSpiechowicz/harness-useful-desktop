//! Dialogs the agent asks the user to answer (extension UI requests, tool approvals) and the
//! webview's answers.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A dialog requested by the agent; answered with a [`UiResponse`] carrying the same `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UiRequest {
    /// Request id chosen by the agent.
    pub id: String,
    /// What to show.
    pub kind: UiRequestKind,
}

/// The dialog to show for a [`UiRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum UiRequestKind {
    /// Pick one option (also used for tool approvals, e.g. `Approve` / `Deny`).
    Select {
        /// Question shown above the options.
        title: String,
        /// Option labels; the chosen label is sent back as the value.
        options: Vec<String>,
        /// Optional secondary text per option (same length as `options`).
        details: Vec<Option<String>>,
        /// The agent gives up after this many milliseconds.
        #[ts(type = "number | null")]
        timeout_ms: Option<u64>,
    },
    /// Yes/no question.
    Confirm {
        /// Dialog title.
        title: String,
        /// Question text.
        message: String,
        /// The agent gives up after this many milliseconds.
        #[ts(type = "number | null")]
        timeout_ms: Option<u64>,
    },
    /// Single-line text input.
    Input {
        /// Prompt shown above the field.
        title: String,
        /// Placeholder text.
        placeholder: Option<String>,
        /// The agent gives up after this many milliseconds.
        #[ts(type = "number | null")]
        timeout_ms: Option<u64>,
    },
    /// Multi-line text editor.
    Editor {
        /// Prompt shown above the editor.
        title: String,
        /// Initial editor content.
        prefill: Option<String>,
    },
}

/// The webview's answer to a [`UiRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UiResponse {
    /// The id of the request being answered.
    pub id: String,
    /// The answer.
    pub outcome: UiOutcome,
}

/// What the user answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum UiOutcome {
    /// Answer to `select`, `input` or `editor`.
    Value {
        /// The chosen option label or the entered text.
        value: String,
    },
    /// Answer to `confirm`.
    Confirmed {
        /// True for yes.
        confirmed: bool,
    },
    /// The user dismissed the dialog.
    Cancelled,
}
