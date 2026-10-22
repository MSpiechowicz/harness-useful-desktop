//! The Codex command catalog (Codex CLI 0.154).
//!
//! - **Native:** the built-in slash commands of Codex's terminal app that have a chat action or a
//!   desktop view (`/fork` duplicates the conversation, a clone), and the ones that exist only
//!   there, marked unsupported with what to do instead.
//! - **Shared:** the desktop's `/new`, `/compact`, `/model`, `/thinking`, `/name` and `/abort` keep
//!   their default mapping (the chat's actions and pickers).
//! - **Live:** the skills the running chat lists (`skills/list`), as objects with `name`,
//!   `description` and the source `skill`. They run as prompt text: the chat engine sends a
//!   `/<skill>` message with the skill attached.

use serde_json::Value;

use crate::harness::{Builtin, CommandCatalog, builtin_specs, gui, parse_live_commands, rpc, unsupported};
use crate::model::{AgentActionKind, CommandInvocation, CommandSource, CommandSpec, GuiTarget};

use AgentActionKind as Action;

/// Why a command of the Codex terminal app has no desktop counterpart.
const TERMINAL_ONLY: &str = "Only in the Codex terminal app.";

/// The Codex catalog.
#[derive(Debug, Clone, Copy, Default)]
pub struct CodexCatalog;

/// The catalog [`super::Codex`] returns.
pub static CATALOG: CodexCatalog = CodexCatalog;

impl CommandCatalog for CodexCatalog {
    fn native(&self) -> Vec<CommandSpec> {
        builtin_specs(builtins())
    }

    fn parse_live(&self, raw: &[Value]) -> Vec<CommandSpec> {
        let source = |label: &str| (label == "skill").then_some(CommandSource::Skill);

        parse_live_commands(raw, source, |_| CommandInvocation::PromptText)
    }
}

/// Codex's built-in commands, in the order of its `/` menu.
fn builtins() -> Vec<Builtin> {
    vec![
        (
            "init",
            "Create an AGENTS.md for this project",
            None,
            unsupported(TERMINAL_ONLY),
        ),
        ("review", "Review the current changes", None, unsupported(TERMINAL_ONLY)),
        ("new", "Start a new conversation", None, rpc(Action::NewSession)),
        (
            "resume",
            "Resume a saved conversation",
            None,
            gui(GuiTarget::ResumePicker),
        ),
        ("fork", "Fork the conversation", None, rpc(Action::Clone)),
        ("compact", "Compact the conversation", None, rpc(Action::Compact)),
        (
            "diff",
            "Show the git diff",
            None,
            unsupported("Only in the Codex terminal app. The chat's tool cards show each change."),
        ),
        ("mention", "Mention a file", None, unsupported(TERMINAL_ONLY)),
        (
            "model",
            "Choose the model and reasoning effort",
            None,
            gui(GuiTarget::ModelPicker),
        ),
        (
            "permissions",
            "Choose what Codex may do without asking",
            None,
            unsupported("Only in the Codex terminal app. The chat asks as Codex's config.toml says."),
        ),
        (
            "skills",
            "Use a skill",
            None,
            unsupported("Type / and the skill's name to use it."),
        ),
        (
            "mcp",
            "List the MCP tools",
            None,
            unsupported("See the MCP servers in the Extensions view."),
        ),
        ("status", "Show the session status", None, rpc(Action::GetSessionStats)),
    ]
}
