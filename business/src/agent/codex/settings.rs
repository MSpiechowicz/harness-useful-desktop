//! The state, pickers, statistics and commands of a [`CodexSession`](super::CodexSession).
//!
//! - **Models:** the ones `model/list` offered (read again when the picker asks, the cached list
//!   stays when that fails), by the id `turn/start` takes, under the thread's provider. The chat's
//!   model is the one it picked, else the one the thread started with (the user's configuration,
//!   which may be one Codex refuses: the turn's error says so and the picker lists Codex's default
//!   model with the others), else Codex's default. A pick applies from the next turn on.
//! - **Reasoning:** a model's levels are its `supportedReasoningEfforts`. The chat's level is the
//!   one it picked, else the thread's, else the model's default. A model change keeps a level the
//!   new model offers, else picks the new model's default.
//! - **Statistics:** the thread's token counts and context meter ([`CodexStats`]) with the
//!   chat's message counts.
//! - **Commands:** the catalog of the harness; the skills join it once listed (`skills.rs`).
//!
//! [`CodexStats`]: crate::codex_rpc::stats::CodexStats

use super::{Chat, CodexSession, Live};
use crate::codex_rpc::wire::outbound::Request;
use crate::codex_rpc::wire::results::{Model, Page};
use crate::error::{BusinessError, Result};
use crate::harness::CommandCatalog;
use crate::harnesses::codex::CodexCatalog;
use crate::model::{AgentAction, AgentEvent, AgentReply, CommandSpec, ModelOption, SessionState};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;

/// The provider the chat names Codex's models with until a thread names its own.
pub(super) const PROVIDER: &str = "openai";

impl CodexSession {
    /// Runs a state, picker or statistics action.
    pub(super) async fn setting(&self, action: AgentAction) -> Result<AgentReply> {
        let chat = &self.chat;

        match action {
            AgentAction::GetAvailableModels => chat.models().await,
            AgentAction::SetModel { model_id, .. } => chat.set_model(&model_id),
            AgentAction::SetThinkingLevel { level } => chat.set_effort(level),
            AgentAction::GetAvailableThinkingLevels => Ok(AgentReply::ThinkingLevels {
                levels: levels(&chat.lock()),
            }),
            AgentAction::GetState => Ok(AgentReply::State {
                state: chat.state(&chat.lock()),
            }),
            AgentAction::GetSessionStats => {
                let live = chat.lock();
                Ok(AgentReply::Stats {
                    stats: live.stats.session_stats(live.message_counts()),
                })
            }
            _ => Err(BusinessError::Internal("not a setting action".to_owned())),
        }
    }
}

impl Chat {
    /// `model/list`, falling back to the models offered before.
    async fn models(&self) -> Result<AgentReply> {
        let listed = self
            .request::<Page<Model>>(&Request::ModelList { include_hidden: false }, DEFAULT_TIMEOUT)
            .await;

        let mut live = self.lock();

        match listed {
            Ok(page) if !page.data.is_empty() => live.models = page.data,
            Ok(_) => {}
            Err(error) => tracing::debug!(%error, "Codex listed no models"),
        }

        Ok(AgentReply::Models {
            models: live.models.iter().map(|model| option(&live, model)).collect(),
        })
    }

    /// Picks one of the offered models for the next turns.
    fn set_model(&self, id: &str) -> Result<AgentReply> {
        let mut live = self.lock();

        if !live.models.iter().any(|model| model.model == id) {
            return Err(BusinessError::InvalidInput(format!("Codex offers no model {id}")));
        }

        live.model = Some(id.to_owned());
        self.shared.emit(AgentEvent::ModelChanged {
            model: current_model(&live),
        });

        let offered = levels(&live);
        let kept = current_effort(&live).is_none_or(|level| offered.contains(&level));

        if !kept {
            live.effort = picked_model(&live)
                .and_then(|model| model.default_reasoning_effort.clone())
                .or_else(|| offered.first().cloned());

            match live.effort.clone() {
                Some(level) => self.shared.emit(AgentEvent::ThinkingChanged { level }),
                None => self.emit_state(&live),
            }
        }

        Ok(AgentReply::Ack)
    }

    /// Picks one of the current model's reasoning levels for the next turns.
    fn set_effort(&self, level: String) -> Result<AgentReply> {
        let mut live = self.lock();

        if !levels(&live).contains(&level) {
            return Err(BusinessError::InvalidInput(format!(
                "the model has no reasoning level {level}"
            )));
        }

        live.effort = Some(level.clone());
        self.shared.emit(AgentEvent::ThinkingChanged { level });

        Ok(AgentReply::Ack)
    }

    /// The session state from `live`.
    pub(super) fn state(&self, live: &Live) -> SessionState {
        let queued = u32::try_from(live.queue.len()).unwrap_or(u32::MAX);

        SessionState {
            session_id: live.thread_id.clone().unwrap_or_default(),
            session_name: live.title.clone(),
            model: current_model(live),
            thinking_level: current_effort(live),
            agent: None,
            context_usage: live.stats.context().cloned(),
            message_count: live.message_counts().total,
            queued_count: queued,
            is_streaming: live.mapper.is_working(),
            is_compacting: live.mapper.is_compacting(),
        }
    }
}

/// The slash-command catalog.
pub(super) fn commands() -> Vec<CommandSpec> {
    CodexCatalog.merged(&[])
}

/// The id of the chat's model: its pick, else the thread's, else Codex's default.
fn model_id(live: &Live) -> Option<&str> {
    let default = || {
        live.models
            .iter()
            .find(|model| model.is_default)
            .map(|model| model.model.as_str())
    };

    live.model.as_deref().or(live.thread_model.as_deref()).or_else(default)
}

/// The offered entry of the chat's model.
fn picked_model(live: &Live) -> Option<&Model> {
    let id = model_id(live)?;

    live.models.iter().find(|model| model.model == id)
}

/// The chat's model (see the module docs).
fn current_model(live: &Live) -> Option<ModelOption> {
    let id = model_id(live)?;

    Some(match picked_model(live) {
        Some(model) => option(live, model),
        None => ModelOption {
            provider: live.provider.clone(),
            id: id.to_owned(),
            name: None,
            reasoning: false,
            context_window: None,
            max_tokens: None,
            input: Vec::new(),
        },
    })
}

/// The chat's reasoning level (see the module docs).
fn current_effort(live: &Live) -> Option<String> {
    let default = || picked_model(live).and_then(|model| model.default_reasoning_effort.clone());

    live.effort
        .clone()
        .or_else(|| live.thread_effort.clone())
        .or_else(default)
}

/// The reasoning levels of the chat's model.
fn levels(live: &Live) -> Vec<String> {
    picked_model(live)
        .map(|model| {
            model
                .supported_reasoning_efforts
                .iter()
                .map(|effort| effort.reasoning_effort.clone())
                .filter(|effort| !effort.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// An offered model as the picker shows it.
fn option(live: &Live, model: &Model) -> ModelOption {
    ModelOption {
        provider: live.provider.clone(),
        id: model.model.clone(),
        name: Some(model.display_name.clone()).filter(|name| !name.is_empty()),
        reasoning: !model.supported_reasoning_efforts.is_empty(),
        context_window: None,
        max_tokens: None,
        input: model.input_modalities.clone(),
    }
}
