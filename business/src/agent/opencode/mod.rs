//! The OpenCode chat engine: one `opencode serve` per chat, driven over loopback HTTP and its event
//! stream ([plan](../../../../docs/plans/opencode-native-chat.md)).
//!
//! - **Process:** the shared [`AgentProcess`] with a random per-launch server password (masked in
//!   every line and event) and SIGTERM on stop, since the server ignores stdin EOF.
//! - **Startup** (`startup.rs`): the listening line, health, the event stream, then the session
//!   (new, or the resumed one when it belongs to the project), the agents, models and commands.
//! - **Events** (`stream.rs`): one task reads the event stream and maps it
//!   ([`Mapper`]) under the chat's lock, so prompt bookkeeping and event order never interleave.
//! - **Actions:** prompts, steering, follow-ups and abort (`prompts.rs`, sent by `delivery.rs`),
//!   history (`history.rs`), model, thinking level and agent pickers read from the startup
//!   catalogs (`settings.rs`), dialogs (`dialogs.rs`) and session actions (`ops.rs`).

mod delivery;
mod dialogs;
mod history;
mod ops;
mod prompts;
mod settings;
mod startup;
mod stream;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::task::AbortHandle;

use super::io::Shared;
use super::process::AgentProcess;
use super::sink::PromptTaken;
use crate::error::Result;
use crate::model::{AgentAction, AgentEvent, AgentOption, AgentReply, ModelOption, SessionState};
use crate::opencode_server::wire::{ModelRef, ProviderModel, Settings, slash_command};
use crate::opencode_server::{Mapper, OpenCodeClient, SessionId};
use crate::process::ExitInfo;

/// The thinking level that sends no model variant.
const DEFAULT_VARIANT: &str = "default";

/// A running OpenCode session (see the module docs).
pub struct OpenCodeSession {
    process: AgentProcess,
    chat: Arc<Chat>,
    events: AbortHandle,
}

/// What the actions and the event task share.
struct Chat {
    shared: Arc<Shared>,
    client: OpenCodeClient,
    /// The session's directory as OpenCode stores it (directory-scoped routes).
    directory: String,
    catalog: Catalog,
    live: Mutex<Live>,
}

/// The models and agents the server offered at startup.
struct Catalog {
    models: Vec<ProviderModel>,
    agents: Vec<AgentOption>,
}

/// The chat's changing state.
struct Live {
    session: SessionId,
    mapper: Mapper,
    title: Option<String>,
    /// The model shown; `None` until OpenCode tells which it uses (`GET /config`, the resumed
    /// session, a `session.updated`).
    model: Option<ModelRef>,
    /// The user picked [`Live::model`]: prompts name it. Until then OpenCode picks (the session's
    /// last model or its default).
    model_chosen: bool,
    variant: Option<String>,
    agent: String,
    /// What the running turn was started with; steering goes with the same.
    turn: TurnSettings,
    /// Names of the server's commands (`GET /command`).
    commands: Vec<String>,
}

/// The agent, model and variant of a turn.
#[derive(Debug, Clone, Default)]
struct TurnSettings {
    agent: String,
    model: Option<ModelRef>,
    variant: Option<String>,
}

impl TurnSettings {
    fn settings(&self) -> Settings<'_> {
        Settings {
            agent: &self.agent,
            model: self.model.as_ref(),
            variant: self.variant.as_deref(),
        }
    }
}

impl Live {
    /// What the next prompt goes with; remembered as the turn's settings.
    fn next_turn(&mut self) -> &TurnSettings {
        self.turn = TurnSettings {
            agent: self.agent.clone(),
            model: self.model.clone().filter(|_| self.model_chosen),
            variant: self.variant.clone(),
        };

        &self.turn
    }

    /// True when `text` is `/name …` for one of the server's commands.
    fn is_command(&self, text: &str) -> bool {
        slash_command(text).is_some_and(|(name, _)| self.commands.iter().any(|known| known == name))
    }
}

impl Catalog {
    fn model(&self, model: &ModelRef) -> Option<&ProviderModel> {
        self.models.iter().find(|candidate| candidate.is(model))
    }

    /// The variants of `model` with [`DEFAULT_VARIANT`] first; empty without variants.
    fn levels(&self, model: Option<&ModelRef>) -> Vec<String> {
        let variants = model.and_then(|model| self.model(model)).map(|model| &model.variants);

        match variants {
            Some(variants) if !variants.is_empty() => std::iter::once(DEFAULT_VARIANT.to_owned())
                .chain(variants.iter().cloned())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The display fields of `model`; just its ids when the server did not list it.
    fn option(&self, model: &ModelRef) -> ModelOption {
        match self.model(model) {
            Some(known) => known.option.clone(),
            None => ModelOption {
                provider: model.provider.clone(),
                id: model.id.clone(),
                name: None,
                reasoning: false,
                context_window: None,
                max_tokens: None,
                input: Vec::new(),
            },
        }
    }
}

impl Chat {
    fn lock(&self) -> MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The session state from `live`.
    fn state(&self, live: &Live) -> SessionState {
        SessionState {
            session_id: live.session.to_string(),
            session_name: live.title.clone(),
            model: live.model.as_ref().map(|model| self.catalog.option(model)),
            thinking_level: self.thinking_level(live),
            agent: Some(live.agent.clone()),
            is_streaming: live.mapper.is_working(),
            is_compacting: false,
            message_count: 0,
            queued_count: 0,
            context_usage: None,
        }
    }

    fn thinking_level(&self, live: &Live) -> Option<String> {
        if let Some(variant) = &live.variant {
            return Some(variant.clone());
        }

        let levels = self.catalog.levels(live.model.as_ref());
        levels.into_iter().next()
    }
}

impl OpenCodeSession {
    /// OpenCode has no RPC handshake.
    pub fn protocol_version(&self) -> Option<u32> {
        None
    }

    /// How the process ended, once it has.
    pub fn exit_info(&self) -> Option<ExitInfo> {
        self.process.exit_info()
    }

    /// True until the process exited.
    pub fn is_running(&self) -> bool {
        self.process.is_running()
    }

    /// Runs `action` (validated first).
    pub async fn send(&self, action: AgentAction) -> Result<AgentReply> {
        use AgentAction as A;

        action.validate()?;
        self.process.ensure_running()?;

        match action {
            A::Prompt { .. }
            | A::Steer { .. }
            | A::FollowUp { .. }
            | A::RemoveQueued { .. }
            | A::Abort
            | A::GetHistory { .. } => self.prompt_action(action).await,
            A::GetState
            | A::GetAvailableModels
            | A::SetModel { .. }
            | A::GetAvailableThinkingLevels
            | A::SetThinkingLevel { .. }
            | A::GetAvailableAgents
            | A::SetAgent { .. }
            | A::RefreshCommands => self.setting(action).await,
            _ => self.session_op(action).await,
        }
    }

    /// Puts `event` into the session's stream, after the events already in it.
    pub fn emit(&self, event: AgentEvent) {
        self.chat.shared.emit(event);
    }

    /// Calls `taken` in the session's event order when the agent takes the next prompt.
    pub fn on_prompt_taken(&self, taken: PromptTaken) {
        self.chat.shared.on_prompt_taken(taken);
    }

    /// Stops reading events, then the server (see `AgentProcess::stop`), and waits until
    /// `exited` was delivered. Never calls `/global/dispose`.
    pub async fn stop(&self) {
        self.events.abort();
        self.process.stop().await;
    }
}

impl Drop for OpenCodeSession {
    fn drop(&mut self) {
        self.events.abort();
    }
}
