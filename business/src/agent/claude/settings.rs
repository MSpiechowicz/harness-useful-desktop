//! The state, pickers and commands of a [`ClaudeSession`], answered from what startup and the
//! frames told.
//!
//! - **Models:** the ones `initialize` offered, read again with `list_models` when the picker asks
//!   for them (the cached list stays when that fails). A model is offered by its `set_model` value
//!   (`default`, `opus`, a full model id) under the provider [`PROVIDER`]; context window and
//!   input kinds are unknown, so they stay empty. The chat's model is the one it picked, else the
//!   entry of the model the newest `system/init` named, else of the one the process started on
//!   (Harness Skills' default role), else of the `default` entry's resolved model, so the picker
//!   names the model the CLI uses from the start. An entry is the one with that value, else one
//!   that resolves to it (another than `default` first).
//! - **Thinking:** a model's levels are its `supportedEffortLevels`, after `off` when it can turn
//!   thinking off. A level is `apply_flag_settings{effortLevel}`, `off` is
//!   `set_max_thinking_tokens 0` (lifted again before the next level). Claude Code does not report
//!   its effort, so the level is unknown until the chat picks one or starts with an `--effort`; a
//!   model change keeps it only when the new model has it.
//! - **Renaming:** `rename_session`; the title comes back as `session_title_changed`.
//! - **Commands:** the `initialize` list, replaced by every `system/commands_changed`, labelled
//!   with the skill names of `system/init`; `refreshCommands` asks `reload_skills` and adds the
//!   skills the list lacks.
//!
//! [`ClaudeSession`]: super::ClaudeSession

use serde_json::{Value, json};

use super::{Chat, ClaudeSession, Live};
use crate::claude_stream::wire::control::{ModelInfo, ModelsReply, SkillsReply, SlashCommand};
use crate::claude_stream::wire::outbound::HostRequest;
use crate::claude_stream::wire::system::Init;
use crate::error::{BusinessError, Result};
use crate::harness::CommandCatalog;
use crate::harnesses::claude::ClaudeCatalog;
use crate::model::{AgentAction, AgentEvent, AgentReply, CommandSpec, ModelOption, SessionState};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;

/// The provider the chat names Claude Code's models with.
pub(super) const PROVIDER: &str = "anthropic";

/// The `set_model` value of the default model.
pub(super) const DEFAULT_MODEL: &str = "default";

/// The thinking level that turns thinking off.
const OFF: &str = "off";

impl ClaudeSession {
    /// Runs a state, picker or command action.
    pub(super) async fn setting(&self, action: AgentAction) -> Result<AgentReply> {
        let run = self.run();
        let chat = &run.chat;

        match action {
            AgentAction::GetState => Ok(AgentReply::State {
                state: chat.state(&chat.lock()),
            }),
            AgentAction::GetAvailableModels => chat.models().await,
            AgentAction::SetModel { model_id, .. } => chat.set_model(model_id).await,
            AgentAction::GetAvailableThinkingLevels => Ok(AgentReply::ThinkingLevels {
                levels: levels(&chat.lock()),
            }),
            AgentAction::SetThinkingLevel { level } => chat.set_thinking_level(level).await,
            AgentAction::SetSessionName { name } => chat.rename(name).await,
            _ => chat.refresh_commands().await,
        }
    }
}

impl Chat {
    /// `rename_session`: Claude Code saves the title and reports it (`session_title_changed`).
    async fn rename(&self, name: String) -> Result<AgentReply> {
        let title = name.trim().to_owned();

        if title.is_empty() {
            return Err(BusinessError::InvalidInput("a chat name must not be blank".to_owned()));
        }

        self.control(&HostRequest::RenameSession { title }, DEFAULT_TIMEOUT)
            .await?;

        Ok(AgentReply::Ack)
    }

