//! The Claude Code command catalog: well-known built-in slash commands of Claude Code 2.1.
//!
//! Chat with Claude Code is not available in the desktop app yet, so every command, the shared
//! ones included, is listed as unsupported with that reason. A running chat will report the full
//! list (skills and plugin commands included) once the chat engine exists.

use crate::harness::{CommandCatalog, SharedCommand, spec, unsupported};
use crate::model::{CommandInvocation, CommandSource, CommandSpec};

/// Why no Claude Code command runs from the desktop app.
const CHAT_COMING_SOON: &str = "Chat with Claude Code is coming soon.";

/// Name, description and argument hint of each built-in command.
const BUILTINS: &[(&str, &str, Option<&str>)] = &[
    ("init", "Create a CLAUDE.md for this project", None),
    ("review", "Review a pull request", None),
    ("clear", "Start a new conversation", None),
    ("resume", "Resume a saved conversation", None),
    ("rename", "Rename the conversation", Some("<name>")),
    ("compact", "Compact the conversation", Some("[instructions]")),
    ("rewind", "Rewind the conversation and code", None),
    ("export", "Export the conversation", None),
    ("model", "Choose the model", None),
    ("context", "Show the context usage", None),
    ("cost", "Show the session cost", None),
    ("agents", "Manage subagents", None),
    ("mcp", "Manage MCP servers", None),
    ("plugin", "Manage plugins", None),
    ("hooks", "Manage hooks", None),
    ("memory", "Edit the CLAUDE.md memory files", None),
    ("permissions", "Manage tool permissions", None),
    ("config", "Open the settings", None),
    ("status", "Show the status", None),
    ("doctor", "Check the installation", None),
    ("help", "Show help", None),
];

/// The Claude Code catalog.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeCatalog;

/// The catalog [`super::Claude`] returns.
pub static CATALOG: ClaudeCatalog = ClaudeCatalog;

impl CommandCatalog for ClaudeCatalog {
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
