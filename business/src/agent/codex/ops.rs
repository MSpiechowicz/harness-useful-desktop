//! Session actions of a [`CodexSession`] that change which thread the chat works on. One
//! `codex app-server` serves any number of threads, so the chat stays on its process and moves to
//! another thread of it; the old thread stays as it is (its notifications are dropped from then
//! on).
//!
//! - **New chat:** `thread/start` in the project. **Switch:** `thread/resume` (without its turns)
//!   of a saved thread of the project (`thread/read` checks its folder first). **Fork** at a
//!   message (an entry id of the history or the Sessions view: a message's id is its turn's):
//!   `thread/fork{lastTurnId}` keeps the turns up to and including that turn, as
//!   [`fork_point`](crate::harnesses::codex::sessions::CodexSessions::fork_point) reads it; a turn
//!   the thread does not hold is `notFound`. **Clone:** `thread/fork` without a cut. Every new
//!   thread runs in the project with the user's `config.toml` ([`Chat::thread_settings`]).
//! - Each is refused while the chat is busy (a turn, a compaction, an ask or a `!command`), as
//!   switching mid-turn would strand the turn, and in a helper chat, whose thread is not saved.
//!   One runs at a time. A fork or clone needs a saved thread: one Codex ran a turn of.
//! - The move: a prompt that arrives while Codex prepares the new thread wins, and the move is
//!   refused. Otherwise the chat follows the new thread with a fresh mapper: no turn, no queue
//!   (held follow-ups are dropped), the new thread's title, model and effort (the chat's own picks
//!   stay), fresh statistics, then a `state` (the manager rebinds the chat's session id). A thread
//!   that starts with saved messages has them counted in the background (`history.rs`).
//! - Right after startup the chat lists its skills and, for a resumed thread, counts its saved
//!   messages, both in the background.

use std::sync::Arc;

use super::io::refused;
use super::startup::thread;
use super::{Chat, CodexSession, Live};
use crate::codex_rpc::CodexMapper;
use crate::codex_rpc::stats::{CodexStats, MessageCounts};
use crate::codex_rpc::wire::outbound::{Request, ThreadFork, ThreadResume, ThreadSettings, ThreadStart};
use crate::codex_rpc::wire::results::{ThreadRead, ThreadSession};
use crate::error::{BusinessError, Result};
use crate::harnesses::codex::is_thread_id;
use crate::model::{AgentAction, AgentReply, HarnessId, SessionRef};
use crate::opencode_server::mapper::project_dir;
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;

impl CodexSession {
    /// Runs a new chat, switch, fork or clone.
    pub(super) async fn session_op(&self, action: AgentAction) -> Result<AgentReply> {
        match action {
            AgentAction::NewSession => self.new_session().await,
            AgentAction::SwitchSession { session } => self.switch_session(&session).await,
            AgentAction::Fork { entry_id } => self.fork(&entry_id).await,
            AgentAction::Clone => self.clone_session().await,
            _ => Err(BusinessError::Internal("not a session action".to_owned())),
        }
    }

    /// Starts a new thread in the chat.
    async fn new_session(&self) -> Result<AgentReply> {
        let request = Request::ThreadStart(ThreadStart {
            settings: self.chat.thread_settings(),
            ..ThreadStart::default()
        });

        self.chat.move_to(&request, false, "start a new chat").await
    }

    /// Moves the chat to the saved thread `session` of the project.
    async fn switch_session(&self, session: &SessionRef) -> Result<AgentReply> {
        let chat = &self.chat;
        chat.saves_threads()?;

        if session.harness != HarnessId::Codex || !is_thread_id(&session.id) {
            return Err(BusinessError::InvalidInput("invalid session to switch to".to_owned()));
        }

        if chat.lock().thread_id.as_deref() == Some(session.id.as_str()) {
            return Ok(AgentReply::Ack);
        }

        chat.check_owned(&session.id).await?;

        let request = Request::ThreadResume(ThreadResume {
            thread_id: session.id.clone(),
            settings: chat.thread_settings(),
            exclude_turns: true,
        });

        chat.move_to(&request, true, "switch the session").await
    }

    /// Forks at message `entry_id` (see the module docs).
    async fn fork(&self, entry_id: &str) -> Result<AgentReply> {
        const ACTION: &str = "fork the chat";

        let chat = &self.chat;
        let source = chat.saved_thread(ACTION)?;
        let read = chat.read_thread(&source).await?;

        if !read.turns.iter().any(|turn| turn == entry_id) {
            return Err(BusinessError::NotFound(format!("message {entry_id} in this chat")));
        }

        let request = Request::ThreadFork(ThreadFork {
            thread_id: source,
            last_turn_id: Some(entry_id.to_owned()),
            settings: chat.thread_settings(),
            exclude_turns: true,
        });

        chat.move_to(&request, true, ACTION).await
    }