    /// `list_models`, falling back to the models offered before.
    async fn models(&self) -> Result<AgentReply> {
        let listed = self.control(&HostRequest::ListModels, DEFAULT_TIMEOUT).await;

        let mut live = self.lock();

        match listed.map(serde_json::from_value::<ModelsReply>) {
            Ok(Ok(reply)) if !reply.models.is_empty() => live.models = reply.models,
            Ok(_) => {}
            Err(error) => tracing::debug!(%error, "Claude Code listed no models"),
        }

        Ok(AgentReply::Models {
            models: live.models.iter().map(option).collect(),
        })
    }

    /// Picks one of the offered models.
    async fn set_model(&self, value: String) -> Result<AgentReply> {
        if !self.lock().models.iter().any(|model| model.value == value) {
            return Err(BusinessError::InvalidInput(format!(
                "Claude Code offers no model {value}"
            )));
        }

        let model = Some(value.clone()).filter(|value| value != DEFAULT_MODEL);
        self.control(&HostRequest::SetModel { model }, DEFAULT_TIMEOUT).await?;

        let mut live = self.lock();
        live.model = Some(value);
        self.shared.emit(AgentEvent::ModelChanged {
            model: current_model(&live),
        });

        if !thinking_offered(&live) {
            live.thinking = None;
            self.emit_state(&live);
        }

        Ok(AgentReply::Ack)
    }

    /// Picks one of the current model's thinking levels.
    async fn set_thinking_level(&self, level: String) -> Result<AgentReply> {
        let was_off = {
            let live = self.lock();

            if !levels(&live).contains(&level) {
                return Err(BusinessError::InvalidInput(format!(
                    "the model has no thinking level {level}"
                )));
            }

            live.thinking.as_deref() == Some(OFF)
        };

        if level == OFF {
            let request = HostRequest::SetMaxThinkingTokens { tokens: Some(0) };
            self.control(&request, DEFAULT_TIMEOUT).await?;
        } else {
            if was_off {
                let request = HostRequest::SetMaxThinkingTokens { tokens: None };
                self.control(&request, DEFAULT_TIMEOUT).await?;
            }

            let request = HostRequest::SetEffort { level: level.clone() };
            self.control(&request, DEFAULT_TIMEOUT).await?;
        }

        self.lock().thinking = Some(level.clone());
        self.shared.emit(AgentEvent::ThinkingChanged { level });

        Ok(AgentReply::Ack)
    }

    /// `reload_skills`, then the merged catalog with the skills the command list lacks.
    async fn refresh_commands(&self) -> Result<AgentReply> {
        let reply = self.control(&HostRequest::ReloadSkills, DEFAULT_TIMEOUT).await?;
        let skills = serde_json::from_value::<SkillsReply>(reply).unwrap_or_default().skills;

        let mut live = self.lock();

        for skill in skills {
            if !live.skills.contains(&skill.name) {
                live.skills.push(skill.name.clone());
            }

            if !live.commands.iter().any(|command| command.name == skill.name) {
                live.commands.push(skill);
            }
        }

        let commands = catalog(&live);
        self.shared.emit(AgentEvent::CommandsUpdated {
            commands: commands.clone(),
        });

        Ok(AgentReply::Commands { commands })
    }

    /// The session state from `live`.
    pub(super) fn state(&self, live: &Live) -> SessionState {
        SessionState {
            session_id: live.session_id.clone().unwrap_or_default(),
            session_name: live.title.clone(),
            model: current_model(live),
            thinking_level: live.thinking.clone(),
            agent: None,
            is_streaming: live.mapper.is_working(),
            is_compacting: live.mapper.is_compacting(),
            message_count: live.counts.total,
            queued_count: u32::try_from(live.queue.len()).unwrap_or(u32::MAX),
            context_usage: live.stats.context().cloned(),
        }
    }

