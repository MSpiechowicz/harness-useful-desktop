//! The Codex chat engine: one `codex app-server` process per chat, JSON-RPC over stdin/stdout
//! ([plan](../../../../docs/plans/codex.md), "Protocol"). Chat is a preview.
//!
//! - **Process:** the shared [`AgentProcess`] (spawn, redacted stderr, `exited` last, stop on
//!   stdin EOF, kill on drop).
//! - **Startup** (`startup.rs`): spawn, the `initialize` → `initialized` handshake within the
//!   startup limit (Codex's version from its user agent), `model/list`, then `thread/start` in the
//!   project (or `thread/resume` without its turns), which the mapper follows from then on, with
//!   the chat's approval mode (`approvals.rs`; none without an override, so the user's
//!   `config.toml` applies). Then `ready`, `state`, `commandsUpdated` and, in a folder whose own
//!   settings Codex ignores, a notice; the skills and a resumed thread's saved messages follow
//!   (`skills.rs`, `history.rs`).
//! - **Frames** (`io.rs`): one task reads stdout (lines up to the frame limit) and maps every frame
//!   with the [`CodexMapper`] under the chat's lock, so bookkeeping and event order never
//!   interleave. Answers to the app's requests resolve the waiting action through the session's
//!   pending-request table (keyed by the request id's text, each with a time limit).
//! - **Actions:** prompts with images, steering, the app-side follow-up queue, withdrawing a
//!   queued message and abort (`prompts.rs`, images as files in `images.rs`); the state, the
//!   model and reasoning pickers and the statistics (`settings.rs`); new, switch, fork and clone,
//!   all threads of the same process with the chat's approval mode (`ops.rs`); renaming,
//!   compaction and `!command`s, refused in Always ask by [`codex_bash_allowed`] (`runs.rs`);
//!   history, the last reply and the HTML export (`history.rs`); skills as slash commands
//!   (`skills.rs`); the helper chat of AI naming (`naming.rs`). Codex's requests become dialogs,
//!   answered with [`CodexSession::respond`] (`dialogs.rs`); dynamic tool calls and token
//!   refreshes are refused.
//! - **Stop:** stdin closes, Codex exits (killed after the grace period).

mod approvals;
mod dialog_text;
mod dialogs;
mod forms;
mod history;
mod images;
mod io;
mod naming;
mod ops;
mod prompts;
mod runs;
mod settings;
mod skills;
mod startup;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

pub use self::approvals::codex_bash_allowed;

use self::dialogs::Dialogs;
use self::runs::Shell;
use super::io::Shared;
use super::process::AgentProcess;
use super::sink::PromptTaken;
use crate::codex_rpc::CodexMapper;
use crate::codex_rpc::stats::{CodexStats, MessageCounts};
use crate::codex_rpc::wire::items::UserInput;
use crate::codex_rpc::wire::results::{Model, Skill};
use crate::error::{BusinessError, Result};
use crate::harnesses::codex::sessions::Transcript;
use crate::model::{AgentAction, AgentEvent, AgentReply, ApprovalMode, BashOutput, StreamingBehavior, UiResponse};
use crate::process::ExitInfo;

/// A running Codex chat (see the module docs).
pub struct CodexSession {
    process: AgentProcess,
    chat: Arc<Chat>,
}

/// What the actions and the reader task share.
struct Chat {
    shared: Arc<Shared>,
    /// The project every thread of the chat runs in.
    project: PathBuf,
    /// Where images are saved for Codex to read (`None`: the app's data folder).
    attachments_dir: Option<PathBuf>,
    /// Where HTML exports are written (`None`: the app's data folder).
    export_dir: Option<PathBuf>,
    /// A helper chat (AI naming): an ephemeral thread without session operations.
    helper: bool,
    /// One session operation at a time.
    moving: tokio::sync::Mutex<()>,
    /// The chat's approval mode, sent with its threads (`approvals.rs`); `None` keeps the user's.
    approval_override: Option<ApprovalMode>,
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
    /// The approval mode the thread runs in (`approvals.rs`).
    mode: Option<ApprovalMode>,
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
    /// The messages of the chat since it entered its thread.
    counts: MessageCounts,
    /// The turns the chat ran since it entered its thread, which `counts` holds already.
    run_turns: Vec<String>,
    /// The messages the thread was saved with when the chat entered it (counted in the
    /// background, `history.rs`).
    saved_counts: MessageCounts,
    /// The thread has saved turns (resumed, forked, cloned, or one ran).
    saved: bool,
    /// The newest read of the thread's saved messages, which later history pages reuse.
    transcript: Option<Arc<Transcript>>,
    /// The enabled skills `skills/list` named, offered as slash commands.
    skills: Vec<Skill>,
    /// The running `!command`.
    shell: Option<Shell>,
    /// A helper chat's prompt, sent once more on Codex's default model when its turn fails.
    naming: Option<String>,
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
            mode: None,
            turn_id: None,
            turn_requests: Vec::new(),
            abort_pending: false,
            queue: Vec::new(),
            dialogs: Dialogs::new(project, helper),
            last_reply: None,
            stats: CodexStats::default(),
            counts: MessageCounts::default(),
            run_turns: Vec::new(),
            saved_counts: MessageCounts::default(),
            saved: false,
            transcript: None,
            skills: Vec::new(),
            shell: None,
            naming: None,
        }
    }

    /// The thread's messages: the saved ones it was entered with and those since.
    fn message_counts(&self) -> MessageCounts {
        let (saved, since) = (self.saved_counts, self.counts);

        MessageCounts {
            user: saved.user + since.user,
            assistant: saved.assistant + since.assistant,
            tool_calls: saved.tool_calls + since.tool_calls,
            total: saved.total + since.total,
        }
    }

    /// True while a turn, a compaction, an ask or a `!command` keeps the chat busy.
    fn is_busy(&self) -> bool {
        self.mapper.is_working() || !self.dialogs.is_empty() || self.shell.is_some()
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
            | A::GetSessionStats => self.setting(action).await,
            A::Prompt { .. } if self.chat.helper => self.helper_prompt(action).await,
            A::Prompt { .. } | A::Steer { .. } | A::FollowUp { .. } | A::RemoveQueued { .. } | A::Abort => {
                self.prompt_action(action).await
            }
            A::RefreshCommands => self.chat.refresh_commands().await,
            A::NewSession | A::SwitchSession { .. } | A::Fork { .. } | A::Clone => self.session_op(action).await,
            A::Compact { .. } | A::SetSessionName { .. } => self.thread_op(action).await,
            A::ExportHtml | A::GetHistory { .. } | A::GetLastAssistantText => self.report(action).await,
            A::GetAvailableAgents | A::SetAgent { .. } => Err(BusinessError::Unsupported(
                "Codex chats have no agent picker".to_owned(),
            )),
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

    /// Runs `command` in the thread's shell; its output reaches the model (see `runs.rs`). The
    /// caller must have the user's explicit confirmation.
    pub async fn bash(&self, command: &str) -> Result<BashOutput> {
        self.process.ensure_running()?;

        self.chat.run_command(command).await
    }

    /// Refuses: a Codex thread keeps its working directory.
    pub async fn move_project(&self, _project: &Path, _sessions_dir: &Path) -> Result<()> {
        let refusal = "Codex cannot move a chat to another project";

        Err(BusinessError::Unsupported(refusal.to_owned()))
    }
}
