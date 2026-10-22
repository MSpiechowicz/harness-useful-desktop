//! The Claude Code command catalog (Claude Code 2.1.304).
//!
//! - **Native:** Claude's built-in commands that have a chat action or a desktop view, and the
//!   terminal-only ones, marked unsupported with what to do instead. `/init` and `/review` are
//!   prompts Claude expands itself, so they run as prompt text.
//! - **Shared:** the desktop's `/new`, `/compact`, `/model`, `/thinking`, `/name` and `/abort` keep
//!   their default mapping (the chat's actions and pickers).
//! - **Live:** the command list a running chat reports (`initialize`, `system/commands_changed`):
//!   objects with `name`, `description`, `argumentHint` and `builtin`, which the chat engine may
//!   label with a `source` (`skill` for the names `system/init` lists as skills). Without one, a
//!   `builtin` entry is built in, `mcp__<server>__<prompt>` is an MCP prompt, `<plugin>:<name>` a
//!   plugin command, anything else a custom command. Every live command runs as prompt text: Claude
//!   reads a `/name args` message as the command.

use serde_json::{Value, json};

use crate::harness::{Builtin, CommandCatalog, builtin_specs, gui, parse_live_commands, rpc, unsupported};
use crate::model::{AgentActionKind, CommandInvocation, CommandSource, CommandSpec, GuiTarget};

use AgentActionKind as Action;

/// Why a command of the Claude Code terminal app has no desktop counterpart.
const TERMINAL_ONLY: &str = "Only in the Claude Code terminal app.";

/// The Claude Code catalog.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeCatalog;

/// The catalog [`super::Claude`] returns.
pub static CATALOG: ClaudeCatalog = ClaudeCatalog;

impl CommandCatalog for ClaudeCatalog {
    fn native(&self) -> Vec<CommandSpec> {
        builtin_specs(builtins())
    }

    fn parse_live(&self, raw: &[Value]) -> Vec<CommandSpec> {
        let labelled: Vec<Value> = raw.iter().map(labelled).collect();

        parse_live_commands(&labelled, live_source, |_| CommandInvocation::PromptText)
    }
}

/// `entry` with its `source` (kept, else derived, see the module docs) and its `argumentHint` as
/// the `input.hint` the shared parser reads.
fn labelled(entry: &Value) -> Value {
    let mut entry = entry.clone();
    let name = entry.get("name").and_then(Value::as_str).unwrap_or_default();
    let builtin = entry.get("builtin").and_then(Value::as_bool).unwrap_or(false);
    let derived = derived_source(name, builtin);
    let hint = entry
        .get("argumentHint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|hint| !hint.is_empty())
        .map(str::to_owned);

    let Some(fields) = entry.as_object_mut() else {
        return entry;
    };

    fields.entry("source").or_insert_with(|| Value::from(derived));

    if let Some(hint) = hint {
        fields.insert("input".to_owned(), json!({ "hint": hint }));
    }

    entry
}

/// The source label of a live command that names none.
fn derived_source(name: &str, builtin: bool) -> &'static str {
    let name = name.trim_start_matches('/');

    if builtin {
        "builtin"
    } else if name.starts_with("mcp__") {
        "mcp"
    } else if name.contains(':') {
        "plugin"
    } else {
        "command"
    }
}

/// Live source labels.
fn live_source(label: &str) -> Option<CommandSource> {
    match label {
        "builtin" => Some(CommandSource::Builtin),
        "skill" => Some(CommandSource::Skill),
        "mcp" => Some(CommandSource::McpPrompt),
        "plugin" => Some(CommandSource::Extension),
        "command" => Some(CommandSource::Custom),
        _ => None,
    }
}

/// Claude's built-in commands, in the order of its `/help`.
fn builtins() -> Vec<Builtin> {
    vec![
        ("clear", "Start a new conversation", None, rpc(Action::NewSession)),
        (
            "resume",
            "Resume a saved conversation",
            None,
            gui(GuiTarget::ResumePicker),
        ),
        (
            "rename",
            "Rename the conversation",
            Some("<name>"),
            rpc(Action::SetSessionName),
        ),
        (
            "compact",
            "Compact the conversation",
            Some("[instructions]"),
            rpc(Action::Compact),
        ),
        (
            "rewind",
            "Rewind the conversation and code",
            None,
            unsupported("Only in the Claude Code terminal app. Fork from an earlier message instead."),
        ),
        (
            "export",
            "Export the conversation as HTML",
            None,
            rpc(Action::ExportHtml),
        ),
        ("copy", "Copy the last reply", None, rpc(Action::GetLastAssistantText)),
        ("model", "Choose the model", None, gui(GuiTarget::ModelPicker)),
        ("context", "Show the context usage", None, rpc(Action::GetSessionStats)),
        ("cost", "Show the session cost", None, rpc(Action::GetSessionStats)),
        (
            "init",
            "Create a CLAUDE.md for this project",
            None,
            CommandInvocation::PromptText,
        ),
        ("review", "Review a pull request", None, CommandInvocation::PromptText),
        ("agents", "Manage subagents", None, unsupported(TERMINAL_ONLY)),
        (
            "mcp",
            "Manage MCP servers",
            None,
            unsupported("Manage MCP servers in the Extensions view."),
        ),
        (
            "plugin",
            "Manage plugins",
            None,
            unsupported("Manage plugins in the Extensions view."),
        ),
        ("hooks", "Manage hooks", None, unsupported(TERMINAL_ONLY)),
        (
            "memory",
            "Edit the CLAUDE.md memory files",
            None,
            unsupported(TERMINAL_ONLY),
        ),
        (
            "permissions",
            "Manage tool permissions",
            None,
            unsupported(TERMINAL_ONLY),
        ),
        ("config", "Open the settings", None, unsupported(TERMINAL_ONLY)),
        ("status", "Show the status", None, unsupported(TERMINAL_ONLY)),
        ("doctor", "Check the installation", None, unsupported(TERMINAL_ONLY)),
        ("help", "Show help", None, unsupported(TERMINAL_ONLY)),
        (
            "exit",
            "Exit Claude Code",
            None,
            unsupported("Stop the session from the desktop app instead."),
        ),
    ]
}
