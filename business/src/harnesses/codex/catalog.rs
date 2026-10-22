//! The Codex command catalog: well-known built-in slash commands of the Codex CLI 0.154.
//!
//! Chat with Codex is not available in the desktop app yet, so every command, the shared ones
//! included, is listed as unsupported with that reason.

use crate::harness::{CommandCatalog, SharedCommand, spec, unsupported};
use crate::model::{CommandInvocation, CommandSource, CommandSpec};

/// Why no Codex command runs from the desktop app.
const CHAT_COMING_SOON: &str = "Chat with Codex is coming soon.";

/// Name, description and argument hint of each built-in command.
const BUILTINS: &[(&str, &str, Option<&str>)] = &[
    ("init", "Create an AGENTS.md for this project", None),
    ("review", "Review the current changes", None),
    ("new", "Start a new conversation", None),
    ("resume", "Resume a saved conversation", None),
    ("fork", "Fork the conversation", None),
    ("compact", "Compact the conversation", None),
    ("diff", "Show the git diff", None),
    ("mention", "Mention a file", None),
    ("model", "Choose the model and reasoning effort", None),
    ("permissions", "Choose what Codex may do without asking", None),
    ("skills", "Use a skill", None),
    ("mcp", "List the MCP tools", None),
    ("status", "Show the session status", None),
];

/// The Codex catalog.
#[derive(Debug, Clone, Copy, Default)]
pub struct CodexCatalog;

/// The catalog [`super::Codex`] returns.
pub static CATALOG: CodexCatalog = CodexCatalog;

impl CommandCatalog for CodexCatalog {
    fn native(&self) -> Vec<CommandSpec> {
        BUILTINS
            .iter()
            .map(|(name, description, hint)| {
                spec(
                    name,
                    description,
                    *hint,
                    CommandSource::Builtin,
                    unsupported(CHAT_COMING_SOON),
                )
            })
            .collect()
    }

    fn map_shared(&self, _command: SharedCommand) -> CommandInvocation {
        unsupported(CHAT_COMING_SOON)
    }
}
