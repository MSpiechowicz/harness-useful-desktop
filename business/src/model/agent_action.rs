//! What the webview may ask a running agent to do (`agent_send`) and the typed replies.
//!
//! [`AgentAction`] is a closed whitelist: the webview never sends raw RPC JSON, and each harness
//! dialect translates an action into its own command.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ChatMessage, CommandSpec, ModelOption, SessionRef, SessionState, SessionStats};
use crate::error::{BusinessError, Result};

/// Largest accepted `message`, `instructions` or `name` text, in bytes (512 KiB).
pub const MAX_ACTION_TEXT_BYTES: usize = 512 * 1024;

/// Largest accepted `getHistory` page size.
pub const MAX_HISTORY_PAGE: u32 = 200;

/// How a prompt sent while the agent is streaming is delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StreamingBehavior {
    /// Interrupt the current turn after the running tool call.
    Steer,
    /// Deliver after the agent finishes the current turn.
    FollowUp,
}

/// An action sent to a running agent session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum AgentAction {
    /// Send a user prompt (slash commands included, where the harness runs them as text).
    Prompt {
        /// Prompt text.
        message: String,
        /// Delivery while the agent is streaming; `None` lets the harness decide.
        streaming_behavior: Option<StreamingBehavior>,
    },
    /// Steer the running turn.
    Steer {
        /// Steering text.
        message: String,
    },
    /// Queue a message for after the running turn.
    FollowUp {
        /// Follow-up text.
        message: String,
    },
    /// Stop the running turn.
    Abort,
    /// Start a fresh session in the same process.
    NewSession,
    /// Switch to another session of the same harness.
    SwitchSession {
        /// The session to switch to.
        session: SessionRef,
    },
    /// Fork the session from an entry.
    Fork {
        /// Session entry id to fork from (see `ChatMessage.id`).
        entry_id: String,
    },
    /// Duplicate the session (PI).
    Clone,
    /// Switch model.
    SetModel {
        /// Provider id.
        provider: String,
        /// Model id within the provider.
        model_id: String,
    },
    /// Change the thinking level.
    SetThinkingLevel {
        /// One of the levels from `getAvailableThinkingLevels`.
        level: String,
    },
    /// Compact the context.
    Compact {
        /// Extra instructions for the summary.
        instructions: Option<String>,
    },
    /// Rename the session.
    SetSessionName {
        /// New name.
        name: String,
    },
    /// Read the session state.
    GetState,
    /// List selectable models.
    GetAvailableModels,
    /// List thinking levels of the active model.
    GetAvailableThinkingLevels,
    /// Read token and cost statistics.
    GetSessionStats,
    /// Export the session as HTML.
    ExportHtml,
    /// Read the last assistant reply (for copying).
    GetLastAssistantText,
    /// Re-read the slash-command catalog.
    RefreshCommands,
    /// Read a page of the message history (newest page first).
    GetHistory {
        /// Cursor from the previous page's `nextCursor`; `None` for the first page.
        cursor: Option<String>,
        /// Page size, 1 to [`MAX_HISTORY_PAGE`].
        limit: u32,
    },
}

impl AgentAction {
    /// Checks the size limits of the action's free-text fields and the history page size.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Prompt { message, .. } | Self::Steer { message } | Self::FollowUp { message } => {
                check_text("message", message)
            }
            Self::Compact {
                instructions: Some(instructions),
            } => check_text("instructions", instructions),
            Self::SetSessionName { name } => check_text("name", name),
            Self::GetHistory { limit, .. } => check_history_limit(*limit),
            _ => Ok(()),
        }
    }
}

fn check_text(field: &str, text: &str) -> Result<()> {
    if text.len() > MAX_ACTION_TEXT_BYTES {
        return Err(BusinessError::InvalidInput(format!(
            "{field} exceeds {MAX_ACTION_TEXT_BYTES} bytes"
        )));
    }

    Ok(())
}

fn check_history_limit(limit: u32) -> Result<()> {
    if limit == 0 || limit > MAX_HISTORY_PAGE {
        return Err(BusinessError::InvalidInput(format!(
            "history limit must be between 1 and {MAX_HISTORY_PAGE}"
        )));
    }

    Ok(())
}

/// The reply to an [`AgentAction`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum AgentReply {
    /// Accepted; results (if any) arrive as events.
    Ack,
    /// A prompt was accepted.
    Prompt {
        /// OMP: whether the model was invoked (false for slash commands handled locally).
        agent_invoked: Option<bool>,
        /// PI: `started`, `queued` or `handled`.
        disposition: Option<String>,
    },
    /// Answer to `getState`.
    State {
        /// The session state.
        state: SessionState,
    },
    /// Answer to `getAvailableModels`.
    Models {
        /// Selectable models.
        models: Vec<ModelOption>,
    },
    /// Answer to `getAvailableThinkingLevels`.
    ThinkingLevels {
        /// Level names (e.g. `["off", "low", "medium", "high"]`).
        levels: Vec<String>,
    },
    /// Answer to `getSessionStats`.
    Stats {
        /// The statistics.
        stats: SessionStats,
    },
    /// Answer to `exportHtml`.
    Exported {
        /// Absolute path of the written HTML file.
        path: String,
    },
    /// Answer to `getLastAssistantText`.
    Text {
        /// The text; `None` when there is no assistant reply yet.
        text: Option<String>,
    },
    /// Answer to `getHistory`.
    History {
        /// Messages of this page, oldest first.
        messages: Vec<ChatMessage>,
        /// Cursor of the next (older) page; `None` on the last page.
        next_cursor: Option<String>,
        /// Total number of messages, when known.
        total: Option<u32>,
    },
    /// Answer to `refreshCommands`.
    Commands {
        /// The merged slash-command catalog.
        commands: Vec<CommandSpec>,
    },
}
