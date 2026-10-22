//! The Claude Code chat engine: a `claude` process in stream-json mode, driven over stdin/stdout
//! with the control protocol ([plan](../../../../docs/plans/claude-code.md)). Chat is a preview.
//!
//! - **Process:** the shared [`AgentProcess`] (spawn, redacted stderr, `exited` last, stop on
//!   stdin EOF, kill on drop). A chat that moves to another session starts the CLI again: its
//!   events keep one stream across processes (`relay.rs`).
//! - **Startup** (`startup.rs`): spawn (a new chat names its session with `--session-id`), the
//!   `initialize` control request (with the ask-all hook in Always ask mode), its reply within the
//!   startup limit (commands and models, never the account), then `ready`, `state` and
//!   `commandsUpdated`. A Yolo override is refused: the app never starts Claude Code's bypass mode.
//!   A helper chat (AI naming) runs without tools and denies every ask.
//! - **Frames** (`io.rs`): one task reads stdout and maps every frame with the [`ClaudeMapper`]
//!   under the chat's lock, so prompt bookkeeping and event order never interleave. Replies to the
//!   app's control requests resolve the waiting action through the session's pending-request table
//!   (keyed by the app's `req_<n>` ids, each with a time limit). Requests the app does not handle are
//!   answered at once with an error. Permission prompts and questions become dialogs, answered with
//!   [`ClaudeSession::respond`]; the CLI's other asks are declined at once (`dialogs.rs`).
//! - **Actions:** prompts, steering, follow-ups, withdrawing a queued message and abort (which
//!   denies the open asks and stops a running `!command`, `prompts.rs`); the state, the model and
//!   thinking pickers, the commands and renaming (`settings.rs`); new, switch, fork and clone
//!   (`ops.rs`); `!command`s and compaction (`runs.rs`); statistics, history, export and the last
//!   reply (`reports.rs`).
//! - **Stop:** stdin closes, Claude Code exits (killed after the grace period).

mod dialogs;
mod io;
mod ops;
mod prompts;
mod relay;
mod reports;
mod runs;
mod settings;
mod startup;

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::watch;

use self::dialogs::Dialogs;
use self::relay::Port;
use self::runs::Shell;
use self::startup::Launcher;
use super::io::Shared;
use super::process::AgentProcess;
use super::sink::PromptTaken;
use crate::claude_stream::ClaudeMapper;
use crate::claude_stream::stats::{ClaudeStats, MessageCounts};
use crate::claude_stream::wire::control::{ModelInfo, SlashCommand};
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply, BashOutput, StreamingBehavior, UiResponse};
use crate::process::ExitInfo;

/// A running Claude Code chat (see the module docs).
pub struct ClaudeSession {
    /// The process the chat runs on now.
    current: Mutex<Arc<Run>>,
    /// What starting another process takes (boxed: the spec and options are large).
    launcher: Box<Launcher>,
    /// One session move at a time.
    moving: tokio::sync::Mutex<()>,
    /// Set by `stop`: a move under way gives up.
    stopped: watch::Sender<bool>,
}

/// One `claude` process of the chat.
struct Run {
    process: AgentProcess,
    chat: Arc<Chat>,
    port: Arc<Port>,
}

/// What the actions and the reader task of one process share.
struct Chat {
    shared: Arc<Shared>,
    live: Mutex<Live>,
}

/// The chat's changing state.
struct Live {
    mapper: ClaudeMapper,
    /// The session id: the one the chat named or resumed, then any other the frames name.
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
    /// Tokens, cost and the context meter of this process.
    stats: ClaudeStats,
    /// The session's messages: those it started with and those since.
    counts: MessageCounts,
    /// The session a fork or clone copied, shown until its own file exists.
    origin: Option<Origin>,
    /// Stops the running `!command`.
    shell: Option<Shell>,
    /// The `!command`s started so far (each run's number).
    shell_runs: u64,
    /// The chat moved on to another process: nothing more is sent here.
    retired: bool,
}

/// What a fork or clone copied: session `session`, up to the message `cut` when it is a fork.
#[derive(Debug, Clone)]
struct Origin {
    session: String,
    cut: Option<String>,
}

/// A queued user message.
struct Queued {
    uuid: String,
    text: String,
    queue: StreamingBehavior,
}

