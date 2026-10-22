//! The Codex chat engine: one `codex app-server` process per chat, JSON-RPC over stdin/stdout
//! ([plan](../../../../docs/plans/codex.md), "Protocol"). The manager does not offer it yet.
//!
//! - **Process:** the shared [`AgentProcess`] (spawn, redacted stderr, `exited` last, stop on
//!   stdin EOF, kill on drop).
//! - **Startup** (`startup.rs`): spawn, the `initialize` → `initialized` handshake within the
//!   startup limit (Codex's version from its user agent), `model/list`, then `thread/start` in the
//!   project (or `thread/resume` without its turns), which the mapper follows from then on. No
//!   override is sent, so the user's `config.toml` applies. Then `ready`, `state` and
//!   `commandsUpdated`.
//! - **Frames** (`io.rs`): one task reads stdout (lines up to the frame limit) and maps every frame
//!   with the [`CodexMapper`] under the chat's lock, so bookkeeping and event order never
//!   interleave. Answers to the app's requests resolve the waiting action through the session's
//!   pending-request table (keyed by the request id's text, each with a time limit).
//! - **Actions:** prompts with images, steering, the app-side follow-up queue, withdrawing a
//!   queued message and abort (`prompts.rs`, images as files in `images.rs`); the state, the
//!   model and reasoning pickers and the statistics (`settings.rs`). Codex's requests become dialogs, answered with
//!   [`CodexSession::respond`] (`dialogs.rs`); dynamic tool calls and token refreshes are refused.
//! - New, switch, fork, clone, rename, compact, history, export and `!command`s are not available
//!   yet.
//! - **Stop:** stdin closes, Codex exits (killed after the grace period).

mod dialogs;
mod forms;
mod images;
mod io;
mod prompts;
mod settings;
mod startup;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use self::dialogs::Dialogs;
use super::io::Shared;
use super::process::AgentProcess;
use super::sink::PromptTaken;
use crate::codex_rpc::CodexMapper;
use crate::codex_rpc::stats::{CodexStats, MessageCounts};
use crate::codex_rpc::wire::items::UserInput;
use crate::codex_rpc::wire::results::Model;
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply, BashOutput, StreamingBehavior, UiResponse};
use crate::process::ExitInfo;

/// A running Codex chat (see the module docs).
pub struct CodexSession {
    process: AgentProcess,
    chat: Arc<Chat>,
}

/// What the actions and the reader task share.
struct Chat {
    shared: Arc<Shared>,
    /// Where images are saved for Codex to read (`None`: the app's data folder).
    attachments_dir: Option<PathBuf>,
    live: Mutex<Live>,
}

/// The chat's changing state.
struct Live {
    mapper: CodexMapper,
    /// The thread the chat runs, once started or resumed.
    thread_id: Option<String>,
    title: Option<String>,
    /// The Codex version its user agent names.
    codex_version: Option<String>,
    /// The provider the thread's models come from (`openai`).
    provider: String,
    /// The models `model/list` offered last.
    models: Vec<Model>,
    /// The model and reasoning effort the thread started with (the user's configuration).
    thread_model: Option<String>,
    thread_effort: Option<String>,
    /// The model and effort the chat picked, sent with every turn from then on.
    model: Option<String>,
    effort: Option<String>,
    /// The running turn as `turn/start` answered it (before its `turn/started`).
    turn_id: Option<String>,
    /// `turn/start` requests not answered yet: their id and whether a prompt action sent them.
    turn_requests: Vec<(String, bool)>,
    /// The user stopped a turn whose id was not known yet: interrupt it once it is.
    abort_pending: bool,
    /// Steering Codex has not taken yet and the app's follow-ups, oldest first.
    queue: Vec<Queued>,
    /// What Codex asks and nobody answered yet.
    dialogs: Dialogs,
    /// The text of the newest finished reply.
    last_reply: Option<String>,
    stats: CodexStats,
    counts: MessageCounts,
}

/// A queued user message.
struct Queued {
    /// The `clientUserMessageId` it is (or will be) sent with.
    client_id: String,
    text: String,
    input: Vec<UserInput>,
    queue: StreamingBehavior,
    /// Sent by a prompt action: its taking runs the prompt-taken call.
    prompt: bool,
}

