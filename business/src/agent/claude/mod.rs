//! The Claude Code chat engine: one `claude` process per chat in stream-json mode, driven over
//! stdin/stdout with the control protocol ([plan](../../../../docs/plans/claude-code.md)). Chat
//! stays off in the harness's capabilities until the engine is complete.
//!
//! - **Process:** the shared [`AgentProcess`] (spawn, redacted stderr, `exited` last, stop on
//!   stdin EOF, kill on drop).
//! - **Startup** (`startup.rs`): spawn, the `initialize` control request (with the ask-all hook
//!   in Always ask mode), its reply within the startup limit (commands and models, never the
//!   account), then `ready`, `state` and `commandsUpdated`. A Yolo override is refused: the app
//!   never starts Claude Code's bypass mode.
//! - **Frames** (`io.rs`): one task reads stdout and maps every frame with the [`ClaudeMapper`]
//!   under the chat's lock, so prompt bookkeeping and event order never interleave. Replies to the
//!   app's control requests resolve the waiting action through the session's pending-request table
//!   (keyed by the app's `req_<n>` ids, each with a time limit). Requests the app does not handle are
//!   answered at once with an error. Permission prompts and questions become dialogs, answered with
//!   [`ClaudeSession::respond`]; the CLI's other asks are declined at once (`dialogs.rs`).
//! - **Actions:** prompts, steering, follow-ups, withdrawing a queued message and abort (which
//!   denies the open asks, `prompts.rs`); the state, the model and thinking pickers and the commands (`settings.rs`).
//!   New, resume, fork, clone, rename, compaction, statistics, export, history and `!command`s are
//!   not available yet and refused as unsupported.
//! - **Stop:** stdin closes, Claude Code exits (killed after the grace period).

mod dialogs;
mod io;
mod prompts;
mod settings;
mod startup;

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use self::dialogs::Dialogs;
use super::io::Shared;
use super::process::AgentProcess;
use super::sink::PromptTaken;
use crate::claude_stream::ClaudeMapper;
use crate::claude_stream::wire::control::{ModelInfo, SlashCommand};
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply, BashOutput, StreamingBehavior, UiResponse};
use crate::process::ExitInfo;

/// A running Claude Code chat (see the module docs).
pub struct ClaudeSession {
    process: AgentProcess,
    chat: Arc<Chat>,
}

/// What the actions and the reader task share.
struct Chat {
    shared: Arc<Shared>,
    live: Mutex<Live>,
}

/// The chat's changing state.
struct Live {
    mapper: ClaudeMapper,
    /// The session id, once known (the resumed one from the start, a new one from the first frame
    /// that names it).
    session_id: Option<String>,
    title: Option<String>,
    /// The models `initialize` (or the latest `list_models`) offered.
    models: Vec<ModelInfo>,
    /// The model the chat picked (a `set_model` value), `None` until it picks one.
    model: Option<String>,
    /// The model the latest `system/init` named.
    running_model: Option<String>,
    /// The thinking level the chat picked, `None` until it picks one (Claude Code does not report
    /// its effort).
    thinking: Option<String>,
    /// The slash commands the CLI reported last.
    commands: Vec<SlashCommand>,
    /// The names the CLI lists as skills.
    skills: Vec<String>,
    /// The permission mode the CLI reported last.
    permission_mode: Option<String>,
    /// Steering and follow-ups the CLI has not taken yet, oldest first.
    queue: Vec<Queued>,
    /// Prompts whose taking runs the prompt-taken call.
    awaiting_take: Vec<String>,
    /// What the CLI asks and nobody answered yet (an abort denies it), and the chat's own
    /// "Always allow" answers.
    dialogs: Dialogs,
    /// The text of the newest finished reply of this process.
    last_reply: Option<String>,
}

/// A queued user message.
struct Queued {
    uuid: String,
    text: String,
    queue: StreamingBehavior,
}

impl Live {
    fn new(project: &Path, resume: Option<&str>, ask_all: bool) -> Self {
        Self {
            mapper: ClaudeMapper::new(project),
            session_id: resume.map(str::to_owned),
            title: None,
            models: Vec::new(),
            model: None,
            running_model: None,
            thinking: None,
            commands: Vec::new(),
            skills: Vec::new(),
            permission_mode: None,
            queue: Vec::new(),
            awaiting_take: Vec::new(),
            dialogs: Dialogs::new(project, ask_all),
            last_reply: None,
        }
    }

    /// The queue as the chat shows it.
    fn queue_update(&self) -> AgentEvent {
        let texts = |queue: StreamingBehavior| {
            self.queue
                .iter()
                .filter(|queued| queued.queue == queue)
                .map(|queued| queued.text.clone())
                .collect()
        };

        AgentEvent::QueueUpdate {
            steering: texts(StreamingBehavior::Steer),
            follow_up: texts(StreamingBehavior::FollowUp),
            live_steered: 0,
        }
    }
}

impl Chat {
    fn lock(&self) -> MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl ClaudeSession {
    /// Claude Code has no RPC protocol version.
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
            A::Prompt { .. } | A::Steer { .. } | A::FollowUp { .. } | A::RemoveQueued { .. } | A::Abort => {
                self.prompt_action(action).await
            }
            A::GetState
            | A::GetAvailableModels
            | A::SetModel { .. }
            | A::GetAvailableThinkingLevels
            | A::SetThinkingLevel { .. }
            | A::RefreshCommands
            | A::GetLastAssistantText => self.setting(action).await,
            A::SwitchSession { .. } => Err(BusinessError::Unsupported(
                "Claude Code chats cannot switch sessions: open the session from the Sessions view".to_owned(),
            )),
            A::GetAvailableAgents | A::SetAgent { .. } => Err(BusinessError::Unsupported(
                "Claude Code chats have no agent picker".to_owned(),
            )),
            A::NewSession => Err(not_yet("start a new session")),
            A::Fork { .. } => Err(not_yet("fork")),
            A::Clone => Err(not_yet("be cloned")),
            A::Compact { .. } => Err(not_yet("compact")),
            A::SetSessionName { .. } => Err(not_yet("be renamed")),
            A::GetSessionStats => Err(not_yet("report statistics")),
            A::ExportHtml => Err(not_yet("export")),
            A::GetHistory { .. } => Err(not_yet("read the history")),
        }
    }

    /// Answers an open permission prompt or question (see `dialogs.rs`).
    pub async fn respond(&self, response: UiResponse) -> Result<()> {
        self.process.ensure_running()?;
        self.answer_dialog(response).await
    }

    /// Refuses: `!command`s are not available yet.
    pub async fn bash(&self, _command: &str) -> Result<BashOutput> {
        Err(not_yet("run commands"))
    }

    /// Refuses: Claude Code keeps a session in its project.
    pub async fn move_project(&self, _project: &Path, _sessions_dir: &Path) -> Result<()> {
        Err(BusinessError::Unsupported(
            "Claude Code cannot move a chat to another project".to_owned(),
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

    /// Stops the agent (see `AgentProcess::stop`) and waits until `exited` was delivered.
    pub async fn stop(&self) {
        self.process.stop().await;
    }
}

/// An action this engine does not offer yet.
fn not_yet(what: &str) -> BusinessError {
    BusinessError::Unsupported(format!("Claude Code chats cannot {what} yet"))
}
