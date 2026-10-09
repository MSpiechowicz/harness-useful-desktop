//! The static PI command catalog.
//!
//! PI's built-in slash commands are terminal-only: sent as prompt text over RPC they would reach
//! the model. Each one therefore maps to the matching RPC command, a desktop view, or is marked
//! unsupported with a reason (PI docs `slash-commands.md`, `rpc.md`). Commands added by
//! extensions, prompt templates and skills arrive with a running session's live list.

use crate::harness::{CommandCatalog, gui, rpc, spec, unsupported};
use crate::model::{AgentActionKind, CommandInvocation, CommandSource, CommandSpec, GuiTarget};

use AgentActionKind as Action;

/// The PI catalog.
#[derive(Debug, Clone, Copy, Default)]
pub struct PiCatalog;

impl CommandCatalog for PiCatalog {
    fn native(&self) -> Vec<CommandSpec> {
        builtins()
            .into_iter()
            .map(|(name, description, hint, invocation)| {
                spec(name, description, hint, CommandSource::Builtin, invocation)
            })
            .collect()
    }
}

type Builtin = (&'static str, &'static str, Option<&'static str>, CommandInvocation);

fn builtins() -> Vec<Builtin> {
    vec![
        ("new", "Start a new session", None, rpc(Action::NewSession)),
        (
            "compact",
            "Compact the current context",
            Some("[instructions]"),
            rpc(Action::Compact),
        ),
        (
            "name",
            "Set the session display name",
            Some("<name>"),
            rpc(Action::SetSessionName),
        ),
        (
            "clone",
            "Duplicate the current session at its current position",
            None,
            rpc(Action::Clone),
        ),
        ("export", "Export the session as HTML", None, rpc(Action::ExportHtml)),
        (
            "session",
            "Show current session information and statistics",
            None,
            rpc(Action::GetSessionStats),
        ),
        (
            "copy",
            "Copy the last assistant message",
            None,
            rpc(Action::GetLastAssistantText),
        ),
        ("model", "Select a model", None, gui(GuiTarget::ModelPicker)),
        (
            "thinking",
            "Set the thinking level",
            None,
            gui(GuiTarget::ThinkingPicker),
        ),
        (
            "fork",
            "Create a new session from an earlier user message",
            None,
            gui(GuiTarget::ForkPicker),
        ),
        ("tree", "Navigate the session tree", None, gui(GuiTarget::SessionTree)),
        (
            "resume",
            "Switch to another saved session",
            None,
            gui(GuiTarget::ResumePicker),
        ),
        ("settings", "Open settings", None, gui(GuiTarget::Settings)),
        (
            "login", // guard-allow: login-rpc (listed as unsupported; never sent)
            "Add provider authentication",
            None,
            unsupported("Sign in from a terminal: run pi and use /login."),
        ),
        (
            "logout",
            "Remove provider authentication",
            None,
            unsupported("Sign out from a terminal: run pi and use /logout."),
        ),
        (
            "trust",
            "Save a project trust decision",
            None,
            unsupported("The desktop app asks for project trust each time it starts Pi."),
        ),
        (
            "reload",
            "Reload keybindings, extensions, skills, templates, themes and context files",
            None,
            unsupported("Start a new session to load changed resources."),
        ),
        (
            "share",
            "Upload the session and return a viewer link",
            None,
            unsupported("Uploading sessions is not available in the desktop app."),
        ),
        (
            "import",
            "Import and resume a JSONL session",
            Some("<path>"),
            unsupported("Import sessions from a terminal: pi /import <path>."),
        ),
        (
            "bug",
            "Prepare a private bug report for the Pi developers",
            None,
            unsupported("Report bugs from a terminal: run pi and use /bug."),
        ),
        (
            "changelog",
            "Show changelog entries",
            None,
            unsupported("Run pi and use /changelog in a terminal."),
        ),
        (
            "hotkeys",
            "Show active keyboard shortcuts",
            None,
            unsupported("Terminal shortcuts do not apply to the desktop app."),
        ),
        (
            "scoped-models",
            "Configure the models used by interactive cycling",
            None,
            unsupported("Configure scoped models from a terminal: run pi and use /scoped-models."),
        ),
        (
            "llama",
            "Manage models on the configured llama.cpp router",
            None,
            unsupported("Manage llama.cpp models from a terminal: run pi and use /llama."),
        ),
        (
            "quit",
            "Quit Pi",
            None,
            unsupported("Stop the session from the desktop app instead."),
        ),
    ]
}
