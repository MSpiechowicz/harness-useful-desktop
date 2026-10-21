//! The server's slash commands and skills an [`OpenCodeV2Session`](super::OpenCodeV2Session)
//! offers, read at startup and on `refreshCommands` (`GET /api/command`, `GET /api/skill`), as
//! OpenCode 1 listed them with its commands.
//!
//! - A command runs on `POST …/command`, a skill as a prompt with the skill attached
//!   (`delivery.rs`). A skill named like a command is left out: the command wins.
//! - Skills are named by their id (what a prompt attaches) and keep their description, never their
//!   path or instructions.

use serde_json::Value;

use super::startup::get_data;
use crate::error::Result;
use crate::opencode_server::OpenCodeClient;
use crate::opencode_server_v2::RouteV2;
use crate::opencode_server_v2::wire::Skill;

/// The server's commands and skills: how the command catalog lists them and how the chat runs them.
pub(super) struct Slash {
    /// The commands and the skills as the command catalog reads them ([`live_commands`],
    /// [`skill_entry`]).
    pub entries: Vec<Value>,
    /// Names of the server's commands (`POST …/command`).
    pub commands: Vec<String>,
    /// Ids of the skills a `/name` runs (a prompt with the skill attached); a command of the same
    /// name wins.
    pub skills: Vec<String>,
}

impl Slash {
    /// `GET /api/command` (which must answer) and `GET /api/skill` in `directory`. Skills are
    /// optional: a skill list the server does not give leaves none. OpenCode loads a directory's
    /// skills after the first request there; the chat reads both lists again on `refreshCommands`.
    pub(super) async fn read(client: &OpenCodeClient, directory: &str) -> Result<Self> {
        let commands = get_data(client, RouteV2::CommandList { directory }).await?;
        let skills = get_data(client, RouteV2::SkillList { directory })
            .await
            .unwrap_or_else(|error| {
                tracing::debug!(%error, "OpenCode listed no skills");
                Vec::new()
            });

        Ok(Self::new(commands, skills))
    }

    /// The commands `commands` (`GET /api/command`) and the skills `skills`.
    fn new(commands: Vec<Value>, skills: Vec<Skill>) -> Self {
        let mut entries = live_commands(commands);
        let commands = command_names(&entries);

        let skills: Vec<Skill> = skills
            .into_iter()
            .filter(|skill| !commands.contains(&skill.id))
            .collect();
        entries.extend(skills.iter().map(skill_entry));

        Self {
            entries,
            commands,
            skills: skills.into_iter().map(|skill| skill.id).collect(),
        }
    }
}

/// The `GET /api/command` list as the command catalog reads it. OpenCode 2 names no source: an MCP
/// prompt is `<server>:<prompt>` (`source: mcp`), every other entry a command (`source: command`).
fn live_commands(commands: Vec<Value>) -> Vec<Value> {
    commands
        .into_iter()
        .map(|mut command| {
            let mcp = command
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name.contains(':'));
            let source = if mcp { "mcp" } else { "command" };

            if let Some(entry) = command.as_object_mut() {
                entry.entry("source").or_insert_with(|| Value::from(source));
            }

            command
        })
        .collect()
}

/// `skill` as the command catalog reads it: named by its id (what a prompt attaches),
/// `source: skill`, as OpenCode 1 listed skills among its commands.
fn skill_entry(skill: &Skill) -> Value {
    let mut entry = serde_json::json!({ "name": skill.id, "source": "skill" });

    if let Some(description) = &skill.description {
        entry["description"] = Value::from(description.as_str());
    }

    entry
}

/// The names in a `GET /api/command` list.
fn command_names(commands: &[Value]) -> Vec<String> {
    commands
        .iter()
        .filter_map(|command| command.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}
