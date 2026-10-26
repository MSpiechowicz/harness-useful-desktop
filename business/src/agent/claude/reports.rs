//! What a [`ClaudeSession`] reports: the statistics, the history, the HTML export and the last
//! reply.
//!
//! - **Statistics:** the tokens and cost of this process (its `result` totals,
//!   [`ClaudeStats`](crate::claude_stream::stats::ClaudeStats)), the session's message counts (the
//!   saved ones it started with, then those since) and the context meter, asked fresh with
//!   `get_context_usage` (the newest turn's tokens when that fails).
//! - **History, export, last reply:** the session file, read as the Sessions view reads it
//!   ([`chat_messages`]), shared with the transcript cache: a history page copies only its own
//!   messages. A fork or clone whose own file Claude Code has not written yet shows what
//!   it copied, a new chat without a file nothing. The last reply is the newest of this process,
//!   else the newest saved one.
//! - **Export:** the shared HTML export ([`write_export`]) as `claude-<session id>.html`.

use std::path::Path;

use super::ops::blocking;
use super::{ClaudeSession, Origin};
use crate::agent::export::{Export, write_export};
use crate::claude_stream::stats::reply_text;
use crate::claude_stream::wire::control::ContextUsageReply;
use crate::claude_stream::wire::outbound::HostRequest;
use crate::error::{BusinessError, Result};
use crate::harnesses::claude::{SavedMessages, chat_messages, is_session_id, session_file};
use crate::model::{AgentReply, MessageRole};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;
use crate::pi_family::rpc::shared::page_history;

/// The title of an export of a session Claude Code has not named yet.
const UNTITLED: &str = "Claude Code session";

impl ClaudeSession {
    /// `getSessionStats` (see the module docs).
    pub(super) async fn stats(&self) -> Result<AgentReply> {
        let run = self.run();
        let chat = &run.chat;
        let asked = chat.control(&HostRequest::GetContextUsage, DEFAULT_TIMEOUT).await;

        let mut live = chat.lock();

        match asked.map(serde_json::from_value::<ContextUsageReply>) {
            Ok(Ok(reply)) => live.stats.on_context_usage(&reply),
            Ok(Err(error)) => tracing::debug!(%error, "Claude Code sent an unreadable context usage"),
            Err(error) => tracing::debug!(%error, "Claude Code reported no context usage"),
        }

        Ok(AgentReply::Stats {
            stats: live.stats.session_stats(live.counts),
        })
    }

    /// The history page ending before `cursor`.
    pub(super) async fn history(&self, cursor: Option<&str>, limit: u32) -> Result<AgentReply> {
        page_history(&self.messages().await?, cursor, limit)
    }

    /// `getLastAssistantText`.
    pub(super) async fn last_reply(&self) -> Result<AgentReply> {
        if let Some(text) = self.run().chat.lock().last_reply.clone() {
            return Ok(AgentReply::Text { text: Some(text) });
        }

        let messages = self.messages().await?;
        let text = messages
            .iter()
            .rev()
            .filter(|message| message.role == MessageRole::Assistant)
            .find_map(|message| reply_text(&message.blocks));

        Ok(AgentReply::Text { text })
    }

    /// `exportHtml` (see the module docs).
    pub(super) async fn export_html(&self) -> Result<AgentReply> {
        let run = self.run();
        let (session, title) = {
            let live = run.chat.lock();
            (live.session_id.clone(), live.title.clone())
        };

        let Some(session) = session else {
            return Err(BusinessError::Unsupported(
                "this chat has no saved session to export".to_owned(),
            ));
        };

        // The id names the export file.
        if !is_session_id(&session) {
            return Err(BusinessError::Internal("the chat's session id is not valid".to_owned()));
        }

        let messages = self.messages().await?;
        let export = Export {
            harness: "Claude Code",
            prefix: "claude",
            title: title.as_deref().unwrap_or(UNTITLED),
            session_id: &session,
            messages: &messages,
        };

        write_export(&run.chat.shared, self.launcher.options.export_dir.as_ref(), &export)
    }

    /// The session's saved messages (see the module docs).
    async fn messages(&self) -> Result<SavedMessages> {
        let (session, origin) = {
            let run = self.run();
            let live = run.chat.lock();
            (live.session_id.clone(), live.origin.clone())
        };

        let Some(session) = session.filter(|_| !self.launcher.helper) else {
            return Ok(SavedMessages::default());
        };

        let (dir, project) = (&self.launcher.sessions_dir, &self.launcher.project);

        match saved_messages(dir, project, &session, None).await {
            Err(BusinessError::NotFound(_)) => copied(dir, project, origin).await,
            read => read,
        }
    }
}

/// What a fork or clone copied from `origin`; nothing without one, or once it is gone.
async fn copied(dir: &Path, project: &Path, origin: Option<Origin>) -> Result<SavedMessages> {
    let Some(origin) = origin else {
        return Ok(SavedMessages::default());
    };

    match saved_messages(dir, project, &origin.session, origin.cut).await {
        Err(BusinessError::NotFound(_)) => Ok(SavedMessages::default()),
        read => read,
    }
}

/// The messages of saved session `id` in `dir`, the session folder of `project`, up to and with
/// `cut`.
pub(super) async fn saved_messages(dir: &Path, project: &Path, id: &str, cut: Option<String>) -> Result<SavedMessages> {
    let (dir, project) = (dir.to_path_buf(), project.to_path_buf());
    let id = id.to_owned();

    blocking(move || {
        let path = session_file(&dir, &project, &id)?;
        chat_messages(&path, cut.as_deref())
    })
    .await
}
