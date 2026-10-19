//! Slash-command catalogs.
//!
//! A catalog merges three lists, deduplicated by name with the first source winning:
//!
//! 1. **native** — the harness's own commands with their desktop mapping (static per harness);
//! 2. **live** — what a running agent reports (`available_commands_update` / `get_commands`),
//!    adding skills, extensions and custom commands; parsed by [`CommandCatalog::parse_live`];
//! 3. **shared** — the commands the desktop always offers ([`SharedCommand`]), mapped per harness
//!    by [`CommandCatalog::map_shared`].

use serde_json::Value;

use crate::model::{AgentActionKind, CommandInvocation, CommandSource, CommandSpec, GuiTarget};

/// A command the desktop offers for every harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedCommand {
    /// `/new`
    New,
    /// `/compact`
    Compact,
    /// `/model`
    Model,
    /// `/thinking`
    Thinking,
    /// `/name`
    Name,
    /// `/abort`
    Abort,
}

impl SharedCommand {
    /// Every shared command, in display order.
    pub const ALL: [SharedCommand; 6] = [
        SharedCommand::New,
        SharedCommand::Compact,
        SharedCommand::Model,
        SharedCommand::Thinking,
        SharedCommand::Name,
        SharedCommand::Abort,
    ];

    /// Command name without the slash.
    pub fn name(self) -> &'static str {
        match self {
            SharedCommand::New => "new",
            SharedCommand::Compact => "compact",
            SharedCommand::Model => "model",
            SharedCommand::Thinking => "thinking",
            SharedCommand::Name => "name",
            SharedCommand::Abort => "abort",
        }
    }

    fn description(self) -> &'static str {
        match self {
            SharedCommand::New => "Start a new session",
            SharedCommand::Compact => "Compact the conversation context",
            SharedCommand::Model => "Choose the model",
            SharedCommand::Thinking => "Choose the thinking level",
            SharedCommand::Name => "Rename this session",
            SharedCommand::Abort => "Stop the running turn",
        }
    }

    fn argument_hint(self) -> Option<&'static str> {
        match self {
            SharedCommand::Compact => Some("[instructions]"),
            SharedCommand::Name => Some("<name>"),
            _ => None,
        }
    }
}

/// The slash-command catalog of one harness.
pub trait CommandCatalog: Send + Sync {
    /// The harness's own commands with their desktop mapping.
    fn native(&self) -> Vec<CommandSpec>;

    /// How the desktop runs a shared command for this harness. The default sends the matching
    /// agent RPC or opens the matching desktop view, which works for every PI-family harness.
    fn map_shared(&self, command: SharedCommand) -> CommandInvocation {
        match command {
            SharedCommand::New => rpc(AgentActionKind::NewSession),
            SharedCommand::Compact => rpc(AgentActionKind::Compact),
            SharedCommand::Model => gui(GuiTarget::ModelPicker),
            SharedCommand::Thinking => gui(GuiTarget::ThinkingPicker),
            SharedCommand::Name => rpc(AgentActionKind::SetSessionName),
            SharedCommand::Abort => rpc(AgentActionKind::Abort),
        }
    }

    /// The shared commands mapped by [`CommandCatalog::map_shared`].
    fn shared(&self) -> Vec<CommandSpec> {
        SharedCommand::ALL
            .into_iter()
            .map(|command| {
                spec(
                    command.name(),
                    command.description(),
                    command.argument_hint(),
                    CommandSource::Shared,
                    self.map_shared(command),
                )
            })
            .collect()
    }

    /// Parses the command list reported by a running agent (see [`parse_live_commands`]). The
    /// default knows no live commands.
    fn parse_live(&self, _raw: &[Value]) -> Vec<CommandSpec> {
        Vec::new()
    }

    /// Native, live and shared commands merged by name (see [`merge_commands`]).
    fn merged(&self, live: &[Value]) -> Vec<CommandSpec> {
        merge_commands(self.native(), self.parse_live(live), self.shared())
    }
}

/// Concatenates the lists in precedence order (native > live > shared), keeping only the first
/// command of each name.
pub fn merge_commands(native: Vec<CommandSpec>, live: Vec<CommandSpec>, shared: Vec<CommandSpec>) -> Vec<CommandSpec> {
    let mut merged: Vec<CommandSpec> = Vec::new();

    for command in native.into_iter().chain(live).chain(shared) {
        if !merged.iter().any(|existing| existing.name == command.name) {
            merged.push(command);
        }
    }

    merged
}

/// Longest accepted live command name.
const MAX_LIVE_NAME: usize = 128;

/// Longest live description kept, in characters.
const MAX_LIVE_DESCRIPTION: usize = 512;

/// Parses a live command list: objects with `name`, optional `description`, optional
/// `input.hint` (OMP) and `source`. `source` maps the harness's source label (entries it rejects
/// are skipped) and `invocation` decides how each name runs. Names lose a leading `/`; empty
/// names, names with whitespace or control characters, and repeated names are skipped.
pub fn parse_live_commands(
    raw: &[Value],
    source: impl Fn(&str) -> Option<CommandSource>,
    invocation: impl Fn(&str) -> CommandInvocation,
) -> Vec<CommandSpec> {
    let mut commands: Vec<CommandSpec> = Vec::new();

    for entry in raw {
        let Some(name) = entry
            .get("name")
            .and_then(Value::as_str)
            .map(|name| name.trim_start_matches('/'))
        else {
            continue;
        };

        let valid_name = !name.is_empty()
            && name.len() <= MAX_LIVE_NAME
            && !name
                .chars()
                .any(|character| character.is_whitespace() || character.is_control());

        let Some(source) = entry.get("source").and_then(Value::as_str).and_then(&source) else {
            continue;
        };

        if !valid_name || commands.iter().any(|existing| existing.name == name) {
            continue;
        }

        let description: String = entry
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .chars()
            .take(MAX_LIVE_DESCRIPTION)
            .collect();
        let hint = entry.pointer("/input/hint").and_then(Value::as_str);

        commands.push(spec(name, &description, hint, source, invocation(name)));
    }

    commands
}

/// A harness's own command: name, description, argument hint and how the desktop runs it.
pub type Builtin = (&'static str, &'static str, Option<&'static str>, CommandInvocation);

/// The [`CommandSource::Builtin`] specs of `builtins`, in order.
pub fn builtin_specs(builtins: Vec<Builtin>) -> Vec<CommandSpec> {
    builtins
        .into_iter()
        .map(|(name, description, hint, invocation)| spec(name, description, hint, CommandSource::Builtin, invocation))
        .collect()
}

/// Builds a [`CommandSpec`].
pub fn spec(
    name: &str,
    description: &str,
    argument_hint: Option<&str>,
    source: CommandSource,
    invocation: CommandInvocation,
) -> CommandSpec {
    CommandSpec {
        name: name.to_owned(),
        description: description.to_owned(),
        argument_hint: argument_hint.map(str::to_owned),
        source,
        invocation,
    }
}

/// Runs the command as an agent RPC.
pub fn rpc(action: AgentActionKind) -> CommandInvocation {
    CommandInvocation::Rpc { action }
}

/// Opens a desktop view instead.
pub fn gui(target: GuiTarget) -> CommandInvocation {
    CommandInvocation::Gui { target }
}

/// Not available in the desktop app, with the reason shown to the user.
pub fn unsupported(reason: &str) -> CommandInvocation {
    CommandInvocation::Unsupported {
        reason: reason.to_owned(),
    }
}