impl Live {
    fn new(project: &Path, session_id: Option<&str>, ask_all: bool, helper: bool) -> Self {
        Self {
            mapper: ClaudeMapper::new(project),
            session_id: session_id.map(str::to_owned),
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
            dialogs: Dialogs::new(project, ask_all, helper),
            last_reply: None,
            stats: ClaudeStats::default(),
            counts: MessageCounts::default(),
            origin: None,
            shell: None,
            shell_runs: 0,
            retired: false,
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

    /// True while a turn, a compaction, an ask or a `!command` keeps the chat busy.
    fn is_busy(&self) -> bool {
        self.mapper.is_working() || !self.dialogs.is_empty() || self.shell.is_some()
    }
}

impl Chat {
    fn lock(&self) -> MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl ClaudeSession {
    /// The process the chat runs on now.
    fn run(&self) -> Arc<Run> {
        self.current.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Claude Code has no RPC protocol version.
    pub fn protocol_version(&self) -> Option<u32> {
        None
    }

    /// How the process ended, once it has.
    pub fn exit_info(&self) -> Option<ExitInfo> {
        self.run().process.exit_info()
    }

    /// True until the process exited.
    pub fn is_running(&self) -> bool {
        self.run().process.is_running()
    }

    /// Runs `action` (validated first).
    pub async fn send(&self, action: AgentAction) -> Result<AgentReply> {
        use AgentAction as A;

        action.validate()?;
        self.run().process.ensure_running()?;

        match action {
            A::Prompt { .. } | A::Steer { .. } | A::FollowUp { .. } | A::RemoveQueued { .. } | A::Abort => {
                self.prompt_action(action).await
            }
            A::GetState
            | A::GetAvailableModels
            | A::SetModel { .. }
            | A::GetAvailableThinkingLevels
            | A::SetThinkingLevel { .. }
            | A::SetSessionName { .. }
            | A::RefreshCommands => self.setting(action).await,
            A::NewSession => self.new_session().await,
            A::SwitchSession { session } => self.switch_session(&session).await,
            A::Fork { entry_id } => self.fork(&entry_id).await,
            A::Clone => self.clone_session().await,
            A::Compact { instructions } => self.compact(instructions.as_deref()).await,
            A::GetSessionStats => self.stats().await,
            A::ExportHtml => self.export_html().await,
            A::GetHistory { cursor, limit } => self.history(cursor.as_deref(), limit).await,
            A::GetLastAssistantText => self.last_reply().await,
            A::GetAvailableAgents | A::SetAgent { .. } => Err(BusinessError::Unsupported(
                "Claude Code chats have no agent picker".to_owned(),
            )),
        }
    }

    /// Answers an open permission prompt or question (see `dialogs.rs`).
    pub async fn respond(&self, response: UiResponse) -> Result<()> {
        let run = self.run();
        run.process.ensure_running()?;

        run.chat.answer_dialog(response).await
    }

    /// Runs `command` in the project and hands its output to the model (see `runs.rs`). The
    /// caller must have the user's explicit confirmation.
    pub async fn bash(&self, command: &str) -> Result<BashOutput> {
        self.run_command(command).await
    }

    /// Refuses: Claude Code keeps a session in its project.
    pub async fn move_project(&self, _project: &Path, _sessions_dir: &Path) -> Result<()> {
        Err(BusinessError::Unsupported(
            "Claude Code cannot move a chat to another project".to_owned(),
        ))
    }

    /// Puts `event` into the session's stream, after the events already in it.
    pub fn emit(&self, event: AgentEvent) {
        self.run().chat.shared.emit(event);
    }

    /// Calls `taken` in the session's event order when the agent takes the next prompt.
    pub fn on_prompt_taken(&self, taken: PromptTaken) {
        self.run().chat.shared.on_prompt_taken(taken);
    }

    /// Stops the agent (see `AgentProcess::stop`) and waits until `exited` was delivered; a running
    /// `!command` stops too, and a move under way gives up.
    pub async fn stop(&self) {
        self.stopped.send_replace(true);

        let run = self.run();
        let shell = run.chat.lock().shell.take();

        if let Some(shell) = shell {
            shell.stop();
        }

        run.process.stop().await;
    }
}
