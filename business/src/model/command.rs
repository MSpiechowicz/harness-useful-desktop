use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where a slash command comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CommandSource {
    /// Built into the harness.
    Builtin,
    /// A desktop-provided command shared by every harness (e.g. `new`, `model`).
    Shared,
    /// A skill exposed as a command.
    Skill,
    /// Registered by an extension module or plugin.
    Extension,
    /// A custom command bundled with the harness or configured by the user (e.g. OMP `/review`).
    Custom,
    /// A Markdown prompt template.
    Prompt,
    /// A prompt published by an MCP server.
    McpPrompt,
    /// A command file discovered on disk.
    File,
}

/// Agent RPC commands that a slash command can map to.
///
/// Only names the action; the typed request with parameters arrives with the agent contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AgentActionKind {
    /// Start a fresh session.
    NewSession,
    /// Compact the conversation context.
    Compact,
    /// Rename the session (takes the command argument as the name).
    SetSessionName,
    /// Duplicate the session.
    Clone,
    /// Export the session as HTML.
    ExportHtml,
    /// Show token and cost statistics.
    GetSessionStats,
    /// Copy the last assistant reply (the UI writes it to the clipboard).
    GetLastAssistantText,
    /// Stop the running turn.
    Abort,
}

/// A desktop view that replaces a terminal-only slash command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GuiTarget {
    /// Model picker.
    ModelPicker,
    /// Thinking-level picker.
    ThinkingPicker,
    /// Session branch tree.
    SessionTree,
    /// Pick an entry to fork from.
    ForkPicker,
    /// Pick a session to resume.
    ResumePicker,
    /// Settings view.
    Settings,
    /// Approval-mode override.
    ApprovalMode,
}

/// How the desktop runs a slash command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum CommandInvocation {
    /// Send `/name args` as prompt text; the harness expands and runs it.
    PromptText,
    /// Send the mapped agent RPC command instead of text.
    Rpc {
        /// The RPC command to send.
        action: AgentActionKind,
    },
    /// Open a desktop view instead of sending anything.
    Gui {
        /// The view to open.
        target: GuiTarget,
    },
    /// Not available in the desktop app.
    Unsupported {
        /// Why, and what to do instead.
        reason: String,
    },
}

/// One entry of the slash-command catalog used for autocomplete and the command palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandSpec {
    /// Command name without the leading slash (e.g. `"compact"`, `"skill:review"`).
    pub name: String,
    /// One-line description.
    pub description: String,
    /// Placeholder for arguments (e.g. `"<name>"`).
    pub argument_hint: Option<String>,
    /// Where the command comes from.
    pub source: CommandSource,
    /// How the desktop runs it.
    pub invocation: CommandInvocation,
}
