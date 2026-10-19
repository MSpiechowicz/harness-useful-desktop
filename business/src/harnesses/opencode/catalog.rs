//! The OpenCode command catalog: the built-in slash commands of OpenCode 1.19.3 (the server's
//! `/init` and `/review`, and the terminal UI's commands).
//!
//! Chat with OpenCode is not available in the desktop app yet, so every built-in command, the
//! shared ones included, is listed as unsupported with that reason. Custom commands appear in the
//! extensions view. A running server's `GET /command` list (custom commands, MCP prompts and
//! skills) is sent as `/name args` prompt text.

use serde_json::Value;

use crate::harness::{CommandCatalog, SharedCommand, parse_live_commands, spec, unsupported};
use crate::model::{CommandInvocation, CommandSource, CommandSpec};

/// Why no OpenCode command runs from the desktop app.
const CHAT_COMING_SOON: &str = "Chat with OpenCode is coming soon.";

/// Name, description and argument hint of each built-in command.
const BUILTINS: &[(&str, &str, Option<&str>)] = &[
    ("init", "Create or update AGENTS.md for this project", None),
    (
        "review",
        "Review changes, by default the uncommitted ones",
        Some("[commit|branch|pr]"),
    ),
    ("new", "Start a new session", None),
    ("sessions", "Switch session", None),
    ("rename", "Rename the session", None),
    ("compact", "Compact the session", None),
    ("undo", "Undo the previous message", None),
    ("redo", "Redo the undone message", None),
    ("fork", "Fork the session", None),
    ("timeline", "Jump to a message", None),
    ("copy", "Copy the session transcript", None),
    ("export", "Export the session transcript", None),
    ("models", "Switch model", None),
    ("variants", "Switch model variant", None),
    ("agents", "Switch agent", None),
    ("mcps", "Turn MCP servers on or off", None),
    ("skills", "Browse skills", None),
    ("connect", "Connect a provider", None),
    ("themes", "Switch theme", None),
    ("thinking", "Expand or collapse thinking", None),
    ("timestamps", "Show or hide timestamps", None),
    ("editor", "Write the prompt in your editor", None),
    ("diff", "Open the diff viewer", None),
    ("status", "View status", None),
    ("help", "Show help", None),
    ("exit", "Exit OpenCode", None),
];

/// The OpenCode catalog.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenCodeCatalog;

/// The catalog [`super::OpenCode`] returns.
pub static CATALOG: OpenCodeCatalog = OpenCodeCatalog;

impl CommandCatalog for OpenCodeCatalog {
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

    fn parse_live(&self, raw: &[Value]) -> Vec<CommandSpec> {
        parse_live_commands(raw, live_source, |_| CommandInvocation::PromptText)
    }
}

/// `GET /command` source labels.
fn live_source(label: &str) -> Option<CommandSource> {
    match label {
        "command" => Some(CommandSource::Custom),
        "mcp" => Some(CommandSource::McpPrompt),
        "skill" => Some(CommandSource::Skill),
        _ => None,
    }
}
