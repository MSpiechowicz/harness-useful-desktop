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

/// Most images that can go with one message.
pub const MAX_ACTION_IMAGES: usize = 8;

/// Largest total base64 size of the images of one message, in bytes (3 MiB). One RPC frame holds
/// 4 MiB, and the message text and the JSON around the images need room too.
pub const MAX_ACTION_IMAGE_BYTES: usize = 3 * 1024 * 1024;

/// MIME types of images the harnesses accept.
pub const IMAGE_MIME_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// An image attached to a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImageAttachment {
    /// Base64-encoded image bytes (no `data:` prefix).
    pub data: String,
    /// One of [`IMAGE_MIME_TYPES`].
    pub mime_type: String,
}

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
        /// Images the agent should look at.
        #[serde(default)]
        images: Vec<ImageAttachment>,
        /// Delivery while the agent is streaming; `None` lets the harness decide.
        streaming_behavior: Option<StreamingBehavior>,
    },
    /// Steer the running turn.
    Steer {
        /// Steering text.
        message: String,
        /// Images the agent should look at.
        #[serde(default)]
        images: Vec<ImageAttachment>,
    },
    /// Queue a message for after the running turn.
    FollowUp {
        /// Follow-up text.
        message: String,
        /// Images the agent should look at.
        #[serde(default)]
        images: Vec<ImageAttachment>,
    },
    /// Withdraw one message still waiting in the steering or follow-up queue (OMP).
    RemoveQueued {
        /// The queued text exactly as the last `queueUpdate` listed it.
        message: String,
        /// The queue it waits in.
        queue: StreamingBehavior,
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
            Self::Prompt { message, images, .. }
            | Self::Steer { message, images }
            | Self::FollowUp { message, images } => {
                check_text("message", message)?;
                check_images(images)
            }
            Self::RemoveQueued { message, .. } => check_text("message", message),
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

fn check_images(images: &[ImageAttachment]) -> Result<()> {
    if images.len() > MAX_ACTION_IMAGES {
        return Err(BusinessError::InvalidInput(format!(
            "a message holds at most {MAX_ACTION_IMAGES} images"
        )));
    }

    if let Some(image) = images
        .iter()
        .find(|image| !IMAGE_MIME_TYPES.contains(&image.mime_type.as_str()))
    {
        return Err(BusinessError::InvalidInput(format!(
            "unsupported image type {}",
            image.mime_type
        )));
    }

    let bytes: usize = images.iter().map(|image| image.data.len()).sum();
    if bytes > MAX_ACTION_IMAGE_BYTES {
        return Err(BusinessError::InvalidInput(format!(
            "images exceed {MAX_ACTION_IMAGE_BYTES} bytes"
        )));
    }

    if images
        .iter()
        .any(|image| image.data.is_empty() || !image.data.bytes().all(is_base64))
    {
        return Err(BusinessError::InvalidInput("an image is not valid base64".to_owned()));
    }

    Ok(())
}

fn is_base64(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')
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
    /// Answer to `removeQueued`.
    QueueRemoval {
        /// False when the message had already left the queue.
        removed: bool,
    },
}