impl Live {
    fn new(project: &Path, helper: bool) -> Self {
        Self {
            mapper: CodexMapper::new(project),
            thread_id: None,
            title: None,
            codex_version: None,
            provider: settings::PROVIDER.to_owned(),
            models: Vec::new(),
            thread_model: None,
            thread_effort: None,
            model: None,
            effort: None,
            turn_id: None,
            turn_requests: Vec::new(),
            abort_pending: false,
            queue: Vec::new(),
            dialogs: Dialogs::new(project, helper),
            last_reply: None,
            stats: CodexStats::default(),
            counts: MessageCounts::default(),
        }
    }

    /// The queue as the chat shows it.
    fn queue_update(&self) -> AgentEvent {
        let mut steering = Vec::new();
        let mut follow_up = Vec::new();

        for queued in &self.queue {
            match queued.queue {
                StreamingBehavior::Steer => steering.push(queued.text.clone()),
                StreamingBehavior::FollowUp => follow_up.push(queued.text.clone()),
            }
        }

        AgentEvent::QueueUpdate {
            steering,
            follow_up,
            live_steered: 0,
        }
    }

    /// True while follow-ups wait for the running turn to end.
    fn has_follow_ups(&self) -> bool {
        self.queue
            .iter()
            .any(|queued| queued.queue == StreamingBehavior::FollowUp)
    }

    /// The running turn: the one Codex reported, else the one `turn/start` answered.
    fn running_turn(&self) -> Option<String> {
        let reported = self.mapper.turn_id().map(str::to_owned);

        reported.or_else(|| self.turn_id.clone().filter(|_| self.mapper.is_working()))
    }
}

impl Chat {
    fn lock(&self) -> MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl CodexSession {
    /// Codex has no RPC protocol version.
    pub fn protocol_version(&self) -> Option<u32> {
        None
    }

    /// The Codex version the handshake named (`0.154.0`), when it named one.
    pub fn codex_version(&self) -> Option<String> {
        self.chat.lock().codex_version.clone()
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
            A::GetState
            | A::GetAvailableModels
            | A::SetModel { .. }
            | A::GetAvailableThinkingLevels
            | A::SetThinkingLevel { .. }
            | A::GetSessionStats
            | A::GetLastAssistantText
            | A::RefreshCommands => self.setting(action).await,
            A::Prompt { .. } | A::Steer { .. } | A::FollowUp { .. } | A::RemoveQueued { .. } | A::Abort => {
                self.prompt_action(action).await
            }
            A::GetAvailableAgents | A::SetAgent { .. } => Err(BusinessError::Unsupported(
                "Codex chats have no agent picker".to_owned(),
            )),
            A::NewSession
            | A::SwitchSession { .. }
            | A::Fork { .. }
            | A::Clone
            | A::Compact { .. }
            | A::SetSessionName { .. }
            | A::ExportHtml
            | A::GetHistory { .. } => Err(not_yet()),
        }
    }

    /// Answers an open dialog (see `dialogs.rs`).
    pub async fn respond(&self, response: UiResponse) -> Result<()> {
        self.process.ensure_running()?;

        self.chat.answer_dialog(response).await
    }

    /// Puts `event` into the session's stream, after the events already in it.
    pub fn emit(&self, event: AgentEvent) {
        self.chat.shared.emit(event);
    }

    /// Calls `taken` in the session's event order when the agent takes the next prompt.
    pub fn on_prompt_taken(&self, taken: PromptTaken) {
        self.chat.shared.on_prompt_taken(taken);
    }

    /// Stops the agent (see `AgentProcess::stop`) and waits until `exited` was delivered.
    pub async fn stop(&self) {
        self.process.stop().await;
    }

    /// Not available yet.
    pub async fn bash(&self, _command: &str) -> Result<BashOutput> {
        Err(not_yet())
    }

    /// Refuses: a Codex thread keeps its working directory.
    pub async fn move_project(&self, _project: &Path, _sessions_dir: &Path) -> Result<()> {
        let refusal = "Codex cannot move a chat to another project";

        Err(BusinessError::Unsupported(refusal.to_owned()))
    }
}

/// What the chat cannot do yet.
fn not_yet() -> BusinessError {
    BusinessError::Unsupported("Codex chats cannot do this yet".to_owned())
}
