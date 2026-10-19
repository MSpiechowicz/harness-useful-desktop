//! The state and pickers of an [`OpenCodeSession`]: answered from what startup loaded and what the
//! events told since. A chosen model, thinking level (model variant) or agent applies to the next
//! prompt, which carries all three; the agent changes only while the chat is idle, as OpenCode reads
//! the session's rules once per run.

use super::startup::{command_names, get_json};
use super::{DEFAULT_VARIANT, OpenCodeSession};
use crate::error::{BusinessError, Result};
use crate::harness::CommandCatalog;
use crate::harnesses::opencode::OpenCodeCatalog;
use crate::model::{AgentAction, AgentEvent, AgentReply};
use crate::opencode_server::Route;
use crate::opencode_server::wire::ModelRef;

impl OpenCodeSession {
    /// Runs a state or picker action.
    pub(super) async fn setting(&self, action: AgentAction) -> Result<AgentReply> {
        let chat = &self.chat;

        match action {
            AgentAction::GetState => Ok(AgentReply::State {
                state: chat.state(&chat.lock()),
            }),
            AgentAction::GetAvailableModels => Ok(AgentReply::Models {
                models: chat.catalog.models.iter().map(|model| model.option.clone()).collect(),
            }),
            AgentAction::GetAvailableThinkingLevels => Ok(AgentReply::ThinkingLevels {
                levels: chat.catalog.levels(chat.lock().model.as_ref()),
            }),
            AgentAction::GetAvailableAgents => Ok(AgentReply::Agents {
                agents: chat.catalog.agents.clone(),
            }),
            AgentAction::SetModel { provider, model_id } => self.set_model(ModelRef { provider, id: model_id }),
            AgentAction::SetThinkingLevel { level } => self.set_thinking_level(&level),
            AgentAction::SetAgent { agent } => self.set_agent(agent),
            _ => self.refresh_commands().await,
        }
    }

    /// Picks the model of the next prompt; a variant it lacks falls back to the default.
    fn set_model(&self, model: ModelRef) -> Result<AgentReply> {
        let chat = &self.chat;
        let Some(known) = chat.catalog.model(&model) else {
            return Err(BusinessError::InvalidInput(format!(
                "OpenCode offers no model {}/{}",
                model.provider, model.id
            )));
        };

        let mut live = chat.lock();
        let dropped = live
            .variant
            .take_if(|variant| !known.variants.contains(variant))
            .is_some();

        live.model = Some(model);
        live.model_chosen = true;
        chat.shared.emit(AgentEvent::ModelChanged {
            model: Some(known.option.clone()),
        });

        if dropped {
            chat.shared.emit(AgentEvent::ThinkingChanged {
                level: DEFAULT_VARIANT.to_owned(),
            });
        }

        Ok(AgentReply::Ack)
    }

    /// Picks the variant of the next prompt (one of `getAvailableThinkingLevels`).
    fn set_thinking_level(&self, level: &str) -> Result<AgentReply> {
        let chat = &self.chat;
        let mut live = chat.lock();

        if !chat
            .catalog
            .levels(live.model.as_ref())
            .iter()
            .any(|known| known == level)
        {
            return Err(BusinessError::InvalidInput(format!(
                "the model has no thinking level {level}"
            )));
        }

        live.variant = (level != DEFAULT_VARIANT).then(|| level.to_owned());
        chat.shared.emit(AgentEvent::ThinkingChanged {
            level: level.to_owned(),
        });

        Ok(AgentReply::Ack)
    }

    /// Picks the agent of the next prompt (one of `getAvailableAgents`), while idle.
    fn set_agent(&self, agent: String) -> Result<AgentReply> {
        let chat = &self.chat;

        if !chat.catalog.agents.iter().any(|known| known.id == agent) {
            return Err(BusinessError::InvalidInput(format!("OpenCode has no agent {agent}")));
        }

        let mut live = chat.lock();

        if live.mapper.is_working() {
            return Err(BusinessError::Unsupported(
                "the agent is working; switch agents once it is done".to_owned(),
            ));
        }

        live.agent = agent.clone();
        chat.shared.emit(AgentEvent::AgentChanged { agent });

        Ok(AgentReply::Ack)
    }

    /// Reads `GET /command` again and emits the merged catalog.
    async fn refresh_commands(&self) -> Result<AgentReply> {
        let chat = &self.chat;
        let raw = get_json(
            &chat.client,
            Route::Commands {
                directory: &chat.directory,
            },
        )
        .await?;
        let raw = raw.as_array().cloned().unwrap_or_default();
        let commands = OpenCodeCatalog.merged(&raw);

        chat.lock().commands = command_names(&raw);
        chat.shared.emit(AgentEvent::CommandsUpdated {
            commands: commands.clone(),
        });

        Ok(AgentReply::Commands { commands })
    }
}
