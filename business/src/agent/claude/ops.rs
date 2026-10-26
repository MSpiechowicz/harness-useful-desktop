//! Session actions of a [`ClaudeSession`] that change which session the chat works on. Claude Code
//! keeps one session per process, so each starts the CLI again (`startup.rs`) and the chat moves to
//! the new process.
//!
//! - **New chat:** a fresh `--session-id`. **Switch:** `--resume <id>` of a session of the project
//!   (the manager holds the id meanwhile). **Fork** at a message (an entry id of the history or the
//!   Sessions view: a line `uuid`): the turn it belongs to is kept, so the fork resumes at that
//!   turn's last line (`--resume-session-at`, see
//!   [`fork_point`](crate::harnesses::claude::fork_point)); a message before the last compaction
//!   is refused. **Clone:** `--fork-session` without a cut. The old session stays as it is.
//! - Each is refused while the agent works, asks or runs a `!command`, as switching mid-turn would
//!   strand the turn, and in a helper chat, which has no saved session. The new process keeps the
//!   chat's model pick (`--model`).
//! - The move: the new process starts (its events held back) while the old one keeps serving the
//!   chat. A prompt that arrives meanwhile wins: the move is refused and the new process stopped.
//!   So is a move whose old process exited meanwhile (its `exited` ended the chat). Otherwise the
//!   old process is retired (its later events, `exited` too, never reach the chat), the new one
//!   opens (`ready`, `state` with the new session id, `commandsUpdated`) and the old one stops. A
//!   stop of the chat gives a move under way up.

use std::path::PathBuf;
use std::sync::{Arc, PoisonError};

use super::startup::{Start, fresh_id};
use super::{ClaudeSession, Run};
use crate::claude_stream::wire::control::InitializeReply;
use crate::error::{BusinessError, Result};
use crate::harnesses::claude::{fork_point, is_session_id, session_file};
use crate::model::{AgentReply, HarnessId, SessionRef};

impl ClaudeSession {
    /// Starts a new session in the chat.
    pub(super) async fn new_session(&self) -> Result<AgentReply> {
        self.move_to(Start::fresh(), "start a new chat").await
    }

    /// Moves the chat to the saved session `session` of the project.
    pub(super) async fn switch_session(&self, session: &SessionRef) -> Result<AgentReply> {
        const ACTION: &str = "switch the session";

        if session.harness != HarnessId::Claude || !is_session_id(&session.id) {
            return Err(BusinessError::InvalidInput("invalid session to switch to".to_owned()));
        }

        if self.session_id()?.as_deref() == Some(session.id.as_str()) {
            return Ok(AgentReply::Ack);
        }

        self.saved_file(&session.id).await?;
        self.move_to(Start::Resume(session.id.clone()), ACTION).await
    }

    /// Forks at message `entry_id` (see the module docs).
    pub(super) async fn fork(&self, entry_id: &str) -> Result<AgentReply> {
        const ACTION: &str = "fork the chat";

        let source = self.saved_session(ACTION)?;
        let path = self.copied_file(&source, ACTION).await?;
        let entry = entry_id.to_owned();
        let at = blocking(move || fork_point(&path, &entry)).await?;

        let start = Start::Fork {
            source,
            id: fresh_id(),
            at: Some(at),
        };

        self.move_to(start, ACTION).await
    }

    /// Duplicates the session with every message.
    pub(super) async fn clone_session(&self) -> Result<AgentReply> {
        const ACTION: &str = "clone the chat";

        let source = self.saved_session(ACTION)?;
        self.copied_file(&source, ACTION).await?;

        let start = Start::Fork {
            source,
            id: fresh_id(),
            at: None,
        };

        self.move_to(start, ACTION).await
    }

    /// The chat's session id; refused in a helper chat.
    fn session_id(&self) -> Result<Option<String>> {
        self.saves_sessions()?;

        Ok(self.run().chat.lock().session_id.clone())
    }

    /// Refuses in a helper chat, which saves no session.
    fn saves_sessions(&self) -> Result<()> {
        if self.launcher.helper {
            return Err(BusinessError::Unsupported(
                "a helper chat has no saved session".to_owned(),
            ));
        }

        Ok(())
    }

    /// The session a fork or clone copies: the chat's, once Claude Code saved a message of it.
    fn saved_session(&self, action: &str) -> Result<String> {
        self.session_id()?.ok_or_else(|| nothing_saved(action))
    }

    /// The file of saved session `id`; `notFound` before Claude Code wrote it.
    async fn saved_file(&self, id: &str) -> Result<PathBuf> {
        let dir = self.launcher.sessions_dir.clone();
        let project = self.launcher.project.clone();
        let id = id.to_owned();

        blocking(move || session_file(&dir, &project, &id)).await
    }

    /// The file of session `id`, which a fork or clone copies; refused before Claude Code wrote it.
    async fn copied_file(&self, id: &str, action: &str) -> Result<PathBuf> {
        match self.saved_file(id).await {
            Err(BusinessError::NotFound(_)) => Err(nothing_saved(action)),
            found => found,
        }
    }

    /// Moves the chat to a new process for `start` (see the module docs).
    async fn move_to(&self, start: Start, action: &str) -> Result<AgentReply> {
        self.saves_sessions()?;

        let _moving = self.moving.lock().await;
        let old = self.run();
        old.process.ensure_running()?;

        let (model, carried, sent) = {
            let live = old.chat.lock();

            if live.is_busy() {
                return Err(working(action));
            }

            (live.model.clone(), live.launched_role.clone(), live.counts.total)
        };

        let mut stopped = self.stopped.subscribe();
        let abort = Box::pin(async move {
            stopped.wait_for(|stopped| *stopped).await.ok();
        });

        let (new, reply) = self.launcher.launch(&start, model.as_deref(), carried, abort).await?;

        if let Err(error) = self.commit(&old, &new, reply, sent, action) {
            new.process.stop().await;
            return Err(error);
        }

        old.process.stop().await;

        Ok(AgentReply::Ack)
    }

    /// Hands the chat from `old` to `new`, unless a stop came meanwhile, the old process exited
    /// (its `exited` ended the chat) or it got a message since it had `sent` (its count of the
    /// session's messages).
    fn commit(&self, old: &Run, new: &Arc<Run>, reply: InitializeReply, sent: u32, action: &str) -> Result<()> {
        let mut current = self.current.lock().unwrap_or_else(PoisonError::into_inner);

        if *self.stopped.borrow() {
            return Err(BusinessError::Process("the session was stopped".to_owned()));
        }

        {
            let mut live = old.chat.lock();

            if live.is_busy() || live.counts.total != sent {
                return Err(working(action));
            }

            if !old.port.retire_running(&old.process) {
                return Err(BusinessError::Process("the agent session has exited".to_owned()));
            }

            live.retired = true;
        }

        new.open(reply);
        *current = new.clone();

        Ok(())
    }
}

/// Runs the file read `read` off the async workers.
pub(super) async fn blocking<T: Send + 'static>(read: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(read)
        .await
        .map_err(|error| BusinessError::Internal(format!("a session file read failed: {error}")))?
}

/// Why a fork or clone has nothing to copy yet.
fn nothing_saved(action: &str) -> BusinessError {
    BusinessError::Unsupported(format!(
        "Claude Code has not saved this chat yet. Send a message first, then {action}"
    ))
}

/// Why a session action waits: "the agent is working. Wait until it is done, then `action`".
pub(super) fn working(action: &str) -> BusinessError {
    BusinessError::Unsupported(format!("the agent is working. Wait until it is done, then {action}"))
}
