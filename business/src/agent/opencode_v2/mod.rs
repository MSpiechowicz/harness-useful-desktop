//! The OpenCode 2 chat engine: one private `opencode serve` per chat, driven over its `/api`
//! routes and event stream ([plan](../../../../docs/plans/opencode-v2.md)), with the observable
//! behaviour of the OpenCode 1 engine ([`super::opencode`]).
//!
//! - **Process:** the shared [`AgentProcess`] with a random per-launch server password (masked in
//!   every line and event) and SIGTERM on stop, since the server ignores stdin EOF.
//! - **Startup** (`startup.rs`): the listening line, `/api/info` readiness, the event stream, then
//!   the session (new, or the resumed one when it belongs to the project, its messages marked as
//!   seen), the agents and models (`catalog.rs`), and the commands and skills (`commands.rs`).
//! - **Events** (`stream.rs`): one task reads the event stream and maps it ([`Mapper`]) under the
//!   chat's lock, so prompt bookkeeping and event order never interleave. A stream that ends while
//!   the server runs is opened again and the chat catches up from the session (`catch_up.rs`).
//! - **Actions:** prompts, steering, follow-ups and abort (`prompts.rs`, sent by `delivery.rs`),
//!   history (`history.rs`), the model, thinking level and agent pickers and renaming
//!   (`settings.rs`), answers to approvals and forms (`dialogs.rs`), new chat, fork and clone
//!   (`ops.rs`), `!command`s and compaction (`runs.rs`), and statistics, the last reply and the HTML
//!   export (`reports.rs`).
//! - **Approval modes** (`approvals.rs`, `probe.rs`): configuration of the chat's own server, read
//!   from a first start without the mode. "Always allow" lasts as long as the chat. **Helper
//!   chats:** a session whose own rules deny everything.
//! - **Stop:** open dialogs close, a helper chat's session is deleted, then the server stops.

mod approvals;
mod catalog;
mod catch_up;
mod commands;
mod delivery;
mod dialogs;
mod history;
mod ops;
mod probe;
mod prompts;
mod reports;
mod runs;
mod settings;
mod startup;
mod stream;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::oneshot;
use tokio::task::AbortHandle;

use self::approvals::{Allowed, LaunchRules};
use self::catalog::Catalog;
use super::io::Shared;
use super::process::AgentProcess;
use super::sink::PromptTaken;
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply, ContextUsage, SessionState, UiResponse};
use crate::opencode_server::OpenCodeClient;
use crate::opencode_server::stats::usage;
use crate::opencode_server::wire::slash_command;
use crate::opencode_server_v2::mapper::{ContextNote, QuietKind, SessionChange, ShellResult};
use crate::opencode_server_v2::wire::ModelRef;
use crate::opencode_server_v2::{Mapper, SessionId};
use crate::process::ExitInfo;

/// A running OpenCode 2 session (see the module docs).
pub struct OpenCodeV2Session {
    process: AgentProcess,
    chat: Arc<Chat>,
    events: AbortHandle,
}

/// What the actions and the event task share.
struct Chat {
    shared: Arc<Shared>,
    client: OpenCodeClient,
    /// The session's directory as OpenCode stores it (the `location[directory]` of its routes).
    directory: String,
    /// The project, canonical.
    project: PathBuf,
    /// The session's directory, canonical (inside the project).
    root: PathBuf,
    /// Where HTML exports go (`SessionOptions::export_dir`).
    export_dir: Option<PathBuf>,
    catalog: Catalog,
    /// The approval mode and whether this is a helper chat.
    rules: LaunchRules,
    live: Mutex<Live>,
}

/// The chat's changing state.
struct Live {
    session: SessionId,
    mapper: Mapper,
    title: Option<String>,
    /// The session's model; the variant is `None` for the default one ([`normalized`]).
    model: Option<ModelRef>,
    agent: String,
    /// Names of the server's commands (`GET /api/command`).
    commands: Vec<String>,
    /// Ids of the skills a `/name` runs (`GET /api/skill`, `commands.rs`).
    skills: Vec<String>,
    /// What the newest finished step or compaction told about the context's size.
    context: Option<ContextNote>,
    /// The chat's own model and agent picks OpenCode has not confirmed yet, oldest first.
    picks: Vec<SessionChange>,
    /// The server time (milliseconds) of the newest event read, at least the session's last
    /// change at startup: a catch-up takes the session's ends after it as missed.
    mark: u64,
    /// Where the running `!command`'s output goes, with its quiet run's generation (`runs.rs`).
    shell: Option<(u64, oneshot::Sender<ShellResult>)>,
    /// What the chat's "Always allow" answers allow.
    allowed: Allowed,
}

