//! Session actions of an [`OpenCodeV2Session`] that change which session the chat works on, as on
//! OpenCode 1 (renaming: `settings.rs`).
//!
//! - **New chat** (`POST /api/session {agent, model}`, so it goes on with the chat's agent, model
//!   and thinking level), **fork** (`POST /api/session/{id}/fork {before}`, which copies the
//!   messages before that one, so the forked message itself is left out, as in PI) and **clone**
//!   (the same without `before`: every message) refuse while the agent works, as switching
//!   mid-turn would strand the turn. The chat then moves to the new session: a fresh [`Mapper`]
//!   (no turn, queue or subagents of the old one; follow-ups still held are dropped; a fork's or
//!   clone's copied messages are read first and never shown again, and the newest reply among them
//!   sizes the context), the new title, the agent and model the session has, and a fresh `state`
//!   (the manager rebinds the chat's session id). The old session stays as it is.
//! - A prompt that arrives while the new session is being prepared wins: the move is refused like
//!   one asked for while the agent works, and the new session is deleted (best effort), as is one
//!   the chat could not move to for another reason. A session OpenCode reports in another directory
//!   is refused and left alone: the chat cannot tell it is the one it asked for.
//! - A fork names an OpenCode message id (`msg_…`, the entry ids of the history and the Sessions
//!   view) of a message the user wrote. Like PI the reply carries no text: the app puts the forked
//!   message back into the composer from its history entry.
//! - **Switching** to another session inside a chat is not offered, **moving** a chat to another
//!   project not possible (`mod.rs`).

use serde::Serialize;
use serde_json::Value;

use super::history::{all_messages, last_context};
use super::{Chat, OpenCodeV2Session, normalized};
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, AgentReply};
use crate::opencode_server_v2::messages::SessionMessage;
use crate::opencode_server_v2::wire::{CreateBody, Envelope, ForkBody, Session};
use crate::opencode_server_v2::{Mapper, MessageId, RouteV2, SessionId};

impl OpenCodeV2Session {
    /// `POST /api/session` with the chat's agent and model, then the chat moves to the new session.
    pub(super) async fn new_session(&self) -> Result<AgentReply> {
        const ACTION: &str = "start a new chat";

        let chat = &self.chat;
        let body = {
            let live = chat.lock();

            if live.mapper.is_working() {
                return Err(working(ACTION));
            }

            CreateBody {
                title: None,
                agent: Some(live.agent.clone()),
                model: live.model.clone(),
            }
        };

        let route = RouteV2::SessionCreate {
            directory: &chat.directory,
        };
        let info = created(chat, route, &body).await?;
        self.switch_to(info, false, ACTION).await?;

        Ok(AgentReply::Ack)
    }

    /// Forks at the user's message `entry_id`: the new session holds the messages before it.
    pub(super) async fn fork(&self, entry_id: &str) -> Result<AgentReply> {
        const ACTION: &str = "fork the chat";

        let message = MessageId::parse(entry_id)?;
        let session = self.idle_session(ACTION)?;
        let records = all_messages(&self.chat.client, &session, &self.chat.directory).await?;

        let kind = records
            .iter()
            .find(|record| record.get("id").and_then(Value::as_str) == Some(message.as_str()))
            .map(|record| record.get("type").and_then(Value::as_str));

        match kind {
            Some(Some("user")) => {}
            Some(_) => {
                return Err(BusinessError::InvalidInput(
                    "a chat forks at a message you wrote".to_owned(),
                ));
            }
            None => return Err(BusinessError::NotFound(format!("message {message} in this chat"))),
        }

        let body = ForkBody { before: Some(message) };
        self.forked(&session, &body, ACTION).await
    }

    /// Duplicates the session with every message.
    pub(super) async fn clone_session(&self) -> Result<AgentReply> {
        const ACTION: &str = "clone the chat";

        let session = self.idle_session(ACTION)?;
        self.forked(&session, &ForkBody::default(), ACTION).await
    }

