//! The state, pickers and name of an [`OpenCodeV2Session`]: answered from what startup loaded and
//! what the events told since.
//!
//! - OpenCode 2 keeps the model, its variant (the thinking level) and the agent on the session: a
//!   pick goes to `POST /api/session/{id}/model` or `/agent` at once and applies from the next
//!   model call. The chat shows the pick before the call and takes it back when OpenCode refuses
//!   it; the `session.*.selected` event that confirms it changes nothing, even when it arrives
//!   after a later pick.
//! - The agent changes only while the chat is idle, as on OpenCode 1. A model without the chosen
//!   variant falls back to the default one.
//! - A change OpenCode reports on its own (another client, the automatic title) is a fresh
//!   `state` (`stream.rs`).
//! - **Rename:** `PATCH /api/session/{id} {title}`; the `session.renamed` event that follows is the
//!   fresh `state`.

use serde::Serialize;

use super::catalog::DEFAULT_VARIANT;
use super::commands::Slash;
use super::{Chat, OpenCodeV2Session};
use crate::error::{BusinessError, Result};
use crate::harness::CommandCatalog;
use crate::harnesses::opencode::OpenCodeCatalog;
use crate::model::{AgentAction, AgentEvent, AgentReply};
use crate::opencode_server_v2::mapper::SessionChange;
use crate::opencode_server_v2::wire::{AgentBody, ModelBody, ModelRef, TitleBody};
use crate::opencode_server_v2::{RouteV2, SessionId};

impl OpenCodeV2Session {
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
            AgentAction::SetModel { provider, model_id } => {
                let model = ModelRef {
                    id: model_id,
                    provider_id: provider,
                    variant: None,
                };
                self.set_model(model).await
            }
            AgentAction::SetThinkingLevel { level } => self.set_thinking_level(&level).await,
            AgentAction::SetAgent { agent } => self.set_agent(agent).await,
            _ => self.refresh_commands().await,
        }
    }

    /// Picks the session's model; a variant it lacks falls back to the default.
    async fn set_model(&self, mut model: ModelRef) -> Result<AgentReply> {
        let chat = &self.chat;
        let Some(known) = chat.catalog.model(&model) else {
            return Err(BusinessError::InvalidInput(format!(
                "OpenCode offers no model {}/{}",
                model.provider_id, model.id
            )));
        };

        let (session, previous, dropped) = {
            let mut live = chat.lock();
            let variant = live.model.as_ref().and_then(|current| current.variant.clone());
            let dropped = variant
                .as_ref()
                .is_some_and(|variant| !known.variants.contains(variant));

            model.variant = variant.filter(|_| !dropped);
            let previous = live.model.replace(model.clone());
            live.picked(SessionChange::Model(model.clone()));
            (live.session.clone(), previous, dropped)
        };

        if let Err(error) = self.post_model(&session, &model).await {
            let mut live = chat.lock();
            live.model = previous;
            live.unpicked(&SessionChange::Model(model));
            return Err(error);
        }

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

    /// Picks the variant of the session's model (one of `getAvailableThinkingLevels`).
    async fn set_thinking_level(&self, level: &str) -> Result<AgentReply> {
        let chat = &self.chat;

        let (session, previous, model) = {
            let mut live = chat.lock();
            let levels = chat.catalog.levels(live.model.as_ref());

            let Some(current) = live.model.clone().filter(|_| levels.iter().any(|known| known == level)) else {
                return Err(BusinessError::InvalidInput(format!(
                    "the model has no thinking level {level}"
                )));
            };

            let model = ModelRef {
                variant: (level != DEFAULT_VARIANT).then(|| level.to_owned()),
                ..current.clone()
            };
            live.model = Some(model.clone());
            live.picked(SessionChange::Model(model.clone()));
            (live.session.clone(), current, model)
        };

        if let Err(error) = self.post_model(&session, &model).await {
            let mut live = chat.lock();
            live.model = Some(previous);
            live.unpicked(&SessionChange::Model(model));
            return Err(error);
        }

        chat.shared.emit(AgentEvent::ThinkingChanged {
            level: level.to_owned(),
        });

        Ok(AgentReply::Ack)
    }

    /// Picks the session's agent (one of `getAvailableAgents`), while idle.
    async fn set_agent(&self, agent: String) -> Result<AgentReply> {
        let chat = &self.chat;

        if !chat.catalog.agents.iter().any(|known| known.id == agent) {
            return Err(BusinessError::InvalidInput(format!("OpenCode has no agent {agent}")));
        }

        let (session, previous) = {
            let mut live = chat.lock();

            if live.mapper.is_working() {
                return Err(BusinessError::Unsupported(
                    "the agent is working. Wait until it is done, then switch agents".to_owned(),
                ));
            }

            let previous = std::mem::replace(&mut live.agent, agent.clone());
            live.picked(SessionChange::Agent(agent.clone()));
            (live.session.clone(), previous)
        };

        let route = RouteV2::SessionAgent {
            session: &session,
            directory: &chat.directory,
        };
        let body = AgentBody { agent: agent.clone() };

        if let Err(error) = post(chat, route, &body).await {
            let mut live = chat.lock();
            live.agent = previous;
            live.unpicked(&SessionChange::Agent(agent));
            return Err(error);
        }

        chat.shared.emit(AgentEvent::AgentChanged { agent });

        Ok(AgentReply::Ack)
    }

    /// Reads the server's commands and skills again and emits the merged catalog.
    async fn refresh_commands(&self) -> Result<AgentReply> {
        let chat = &self.chat;
        let slash = Slash::read(&chat.client, &chat.directory).await?;
        let commands = OpenCodeCatalog.merged(&slash.entries);

        {
            let mut live = chat.lock();
            live.commands = slash.commands;
            live.skills = slash.skills;
        }

        chat.shared.emit(AgentEvent::CommandsUpdated {
            commands: commands.clone(),
        });

        Ok(AgentReply::Commands { commands })
    }

    /// `PATCH /api/session/{id} {title}`; the rename event shows the new title.
    pub(super) async fn rename(&self, name: String) -> Result<AgentReply> {
        let chat = &self.chat;
        let session = chat.lock().session.clone();
        let route = RouteV2::SessionUpdate {
            session: &session,
            directory: &chat.directory,
        };

        post(chat, route, &TitleBody { title: name }).await?;

        Ok(AgentReply::Ack)
    }

    /// `POST /api/session/{id}/model`; the default variant is sent as none.
    async fn post_model(&self, session: &SessionId, model: &ModelRef) -> Result<()> {
        let chat = &self.chat;
        let route = RouteV2::SessionModel {
            session,
            directory: &chat.directory,
        };

        post(chat, route, &ModelBody { model: model.clone() }).await
    }
}

/// Sends `route` with the JSON of `body`; `Ok` for a `2xx` answer.
pub(super) async fn post(chat: &Chat, route: RouteV2<'_>, body: &impl Serialize) -> Result<()> {
    let body = serde_json::to_value(body)
        .map_err(|error| BusinessError::Internal(format!("an OpenCode request body: {error}")))?;

    chat.client.send(route, Some(&body)).await?.error_for_status()?;

    Ok(())
}