/// Picks kept waiting for OpenCode's confirmation at most.
const MAX_PICKS: usize = 8;

impl Live {
    /// Remembers `pick` (a model or agent the chat sent) until OpenCode confirms it.
    fn picked(&mut self, pick: SessionChange) {
        if self.picks.len() == MAX_PICKS {
            self.picks.remove(0);
        }

        self.picks.push(pick);
    }

    /// Forgets `pick`, which OpenCode refused.
    fn unpicked(&mut self, pick: &SessionChange) {
        if let Some(position) = self.picks.iter().rposition(|waiting| waiting == pick) {
            self.picks.remove(position);
        }
    }

    /// True when `change` confirms one of the chat's picks (the older ones are settled with it).
    fn confirms(&mut self, change: &SessionChange) -> bool {
        let Some(position) = self.picks.iter().position(|pick| pick == change) else {
            return false;
        };

        self.picks.drain(..=position);
        true
    }

    /// True when `text` is `/name …` for one of the server's commands or skills.
    fn is_command(&self, text: &str) -> bool {
        slash_command(text).is_some_and(|(name, _)| self.commands.iter().chain(&self.skills).any(|known| known == name))
    }
}

impl Chat {
    fn lock(&self) -> MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The session state from `live`.
    fn state(&self, live: &Live) -> SessionState {
        let quiet = live.mapper.quiet_kind();

        SessionState {
            session_id: live.session.to_string(),
            session_name: live.title.clone(),
            model: live.model.as_ref().map(|model| self.catalog.option(model)),
            thinking_level: self.thinking_level(live),
            agent: Some(live.agent.clone()),
            is_streaming: live.mapper.is_working() && quiet.is_none(),
            is_compacting: quiet == Some(QuietKind::Compaction),
            message_count: 0,
            queued_count: 0,
            context_usage: self.context_usage(live),
        }
    }

    /// The newest step's tokens against its model's context window.
    fn context_usage(&self, live: &Live) -> Option<ContextUsage> {
        let Some(ContextNote::Tokens { tokens, model }) = &live.context else {
            return None;
        };

        let model = model.as_ref().or(live.model.as_ref())?;
        let window = self.catalog.model(model).and_then(|known| known.option.context_window);

        usage(*tokens, window)
    }

    /// The model's variant, else its default level when it has variants.
    fn thinking_level(&self, live: &Live) -> Option<String> {
        let model = live.model.as_ref()?;

        if let Some(variant) = &model.variant {
            return Some(variant.clone());
        }

        self.catalog.levels(Some(model)).into_iter().next()
    }
}

/// `model` with the default variant written as none, as the chat keeps it.
fn normalized(mut model: ModelRef) -> ModelRef {
    model.variant = model.variant.filter(|variant| variant != catalog::DEFAULT_VARIANT);
    model
}

impl OpenCodeV2Session {
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

        let reply = match action {
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
            A::SetSessionName { name } => self.rename(name).await,
            A::SwitchSession { .. } => Err(BusinessError::Unsupported(
                "OpenCode chats cannot switch sessions: open the session from the Sessions view".to_owned(),
            )),
            A::NewSession => self.new_session().await,
            A::Fork { entry_id } => self.fork(&entry_id).await,
            A::Clone => self.clone_session().await,
            A::Compact { instructions } => self.compact(instructions.as_deref()).await,
            A::GetSessionStats => self.stats().await,
            A::ExportHtml => self.export_html().await,
            A::GetLastAssistantText => self.last_reply().await,
        };

        // Tools see the server password: whatever OpenCode stored never shows it.
        self.chat.shared.masked_reply(reply?)
    }

    /// Answers an open dialog (`dialogs.rs`).
    pub async fn respond(&self, response: UiResponse) -> Result<()> {
        self.process.ensure_running()?;

        self.answer_dialog(response).await
    }

    /// Refuses: OpenCode cannot move a session to another project.
    pub async fn move_project(&self, _project: &Path, _sessions_dir: &Path) -> Result<()> {
        Err(BusinessError::Unsupported(
            "OpenCode cannot move a chat to another project".to_owned(),
        ))
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
    /// `exited` was delivered.
    pub async fn stop(&self) {
        self.events.abort();

        let cancelled = self.chat.lock().mapper.cancel_asks();
        cancelled.into_iter().for_each(|event| self.chat.shared.emit(event));

        self.chat.delete_helper_session().await;
        self.process.stop().await;
    }
}

impl Drop for OpenCodeV2Session {
    fn drop(&mut self) {
        self.events.abort();
    }
}
