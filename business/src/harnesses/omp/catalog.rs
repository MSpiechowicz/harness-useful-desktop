//! The static OMP command catalog.
//!
//! OMP runs a slash command sent as prompt text. The builtins below are the ones OMP 18.6.1
//! advertises over RPC (`BUILTIN_SLASH_COMMANDS_INTERNAL` entries with a non-TUI handler, using
//! their RPC description and input hint) plus the bundled custom commands. TUI-only builtins such
//! as `/new`, `/tree` or `/resume` are absent; the shared desktop commands cover them over RPC.
//! A running session reports its live list (`available_commands_update`,
//! `get_available_commands`), which adds skills, extension, custom, file and MCP prompt commands;
//! every live command runs as prompt text except `/model`, which opens the model picker.

use serde_json::Value;

use crate::harness::{CommandCatalog, gui, parse_live_commands, spec};
use crate::model::{CommandInvocation, CommandSource, CommandSpec, GuiTarget};

/// `(name, description, argument hint)` of the RPC-capable builtins.
const BUILTINS: &[(&str, &str, Option<&str>)] = &[
    (
        "security",
        "Plan, run, inspect, import, and compare OMP-native security scans",
        Some("<plan|scan|status|cancel|scans|show|import|export|validate|compare|disposition>"),
    ),
    ("model", "Show current model selection", None),
    ("switch", "Switch model for this session only", Some("[model]")),
    ("fast", "Toggle fast mode", Some("[on|ultra|off|status]")),
    ("slow", "Toggle slow mode", Some("[on|off|status]")),
    ("skillful", "Toggle skill listing", Some("[on|off|status]")),
    ("extended-context", "Toggle extended context", Some("[on|off|status]")),
    ("computer", "Toggle computer use", Some("[on|off|status]")),
    (
        "ratchet",
        "Build (or reuse) an eval for an LLM flow, then hillclimb it unattended",
        Some("[flow and goal]"),
    ),
    ("prewalk", "Arm or restart prewalk", Some("[restart]")),
    (
        "modelpreset",
        "Manage model presets",
        Some("[list|save|switch|delete] [name]"),
    ),
    (
        "effort",
        "Set or show reasoning effort (thinking level, intelligence)",
        Some("[level]"),
    ),
    (
        "advisor",
        "Toggle advisor",
        Some("[on|off|status|dump [raw]|configure]"),
    ),
    ("export", "Export session to HTML file", Some("[--themes] [path]")),
    ("trace", "Open this session's trace in the stats dashboard", None),
    (
        "dump",
        "Return full transcript as plain text, with LLM request JSON path",
        Some("[all]"),
    ),
    (
        "share",
        "Share session via an encrypted link (share server or secret gist)",
        None,
    ),
    (
        "browser",
        "Toggle browser eval-prelude headless vs visible mode",
        Some("[headless|visible]"),
    ),
    ("todo", "Manage todos", Some("<subcommand>")),
    (
        "session",
        "Show or configure the current session",
        Some("[info|delete|pin [account]]"),
    ),
    ("jobs", "Show background jobs", Some("[full]")),
    (
        "usage",
        "Show token usage",
        Some("[show|reset [provider/credential-id|provider/active]]"),
    ),
    (
        "stats",
        "Launch the local stats dashboard",
        Some("[--port <port>] [--host <host>]"),
    ),
    ("changelog", "Show changelog", Some("[full|last [N]]")),
    ("tools", "Show available tools", None),
    ("context", "Show context usage", None),
    ("mcp", "Manage MCP servers", Some("<subcommand>")),
    ("ssh", "Manage SSH connections", Some("<subcommand>")),
    (
        "fresh",
        "Reset provider stream state without changing the local transcript",
        None,
    ),
    (
        "compact",
        "Compact the conversation",
        Some("[soft|remote|snapcompact] [focus]"),
    ),
    (
        "shake",
        "Shake heavy content out of the conversation context",
        Some("[elide|images|thinking]"),
    ),
    (
        "handoff",
        "Summarize the session into a handoff document and compact in place",
        Some("[focus instructions]"),
    ),
    (
        "pin",
        "Pin or unpin a session at the top of the resume list",
        Some("[session id]"),
    ),
    ("retry", "Retry the last failed agent turn", None),
    ("memory", "Manage memory", Some("<subcommand>")),
    (
        "rename",
        "Rename the current session (omit title to generate)",
        Some("[title]"),
    ),
    (
        "move",
        "Move the current session to a different directory",
        Some("[<path>]"),
    ),
    (
        "wt",
        "Move this session into a new worktree, changes included",
        Some("[<branch>]"),
    ),
    ("add-dir", "Add a workspace directory to this session", Some("<path>")),
    (
        "remove-dir",
        "Remove a workspace directory from this session",
        Some("<path>"),
    ),
    ("dirs", "List this session's workspace directories", None),
    ("marketplace", "Manage plugins from marketplaces", Some("<subcommand>")),
    ("plugins", "Manage plugins", Some("[list|enable|disable]")),
    ("reload-plugins", "Reload all plugins", None),
    (
        "force",
        "Force next turn to use a specific tool",
        Some("<tool-name> [prompt]"),
    ),
];

/// Commands bundled with OMP (`extensibility/custom-commands/bundled`).
const BUNDLED: &[(&str, &str)] = &[
    (
        "green",
        "Generate a prompt to iterate on CI failures until the branch is green",
    ),
    ("review", "Launch interactive code review"),
    (
        "annotate",
        "Annotate a diff or text from a file, prompt, latest reply, or session",
    ),
];

/// The OMP catalog.
#[derive(Debug, Clone, Copy, Default)]
pub struct OmpCatalog;

impl CommandCatalog for OmpCatalog {
    fn native(&self) -> Vec<CommandSpec> {
        let builtins = BUILTINS
            .iter()
            .map(|(name, description, hint)| spec(name, description, *hint, CommandSource::Builtin, invocation(name)));

        let bundled = BUNDLED.iter().map(|(name, description)| {
            spec(
                name,
                description,
                None,
                CommandSource::Custom,
                CommandInvocation::PromptText,
            )
        });

        builtins.chain(bundled).collect()
    }

    fn parse_live(&self, raw: &[Value]) -> Vec<CommandSpec> {
        parse_live_commands(raw, live_source, invocation)
    }
}

/// OMP's `SlashCommandSource` labels.
fn live_source(label: &str) -> Option<CommandSource> {
    match label {
        "builtin" => Some(CommandSource::Builtin),
        "skill" => Some(CommandSource::Skill),
        "extension" => Some(CommandSource::Extension),
        "custom" => Some(CommandSource::Custom),
        "mcp_prompt" => Some(CommandSource::McpPrompt),
        "file" => Some(CommandSource::File),
        _ => None,
    }
}

/// Prompt text, except `/model`, which opens the desktop model picker.
fn invocation(name: &str) -> CommandInvocation {
    match name {
        "model" => gui(GuiTarget::ModelPicker),
        _ => CommandInvocation::PromptText,
    }
}
