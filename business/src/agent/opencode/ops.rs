//! Session actions of an [`OpenCodeSession`] that change which session the chat works on, or its
//! name, and the dispatch of every session action.
//!
//! - **New chat** (`POST /session?directory=`), **fork** (`POST /session/{id}/fork {messageID}`,
//!   which copies the messages before that one, so the forked message itself is left out, as in
//!   PI) and **clone** (the same without `messageID`, every message) refuse while the agent works,
//!   as switching mid-turn would strand the turn. The chat then moves to the new session: a fresh
//!   [`Mapper`] (no turn, queue or subagents of the old one; follow-ups still held are dropped;
//!   a fork's or clone's copied messages are read first and never shown again), the new title,
//!   the same agent, model and thinking level, and a fresh `state` (the manager rebinds the chat's
//!   session id). An approval mode needs nothing: it is the server's configuration, for every
//!   session.
//! - A prompt that arrives while the new session is being prepared wins: the move is refused
//!   like one asked for while the agent works, and the new session is deleted (best effort), as
//!   is one the chat could not move to for another reason. A session OpenCode reports in another
//!   directory is refused and left alone: the chat cannot tell it is the one it asked for.
//! - A fork names an OpenCode message id (`msg_…`, the entry ids of the history and the Sessions
//!   view) of a message the user wrote; OpenCode itself would clone the whole session for an id it
//!   does not know.
//! - **Rename:** `PATCH /session/{id} {title}`; a changed title is a fresh `state`.
//! - **Switching** to another session inside a chat is not offered: OpenCode sessions open from the
//!   Sessions view. **Moving** a chat to another project is not possible: OpenCode keeps a session
//!   in the directory it was created in.

use std::path::Path;

use serde_json::{Value, json};

use super::approvals::delete_session;
use super::history::all_messages;
use super::startup::read_session;
use super::{Context, OpenCodeSession, TurnSettings};
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply};
use crate::opencode_server::stats::last_context;
use crate::opencode_server::wire::SessionInfo;
use crate::opencode_server::{MessageId, Route, SessionId};

impl OpenCodeSession {
    /// Runs a session action.
    pub(super) async fn session_op(&self, action: AgentAction) -> Result<AgentReply> {
        use AgentAction as A;

        match action {
            A::NewSession => self.new_session().await,
            A::Fork { entry_id } => self.fork(&entry_id).await,
            A::Clone => self.clone_session().await,
            A::SetSessionName { name } => self.rename(name).await,
            A::Compact { instructions } => self.compact(instructions.as_deref()).await,
            A::GetSessionStats => self.stats().await,
            A::ExportHtml => self.export_html().await,
            A::GetLastAssistantText => self.last_reply().await,
            A::SwitchSession { .. } => Err(BusinessError::Unsupported(
                "OpenCode chats cannot switch sessions: open the session from the Sessions view".to_owned(),
            )),
            A::Prompt { .. }
            | A::Steer { .. }
            | A::FollowUp { .. }
            | A::RemoveQueued { .. }
            | A::Abort
            | A::GetHistory { .. }
            | A::GetState
            | A::GetAvailableModels
            | A::SetModel { .. }
            | A::GetAvailableThinkingLevels
            | A::SetThinkingLevel { .. }
            | A::GetAvailableAgents
            | A::SetAgent { .. }
            | A::RefreshCommands => Err(BusinessError::Internal("not a session action".to_owned())),
        }
    }

    /// Refuses: OpenCode cannot move a session to another project.
    pub async fn move_project(&self, _project: &Path, _sessions_dir: &Path) -> Result<()> {
        Err(BusinessError::Unsupported(
            "OpenCode cannot move a chat to another project".to_owned(),
        ))
    }

    /// The chat's session; refused with "the agent is working. Wait until it is done, then `action`"
    /// while it works. Waits first for the `idle` of a `!command` or compaction that answered
    /// (`runs.rs`).
    pub(super) async fn idle_session(&self, action: &str) -> Result<SessionId> {
        self.chat.settle_answered_run().await;

        let live = self.chat.lock();

        if live.mapper.is_working() {
            return Err(working(action));
        }

        Ok(live.session.clone())
    }

    /// `POST /session?directory=`, then the chat moves to the new session.
    async fn new_session(&self) -> Result<AgentReply> {
        const ACTION: &str = "start a new chat";
        self.idle_session(ACTION).await?;

        let chat = &self.chat;
        let route = Route::SessionCreate {
            directory: &chat.directory,
        };
        let response = chat.client.send(route, Some(&json!({}))).await?.error_for_status()?;

        self.switch_to(read_session(&response.json()?)?, false, ACTION).await?;

        Ok(AgentReply::Ack)
    }

