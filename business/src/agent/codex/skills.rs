//! Codex's skills as slash commands of a [`CodexSession`](super::CodexSession).
//!
//! - **List:** `skills/list` for the project, right after startup (in the background, then a fresh
//!   `commandsUpdated`) and on every `refreshCommands`. Enabled skills only, each name once; a
//!   list Codex refuses keeps the skills listed before.
//! - **Commands:** the harness catalog with the skills as its live commands ([`CodexCatalog`]):
//!   source `skill`, run as prompt text. A built-in command of the same name wins.
//! - **Use:** a message whose first word is `/<skill>` goes to Codex as its text plus a `skill`
//!   input item naming the skill's `SKILL.md`, which Codex loads for the turn.

use std::sync::Arc;

use serde_json::{Value, json};

use super::Chat;
use crate::codex_rpc::wire::items::UserInput;
use crate::codex_rpc::wire::outbound::Request;
use crate::codex_rpc::wire::results::{Skill, SkillsList};
use crate::error::Result;
use crate::harness::CommandCatalog;
use crate::harnesses::codex::CodexCatalog;
use crate::model::{AgentEvent, AgentReply, CommandSpec};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;

impl Chat {
    /// `refreshCommands`: the skills listed again, then the catalog.
    pub(super) async fn refresh_commands(&self) -> Result<AgentReply> {
        self.load_skills().await;

        Ok(AgentReply::Commands {
            commands: self.commands_now(),
        })
    }

    /// Lists the skills in the background and offers them once listed.
    pub(super) fn load_skills_later(self: &Arc<Self>) {
        let chat = self.clone();

        tokio::spawn(async move {
            if chat.load_skills().await {
                let commands = chat.commands_now();
                chat.shared.emit(AgentEvent::CommandsUpdated { commands });
            }
        });
    }

    /// `skills/list` for the project; false when Codex listed none.
    async fn load_skills(&self) -> bool {
        let request = Request::SkillsList {
            cwds: vec![self.project.to_string_lossy().into_owned()],
        };

        match self.request::<SkillsList>(&request, DEFAULT_TIMEOUT).await {
            Ok(listed) => {
                self.lock().skills = enabled(listed);
                true
            }
            Err(error) => {
                tracing::debug!(%error, "Codex listed no skills");
                false
            }
        }
    }

    /// The catalog with the skills listed last.
    fn commands_now(&self) -> Vec<CommandSpec> {
        let live = self.lock();
        let skills: Vec<Value> = live
            .skills
            .iter()
            .map(|skill| json!({ "name": skill.name, "description": skill.description, "source": "skill" }))
            .collect();

        CodexCatalog.merged(&skills)
    }

    /// The skill `text` calls (`/<name> …` of a listed skill) as Codex's input item.
    pub(super) fn skill_input(&self, text: &str) -> Option<UserInput> {
        let name = text.strip_prefix('/')?.split_whitespace().next()?;
        let live = self.lock();
        let skill = live.skills.iter().find(|skill| skill.name == name)?;

        Some(UserInput::Skill {
            name: skill.name.clone(),
            path: skill.path.clone(),
        })
    }
}

/// The enabled skills of `listed`, each name once.
fn enabled(listed: SkillsList) -> Vec<Skill> {
    let mut skills: Vec<Skill> = Vec::new();

    for skill in listed.data.into_iter().flat_map(|entry| entry.skills) {
        let usable = skill.enabled && !skill.name.is_empty() && !skill.path.is_empty();

        if usable && !skills.iter().any(|known| known.name == skill.name) {
            skills.push(skill);
        }
    }

    skills
}
