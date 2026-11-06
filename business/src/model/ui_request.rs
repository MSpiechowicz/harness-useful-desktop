//! Dialogs the agent asks the user to answer (extension UI requests, tool approvals) and the
//! webview's answers.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::agent_action::MAX_ACTION_TEXT_BYTES;
use crate::error::{BusinessError, Result};

/// A dialog requested by the agent; answered with a [`UiResponse`] carrying the same `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UiRequest {
    /// Request id chosen by the agent.
    pub id: String,
    /// What to show.
    pub kind: UiRequestKind,
    /// What the dialog is for, when the agent says so. Without it (OMP, PI) the UI tells an
    /// approval by its option labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub purpose: Option<UiPurpose>,
    /// The tool call an approval is about, as named in its `toolStart`, when the agent says which
    /// (OpenCode). Without it the UI matches open calls by the tool the title names and never picks among several.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tool_call_id: Option<String>,
    /// An input whose answer must not show on screen (OMP asks for a secret).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub secret: Option<bool>,
}

impl UiRequest {
    /// A dialog that says nothing about its purpose or tool call.
    pub fn new(id: impl Into<String>, kind: UiRequestKind) -> Self {
        Self {
            id: id.into(),
            kind,
            purpose: None,
            tool_call_id: None,
            secret: None,
        }
    }
}

/// What a [`UiRequest`] is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UiPurpose {
    /// Whether a tool may run: shown as an approval card.
    Approval,
    /// A question the agent asks: always a dialog, never an approval card, whatever its options say.
    Question,
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
    /// Every question of one call at once, answered together, so an answer can still be changed
    /// until the last one is submitted.
    Questions {
        /// The questions in order.
        questions: Vec<AskQuestion>,
        /// The agent gives up after this many milliseconds.
        #[ts(type = "number | null")]
        timeout_ms: Option<u64>,
    },
}

/// One question of [`UiRequestKind::Questions`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AskQuestion {
    /// Sent back with its answer.
    pub id: String,
    /// A short label for the question, which OMP's ratchet approvals use for their stage.
    pub header: Option<String>,
    /// What is asked.
    pub question: String,
    /// The options to pick from.
    pub options: Vec<AskOption>,
    /// More than one option may be picked.
    pub multi: bool,
    /// A typed answer is taken besides the options. A question without options is always typed.
    pub custom: bool,
    /// What a typed answer has to be.
    pub input: AskInput,
    /// Index of the option the agent recommends.
    #[ts(type = "number | null")]
    pub recommended: Option<u64>,
}

/// What a typed answer to an [`AskQuestion`] has to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AskInput {
    /// Any text.
    Text,
    /// A number.
    Number,
    /// A whole number.
    Integer,
}

/// One option of an [`AskQuestion`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AskOption {
    /// The label, sent back exactly as it came.
    pub label: String,
    /// Secondary text.
    pub description: Option<String>,
}

/// The answer to one [`AskQuestion`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AskAnswer {
    /// The id of the question.
    pub id: String,
    /// The labels picked.
    pub selected_options: Vec<String>,
    /// The text the user typed instead of, or for a multi-choice question beside, the labels.
    pub custom_input: Option<String>,
}

impl AskQuestion {
    /// A question as `id`, with the options as `(label, description)`.
    pub fn new(id: impl Into<String>, question: impl Into<String>, options: Vec<AskOption>, multi: bool) -> Self {
        Self {
            id: id.into(),
            header: None,
            question: question.into(),
            options,
            multi,
            custom: true,
            input: AskInput::Text,
            recommended: None,
        }
    }
}

impl AskAnswer {
    /// The typed text, trimmed; `None` when blank.
    pub fn typed(&self) -> Option<&str> {
        self.custom_input
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
    }

    /// Fails unless the labels are among `labels`, a single choice holds at most one answer, and
    /// typed text comes only where it is taken: with `custom`, or when there is nothing to pick.
    pub fn check(&self, labels: &[&str], multi: bool, custom: bool) -> Result<()> {
        if self.typed().is_some() && !custom && !labels.is_empty() {
            return Err(BusinessError::InvalidInput(
                "this question takes no typed answer".to_owned(),
            ));
        }

        if let Some(label) = self
            .selected_options
            .iter()
            .find(|label| !labels.contains(&label.as_str()))
        {
            return Err(BusinessError::InvalidInput(format!(
                "{label} is not one of the options"
            )));
        }

        if !multi && self.selected_options.len() + usize::from(self.typed().is_some()) > 1 {
            return Err(BusinessError::InvalidInput("a single choice has one answer".to_owned()));
        }

        Ok(())
    }
}

/// `answers` paired with the `count` questions whose ids are their positions; fails on anything else.
pub fn answers_by_position(answers: &[AskAnswer], count: usize) -> Result<impl Iterator<Item = (usize, &AskAnswer)>> {
    let in_order = answers.len() == count
        && answers
            .iter()
            .enumerate()
            .all(|(index, answer)| answer.id == index.to_string());

    if !in_order {
        return Err(BusinessError::InvalidInput(
            "the answers do not match the questions".to_owned(),
        ));
    }

    Ok(answers.iter().enumerate())
}

impl UiOutcome {
    /// Fails when the answer holds more text than an agent may be sent.
    pub fn check_size(&self) -> Result<()> {
        let size = match self {
            Self::Value { value } => value.len(),
            Self::Answers { answers } => answers
                .iter()
                .map(|answer| {
                    let labels: usize = answer.selected_options.iter().map(String::len).sum();

                    labels + answer.custom_input.as_deref().map_or(0, str::len)
                })
                .sum(),
            Self::Confirmed { .. } | Self::Cancelled => 0,
        };

        if size > MAX_ACTION_TEXT_BYTES {
            return Err(BusinessError::InvalidInput(format!(
                "value exceeds {MAX_ACTION_TEXT_BYTES} bytes"
            )));
        }

        Ok(())
    }
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
    /// Answer to `questions`, one per question in order.
    Answers {
        /// The answers.
        answers: Vec<AskAnswer>,
    },
    /// The user dismissed the dialog.
    Cancelled,
}
