//! The OpenCode command catalog (OpenCode 1.19.3).
//!
//! The terminal app's slash commands (`packages/tui`) never reach the server: sent as prompt text
//! they would go to the model. Each one therefore maps to the matching chat action or desktop
//! view, or is marked unsupported with a reason. The shared desktop commands (`/name`, `/model`,
//! `/thinking`, …) keep their default mapping, which the chat engine supports.
//!
//! The server's own commands (`/init`, `/review`), custom commands, MCP prompts and skills arrive
//! with a running chat's `GET /command` list ([`CommandCatalog::parse_live`]) and run as `/name
//! args` prompt text, which the chat engine sends to `POST /session/{id}/command`. Their `hints`
//! (the template's `$1`, `$2`, … and `$ARGUMENTS`) become the argument hint: `<arg1> <arg2>
//! [arguments]`.

use serde_json::{Value, json};

use crate::harness::{Builtin, CommandCatalog, builtin_specs, gui, parse_live_commands, rpc, unsupported};
use crate::model::{AgentActionKind, CommandInvocation, CommandSource, CommandSpec, GuiTarget};

use AgentActionKind as Action;

/// Why a command of the OpenCode terminal app has no desktop counterpart.
const TERMINAL_ONLY: &str = "Only in the OpenCode terminal app.";

/// The OpenCode catalog.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenCodeCatalog;

/// The catalog [`super::OpenCode`] returns.
pub static CATALOG: OpenCodeCatalog = OpenCodeCatalog;

impl CommandCatalog for OpenCodeCatalog {
    fn native(&self) -> Vec<CommandSpec> {
        builtin_specs(builtins())
    }

    fn parse_live(&self, raw: &[Value]) -> Vec<CommandSpec> {
        let raw: Vec<Value> = raw.iter().map(with_input_hint).collect();

        parse_live_commands(&raw, live_source, |_| CommandInvocation::PromptText)
    }
}

/// `entry` with its `hints` as the `input.hint` the shared parser reads.
fn with_input_hint(entry: &Value) -> Value {
    let mut entry = entry.clone();
    let hints = entry.get("hints").and_then(Value::as_array).into_iter().flatten();
    let hint: Vec<String> = hints.filter_map(Value::as_str).filter_map(hint_word).collect();

    if !hint.is_empty() && entry.is_object() {
        entry["input"] = json!({ "hint": hint.join(" ") });
    }

    entry
}

/// One of OpenCode's argument placeholders as the hint shows it.
fn hint_word(placeholder: &str) -> Option<String> {
    if placeholder == "$ARGUMENTS" {
        return Some("[arguments]".to_owned());
    }

    let number = placeholder.strip_prefix('$')?;

    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    Some(format!("<arg{number}>"))
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

/// The terminal app's commands (with `/resume` and `/agent`, which the desktop also accepts), in
/// the terminal app's order.
fn builtins() -> Vec<Builtin> {
    vec![
        ("new", "Start a new session", None, rpc(Action::NewSession)),
        (
            "sessions",
            "Switch to another saved session",
            None,
            gui(GuiTarget::ResumePicker),
        ),
        (
            "resume",
            "Switch to another saved session",
            None,
            gui(GuiTarget::ResumePicker),
        ),
        (
            "rename",
            "Rename the session",
            Some("<name>"),
            rpc(Action::SetSessionName),
        ),
        ("compact", "Compact the session", None, rpc(Action::Compact)),
        (
            "fork",
            "Create a new session from an earlier message",
            None,
            gui(GuiTarget::ForkPicker),
        ),
        ("export", "Export the session as HTML", None, rpc(Action::ExportHtml)),
        (
            "copy",
            "Copy the last assistant message",
            None,
            rpc(Action::GetLastAssistantText),
        ),
        ("models", "Choose the model", None, gui(GuiTarget::ModelPicker)),
        (
            "variants",
            "Choose the model variant",
            None,
            gui(GuiTarget::ThinkingPicker),
        ),
        ("agents", "Switch agent", None, gui(GuiTarget::AgentPicker)),
        ("agent", "Switch agent", None, gui(GuiTarget::AgentPicker)),
        (
            "undo",
            "Undo the previous message",
            None,
            unsupported("Only in the OpenCode terminal app. Fork from an earlier message instead."),
        ),
        ("redo", "Redo the undone message", None, unsupported(TERMINAL_ONLY)),
        ("timeline", "Jump to a message", None, unsupported(TERMINAL_ONLY)),
        (
            "move",
            "Move the session to another copy of the project",
            None,
            unsupported(TERMINAL_ONLY),
        ),
        ("mcps", "Turn MCP servers on or off", None, unsupported(TERMINAL_ONLY)),
        (
            "skills",
            "Browse skills",
            None,
            unsupported("Skills are listed with the other slash commands."),
        ),
        (
            "connect",
            "Connect a provider",
            None,
            unsupported("Connect providers from a terminal: run opencode and use /connect."),
        ),
        ("themes", "Switch theme", None, unsupported(TERMINAL_ONLY)),
        (
            "timestamps",
            "Show or hide timestamps",
            None,
            unsupported(TERMINAL_ONLY),
        ),
        (
            "editor",
            "Write the prompt in your editor",
            None,
            unsupported(TERMINAL_ONLY),
        ),
        ("diff", "Open the diff viewer", None, unsupported(TERMINAL_ONLY)),
        ("status", "View status", None, unsupported(TERMINAL_ONLY)),
        ("help", "Show help", None, unsupported(TERMINAL_ONLY)),
        (
            "exit",
            "Exit OpenCode",
            None,
            unsupported("Stop the session from the desktop app instead."),
        ),
    ]
}