    /// The chat's session; refused with "the agent is working. Wait until it is done, then `action`"
    /// while it works.
    pub(super) fn idle_session(&self, action: &str) -> Result<SessionId> {
        let live = self.chat.lock();

        if live.mapper.is_working() {
            return Err(working(action));
        }

        Ok(live.session.clone())
    }

    /// `POST /api/session/{id}/fork` with `body`, then the chat moves to the new session.
    async fn forked(&self, session: &SessionId, body: &ForkBody, action: &str) -> Result<AgentReply> {
        let chat = &self.chat;
        let route = RouteV2::SessionFork {
            session,
            directory: &chat.directory,
        };
        let info = created(chat, route, body).await?;
        self.switch_to(info, true, action).await?;

        Ok(AgentReply::Ack)
    }

    /// Moves the chat to session `info`, which the server just created (see the module docs);
    /// `copied` when it starts with the messages of a fork or clone, which are read first.
    /// Refused with `action` when a prompt came meanwhile.
    async fn switch_to(&self, info: Session, copied: bool, action: &str) -> Result<()> {
        let chat = &self.chat;
        let session = SessionId::parse(&info.id)
            .map_err(|_| BusinessError::Protocol("OpenCode sent an invalid session id".to_owned()))?;

        if info.location.directory != chat.directory {
            return Err(BusinessError::Protocol(
                "OpenCode put the new session in another directory".to_owned(),
            ));
        }

        let records = if copied {
            all_messages(&chat.client, &session, &chat.directory).await
        } else {
            Ok(Vec::new())
        };
        let moved = records.and_then(|records| {
            let messages: Vec<SessionMessage> = records.iter().map(SessionMessage::parse).collect();
            self.commit(session.clone(), info, &messages, action)
        });

        if moved.is_err() {
            delete_session(chat, &session).await;
        }

        moved
    }

    /// Moves the chat to `session` unless it works by now.
    fn commit(&self, session: SessionId, info: Session, messages: &[SessionMessage], action: &str) -> Result<()> {
        let chat = &self.chat;
        let mut live = chat.lock();

        if live.mapper.is_working() {
            return Err(working(action));
        }

        let held = live.mapper.has_follow_ups();

        let mut mapper = chat.mapper(&session);
        mapper.seen(messages);

        live.mapper = mapper;
        live.session = session;
        live.title = info.title;
        live.context = last_context(messages);
        live.picks.clear();
        live.mark = info.time.updated;

        if let Some(model) = info.model {
            live.model = Some(normalized(model));
        }

        let known = |agent: &String| chat.catalog.agents.iter().any(|option| option.id == *agent);

        if let Some(agent) = info.agent.filter(known) {
            live.agent = agent;
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
}

impl Chat {
    /// A fresh mapper for `session`.
    pub(super) fn mapper(&self, session: &SessionId) -> Mapper {
        Mapper::new(session.as_str(), &self.project, &self.root)
    }
}

/// Sends `route` (a session create or fork) with the JSON of `body`; the session it answers.
async fn created(chat: &Chat, route: RouteV2<'_>, body: &impl Serialize) -> Result<Session> {
    let body = serde_json::to_value(body)
        .map_err(|error| BusinessError::Internal(format!("an OpenCode request body: {error}")))?;
    let response = chat.client.send(route, Some(&body)).await?.error_for_status()?;

    Ok(response.json::<Envelope<Session>>()?.data)
}

/// `DELETE /api/session/{id}`, best effort: a session the chat created but could not move to.
async fn delete_session(chat: &Chat, session: &SessionId) {
    let route = RouteV2::SessionDelete {
        session,
        directory: &chat.directory,
    };
    let deleted = chat.client.send(route, None).await;

    if let Err(error) = deleted.and_then(|response| response.error_for_status()) {
        tracing::debug!(%error, "the OpenCode session the chat did not move to was not deleted");
    }
}

/// Why a session action waits: "the agent is working. Wait until it is done, then `action`".
pub(super) fn working(action: &str) -> BusinessError {
    BusinessError::Unsupported(format!("the agent is working. Wait until it is done, then {action}"))
}