    /// Forks at the user's message `entry_id`: the new session holds the messages before it.
    async fn fork(&self, entry_id: &str) -> Result<AgentReply> {
        let message = MessageId::parse(entry_id)?;
        const ACTION: &str = "fork the chat";
        let session = self.idle_session(ACTION).await?;
        let records = all_messages(&self.chat, &session).await?;

        let role = records
            .iter()
            .map(|record| record.get("info").unwrap_or(&Value::Null))
            .find(|info| info.get("id").and_then(Value::as_str) == Some(message.as_str()))
            .and_then(|info| info.get("role").and_then(Value::as_str));

        match role {
            Some("user") => {}
            Some(_) => {
                return Err(BusinessError::InvalidInput(
                    "a chat forks at a message you wrote".to_owned(),
                ));
            }
            None => return Err(BusinessError::NotFound(format!("message {message} in this chat"))),
        }

        self.forked(&session, json!({ "messageID": message.as_str() }), ACTION)
            .await
    }

    /// Duplicates the session with every message.
    async fn clone_session(&self) -> Result<AgentReply> {
        const ACTION: &str = "clone the chat";
        let session = self.idle_session(ACTION).await?;

        self.forked(&session, json!({}), ACTION).await
    }

    /// `POST /session/{id}/fork` with `body`, then the chat moves to the new session.
    async fn forked(&self, session: &SessionId, body: Value, action: &str) -> Result<AgentReply> {
        let response = self
            .chat
            .client
            .send(Route::Fork { session }, Some(&body))
            .await?
            .error_for_status()?;

        self.switch_to(read_session(&response.json()?)?, true, action).await?;

        Ok(AgentReply::Ack)
    }

    /// Moves the chat to session `info`, which the server just created (see the module docs);
    /// `copied` when it starts with the messages of a fork or clone, which are read first: the
    /// chat must not show them again when their events arrive late, and the newest reply among
    /// them sizes the context. Refused with `action` when a prompt came meanwhile.
    async fn switch_to(&self, info: SessionInfo, copied: bool, action: &str) -> Result<()> {
        let chat = &self.chat;
        let session = SessionId::parse(&info.id)
            .map_err(|_| BusinessError::Protocol("OpenCode sent an invalid session id".to_owned()))?;

        if info.directory != chat.directory {
            return Err(BusinessError::Protocol(
                "OpenCode put the new session in another directory".to_owned(),
            ));
        }

        let records = if copied {
            all_messages(chat, &session).await
        } else {
            Ok(Vec::new())
        };
        let moved = records.and_then(|records| self.commit(session.clone(), info, &records, action));

        if moved.is_err() {
            delete_session(&chat.client, &session).await;
        }

        moved
    }

    /// Moves the chat to `session` unless it works by now.
    fn commit(&self, session: SessionId, info: SessionInfo, records: &[Value], action: &str) -> Result<()> {
        let chat = &self.chat;
        let mut live = chat.lock();

        if live.mapper.is_working() {
            return Err(working(action));
        }

        let held = live.mapper.has_follow_ups();

        live.mapper = chat.mapper(&session);
        live.mapper.seen(records);
        live.session = session;
        live.title = info.title;
        live.turn = TurnSettings::default();
        live.context = Context {
            mark: last_context(records),
            loaded: true,
        };

        if !live.model_chosen && info.model.is_some() {
            live.model = info.model;
            live.variant = info.variant;
        }

        if held {
            chat.shared.emit(AgentEvent::QueueUpdate {
                steering: Vec::new(),
                follow_up: Vec::new(),
                live_steered: 0,
            });
        }

        chat.shared.emit(AgentEvent::State {
            state: chat.state(&live),
        });

        Ok(())
    }

    /// `PATCH /session/{id} {title}`; a changed title is a fresh `state`.
    async fn rename(&self, name: String) -> Result<AgentReply> {
        let chat = &self.chat;
        let session = chat.lock().session.clone();
        let response = chat
            .client
            .send(
                Route::SessionPatch { session: &session },
                Some(&json!({ "title": name })),
            )
            .await?
            .error_for_status()?;
        let info = read_session(&response.json()?)?;

        let mut live = chat.lock();

        if live.session == session && live.title != info.title {
            live.title = info.title;
            chat.shared.emit(AgentEvent::State {
                state: chat.state(&live),
            });
        }

        Ok(AgentReply::Ack)
    }
}

/// Why a session action waits: "the agent is working. Wait until it is done, then `action`".
fn working(action: &str) -> BusinessError {
    BusinessError::Unsupported(format!("the agent is working. Wait until it is done, then {action}"))
}