    /// Duplicates the thread with every turn.
    async fn clone_session(&self) -> Result<AgentReply> {
        const ACTION: &str = "clone the chat";

        let chat = &self.chat;
        let request = Request::ThreadFork(ThreadFork {
            thread_id: chat.saved_thread(ACTION)?,
            last_turn_id: None,
            settings: chat.thread_settings(),
            exclude_turns: true,
        });

        chat.move_to(&request, true, ACTION).await
    }
}

impl Chat {
    /// What every thread of the chat starts with: the project as its folder, the rest from the
    /// user's `config.toml`.
    pub(super) fn thread_settings(&self) -> ThreadSettings {
        ThreadSettings {
            cwd: Some(self.project.to_string_lossy().into_owned()),
            ..ThreadSettings::default()
        }
    }

    /// Right after startup: the skills, and the saved messages of a `resumed` thread, in the
    /// background. A helper chat needs neither.
    pub(super) fn opened(self: &Arc<Self>, resumed: bool) {
        if self.helper {
            return;
        }

        self.load_skills_later();

        if resumed {
            self.lock().saved = true;
            self.count_saved();
        }
    }

    /// Refuses in a helper chat, whose thread is not saved.
    pub(super) fn saves_threads(&self) -> Result<()> {
        if self.helper {
            return Err(BusinessError::Unsupported(
                "a helper chat has no saved session".to_owned(),
            ));
        }

        Ok(())
    }

    /// The thread a fork or clone copies: the chat's, once Codex saved a turn of it.
    fn saved_thread(&self, action: &str) -> Result<String> {
        self.saves_threads()?;

        let live = self.lock();

        live.thread_id.clone().filter(|_| live.saved).ok_or_else(|| {
            BusinessError::Unsupported(format!(
                "Codex has not saved this chat yet. Send a message first, then {action}"
            ))
        })
    }

    /// Checks that saved thread `id` belongs to the project (or a folder inside it).
    async fn check_owned(&self, id: &str) -> Result<()> {
        let request = Request::ThreadRead {
            thread_id: id.to_owned(),
        };
        let not_found = || BusinessError::NotFound(format!("session {id}"));

        let read = match self.call(&request, DEFAULT_TIMEOUT).await? {
            Ok(read) => serde_json::from_value::<ThreadRead>(read).map_err(|_| not_found())?,
            Err(error) if error.message.contains("not found") || error.message.contains("not loaded") => {
                return Err(not_found());
            }
            Err(error) => return Err(refused(request.method(), &error)),
        };

        if read.thread.id != id || project_dir(&read.thread.cwd, &self.project).is_none() {
            return Err(not_found());
        }

        Ok(())
    }

    /// Moves the chat to the thread `request` starts, resumes or forks (see the module docs);
    /// `saved`: it starts with saved turns.
    async fn move_to(self: &Arc<Self>, request: &Request, saved: bool, action: &str) -> Result<AgentReply> {
        self.saves_threads()?;

        let _moving = self.moving.lock().await;

        let sent = {
            let live = self.lock();

            if live.is_busy() {
                return Err(working(action));
            }

            live.counts.total
        };

        let session = thread(self, request, DEFAULT_TIMEOUT).await?;

        {
            let mut live = self.lock();

            if live.is_busy() || live.counts.total != sent {
                return Err(working(action));
            }

            self.enter(&mut live, session, saved);
        }

        if saved {
            self.count_saved();
        }

        Ok(AgentReply::Ack)
    }

    /// The chat follows thread `session` from now on (see the module docs).
    fn enter(&self, live: &mut Live, session: ThreadSession, saved: bool) {
        let held = !live.queue.is_empty();

        live.mapper = CodexMapper::new(&self.project);
        live.follow(session);
        live.turn_id = None;
        live.turn_requests.clear();
        live.abort_pending = false;
        live.queue.clear();
        live.last_reply = None;
        live.stats = CodexStats::default();
        live.counts = MessageCounts::default();
        live.saved = saved;
        live.transcript = None;

        if held {
            self.shared.emit(live.queue_update());
        }

        self.emit_state(live);
    }
}

impl Live {
    /// Follows thread `session` (started, resumed or forked): its id, title, model, effort and
    /// provider.
    pub(super) fn follow(&mut self, session: ThreadSession) {
        self.mapper.follow_thread(&session.thread.id);
        self.thread_id = Some(session.thread.id.clone()).filter(|id| !id.is_empty());
        self.title = session.thread.name.clone().filter(|name| !name.trim().is_empty());
        self.thread_model = Some(session.model).filter(|model| !model.is_empty());
        self.thread_effort = session.reasoning_effort;

        if !session.model_provider.is_empty() {
            self.provider = session.model_provider;
        }
    }
}

/// Why a session action waits: "the agent is working. Wait until it is done, then `action`".
pub(super) fn working(action: &str) -> BusinessError {
    BusinessError::Unsupported(format!("the agent is working. Wait until it is done, then {action}"))
}