    /// A turn began: the model, permission mode and skills it names.
    pub(super) fn on_init(&self, live: &mut Live, init: &Init) {
        live.permission_mode.clone_from(&init.permission_mode);

        let model = Some(init.model.clone()).filter(|model| !model.is_empty());
        if model.is_some() && model != live.running_model {
            let before = current_model(live);
            live.running_model = model;

            let after = current_model(live);
            if after != before {
                self.shared.emit(AgentEvent::ModelChanged { model: after });
            }
        }

        if live.skills != init.skills {
            live.skills.clone_from(&init.skills);
            self.emit_commands(live);
        }
    }

    /// Emits the merged command catalog.
    pub(super) fn emit_commands(&self, live: &Live) {
        self.shared.emit(AgentEvent::CommandsUpdated {
            commands: catalog(live),
        });
    }
}

/// The merged catalog of the CLI's commands.
pub(super) fn catalog(live: &Live) -> Vec<CommandSpec> {
    let entries: Vec<Value> = live
        .commands
        .iter()
        .map(|command| live_entry(command, live.skills.contains(&command.name)))
        .collect();

    ClaudeCatalog.merged(&entries)
}

/// `command` as the catalog reads it (see `harnesses/claude/catalog.rs`).
fn live_entry(command: &SlashCommand, skill: bool) -> Value {
    let mut entry = json!({
        "name": command.name,
        "description": command.description,
        "argumentHint": command.argument_hint,
    });

    if let Some(builtin) = command.builtin {
        entry["builtin"] = Value::Bool(builtin);
    }

    if skill {
        entry["source"] = Value::from("skill");
    }

    entry
}

/// The chat's model (see the module docs).
fn current_model(live: &Live) -> Option<ModelOption> {
    if let Some(value) = &live.model {
        return Some(match live.models.iter().find(|model| &model.value == value) {
            Some(model) => option(model),
            None => bare(value),
        });
    }

    let default = live.models.iter().find(|model| model.value == DEFAULT_MODEL);
    let known = live.running_model.as_deref().or(live.launched_model.as_deref());

    let Some(id) = known.or_else(|| default?.resolved_model.as_deref()) else {
        return default.map(option);
    };

    Some(named(live, id).map_or_else(|| bare(id), option))
}

/// The offered model `id` names (see the module docs).
fn named<'a>(live: &'a Live, id: &str) -> Option<&'a ModelInfo> {
    let models = &live.models;
    let resolves = |model: &&ModelInfo| model.resolved_model.as_deref() == Some(id);
    let alias = |model: &&ModelInfo| model.value != DEFAULT_MODEL && resolves(model);

    models
        .iter()
        .find(|model| model.value == id)
        .or_else(|| models.iter().find(alias))
        .or_else(|| models.iter().find(resolves))
}

/// True unless the chat's thinking level is one its model does not offer.
pub(super) fn thinking_offered(live: &Live) -> bool {
    live.thinking.as_ref().is_none_or(|level| levels(live).contains(level))
}

/// The thinking levels of the chat's model.
fn levels(live: &Live) -> Vec<String> {
    let Some(current) = current_model(live) else {
        return Vec::new();
    };

    let Some(model) = live.models.iter().find(|model| model.value == current.id) else {
        return Vec::new();
    };

    let off = (model.supports_thinking_off == Some(true)).then(|| OFF.to_owned());
    let efforts = if model.supports_effort {
        model.supported_effort_levels.clone()
    } else {
        Vec::new()
    };

    off.into_iter().chain(efforts).collect()
}

/// An offered model as the picker shows it.
fn option(model: &ModelInfo) -> ModelOption {
    ModelOption {
        provider: PROVIDER.to_owned(),
        id: model.value.clone(),
        name: model.display_name.clone().filter(|name| !name.is_empty()),
        reasoning: model.supports_effort,
        context_window: None,
        max_tokens: None,
        input: Vec::new(),
    }
}

/// A model known by its id only.
fn bare(id: &str) -> ModelOption {
    ModelOption {
        provider: PROVIDER.to_owned(),
        id: id.to_owned(),
        name: None,
        reasoning: false,
        context_window: None,
        max_tokens: None,
        input: Vec::new(),
    }
}
