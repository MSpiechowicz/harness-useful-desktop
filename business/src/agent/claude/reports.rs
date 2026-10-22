//! What a [`ClaudeSession`] reports: the statistics, the history, the HTML export and the last
//! reply.
//!
//! - **Statistics:** the tokens and cost of this process (its `result` totals,
//!   [`ClaudeStats`](crate::claude_stream::stats::ClaudeStats)), the session's message counts (the
//!   saved ones it started with, then those since) and the context meter, asked fresh with
//!   `get_context_usage` (the newest turn's tokens when that fails).
//! - **History, export, last reply:** the session file, read as the Sessions view reads it
//!   ([`chat_messages`]). A fork or clone whose own file Claude Code has not written yet shows what
//!   it copied, a new chat without a file nothing. The last reply is the newest of this process,
//!   else the newest saved one.
//! - **Export:** rendered escaped by the shared exporter ([`render`]) and written (`0600`, replaced
//!   in one rename) as `claude-<session id>.html` in
//!   [`SessionOptions::export_dir`](crate::agent::SessionOptions::export_dir) (`0700`), by default
//!   the `exports` folder in the app's data folder, never the project.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::ops::blocking;
use super::{ClaudeSession, Origin};
use crate::agent::rfc3339;
use crate::app_dirs::exports_dir;
use crate::claude_stream::stats::MessageCounts;
use crate::claude_stream::wire::control::ContextUsageReply;
use crate::claude_stream::wire::outbound::HostRequest;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{ensure_private_dir, replace_atomically};
use crate::harnesses::claude::{chat_messages, session_file};
use crate::model::{AgentReply, ChatBlock, ChatMessage, MessageRole};
use crate::opencode_server::export_html::{ExportPage, render};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;
use crate::pi_family::rpc::shared::slice_history;

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
        slice_history(self.messages().await?, cursor, limit)
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

        let messages = self.messages().await?;
        let exported_at = rfc3339(SystemTime::now());
        let page = ExportPage {
            title: title.as_deref().unwrap_or(UNTITLED),
            harness: "Claude Code",
            session_id: &session,
            exported_at: &exported_at,
        };
        let html = run.chat.shared.mask(&render(&page, &messages), false);

        let dir = export_dir(self.launcher.options.export_dir.as_ref())?;
        ensure_private_dir(&dir, "create the export folder")?;

        let path = dir.join(format!("claude-{session}.html"));
        replace_atomically(&path, html.as_bytes(), "write the HTML export")?;

        Ok(AgentReply::Exported {
            path: path.to_string_lossy().into_owned(),
        })
    }

    /// The session's saved messages (see the module docs).
    async fn messages(&self) -> Result<Vec<ChatMessage>> {
        let (session, origin) = {
            let run = self.run();
            let live = run.chat.lock();
            (live.session_id.clone(), live.origin.clone())
        };

        let Some(session) = session.filter(|_| !self.launcher.helper) else {
            return Ok(Vec::new());
        };

        let dir = &self.launcher.sessions_dir;

        match saved_messages(dir, &session, None).await {
            Err(BusinessError::NotFound(_)) => copied(dir, origin).await,
            read => read,
        }
    }
}

/// What a fork or clone copied from `origin`; nothing without one, or once it is gone.
async fn copied(dir: &Path, origin: Option<Origin>) -> Result<Vec<ChatMessage>> {
    let Some(origin) = origin else {
        return Ok(Vec::new());
    };

    match saved_messages(dir, &origin.session, origin.cut).await {
        Err(BusinessError::NotFound(_)) => Ok(Vec::new()),
        read => read,
    }
}

/// The messages of saved session `id` in the session folder `dir`, up to and with `cut`.
pub(super) async fn saved_messages(dir: &Path, id: &str, cut: Option<String>) -> Result<Vec<ChatMessage>> {
    let dir = dir.to_path_buf();
    let id = id.to_owned();

    blocking(move || {
        let path = session_file(&dir, &id)?;
        chat_messages(&path, cut.as_deref())
    })
    .await
}

/// The message counts of `messages`.
pub(super) fn counts(messages: &[ChatMessage]) -> MessageCounts {
    let mut counts = MessageCounts::default();

    for message in messages {
        counts.total += 1;

        match message.role {
            MessageRole::User => counts.user += 1,
            MessageRole::Assistant => {
                counts.assistant += 1;

                let calls = message
                    .blocks
                    .iter()
                    .filter(|block| matches!(block, ChatBlock::ToolCall { .. }))
                    .count();
                counts.tool_calls += u32::try_from(calls).unwrap_or(u32::MAX);
            }
            _ => {}
        }
    }

    counts
}

/// The text of a reply's `blocks`, when they have some.
pub(super) fn reply_text(blocks: &[ChatBlock]) -> Option<String> {
    let texts: Vec<&str> = blocks
        .iter()
        .filter_map(|block| match block {
            ChatBlock::Text { text } if !text.is_empty() => Some(text.as_str()),
            _ => None,
        })
        .collect();

    (!texts.is_empty()).then(|| texts.join("\n\n"))
}

/// `configured`, or `exports` in the app's data folder.
fn export_dir(configured: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = configured {
        return Ok(dir.clone());
    }

    let env = Env::from_process()?;

    exports_dir(&env).ok_or_else(|| BusinessError::NotFound("the app's data folder".to_owned()))
}
