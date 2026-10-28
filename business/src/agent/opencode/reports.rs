//! What an [`OpenCodeSession`] reports from its session's messages: the statistics, the context
//! meter, the last reply and the HTML export ([`stats`](crate::opencode_server::stats),
//! [`export_html`](crate::opencode_server::export_html)).
//!
//! - **Context meter:** the `state`'s context usage comes from the newest reply with output, which
//!   the event task notes as replies arrive ([`note_context`]). A resumed session's newest
//!   messages are read once, when the state is first asked for (a fork or clone reads its copies
//!   when the chat moves to it).
//! - **Export:** every message, rendered escaped and redacted (the server password masked), written
//!   (`0600`, replaced in one rename) as `opencode-<session id>.html` in
//!   [`SessionOptions::export_dir`] (`0700`, also when it already exists), by default the
//!   `exports` folder in the app's data folder, never the project: OMP and PI write theirs into
//!   the working directory, which would put the transcript in the repository.
//!
//! [`SessionOptions::export_dir`]: crate::agent::SessionOptions::export_dir

use std::path::PathBuf;
use std::time::SystemTime;

use super::history::all_messages;
use super::startup::get_json;
use super::{Live, OpenCodeSession};
use crate::agent::rfc3339;
use crate::app_dirs::exports_dir;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{ensure_private_dir, replace_atomically};
use crate::model::{AgentReply, ChatMessage};
use crate::opencode_server::export_html::{ExportPage, render};
use crate::opencode_server::mapper::history_messages;
use crate::opencode_server::stats::{context_mark, last_context, last_reply_text, session_stats};
use crate::opencode_server::wire::{Envelope, ModelRef};
use crate::opencode_server::{Route, SessionId};

/// Messages read for the context meter of a resumed session.
const CONTEXT_PAGE: u32 = 50;

/// The title of an export of a session OpenCode has not named yet.
const UNTITLED: &str = "OpenCode session";

impl OpenCodeSession {
    /// `getState`: the chat's state, with the context meter read first when it is not known yet.
    pub(super) async fn current_state(&self) -> Result<AgentReply> {
        let chat = &self.chat;
        let unread = {
            let live = chat.lock();
            (!live.context.loaded).then(|| live.session.clone())
        };

        if let Some(session) = unread {
            let route = Route::Messages {
                session: &session,
                limit: Some(CONTEXT_PAGE),
                before: None,
            };
            let page = get_json(&chat.client, route).await;
            let mut live = chat.lock();

            if live.session == session {
                live.context.loaded = true;

                if live.context.mark.is_none() {
                    let records = page.ok().and_then(|page| page.as_array().cloned()).unwrap_or_default();
                    live.context.mark = last_context(&records);
                }
            }
        }

        Ok(AgentReply::State {
            state: chat.state(&chat.lock()),
        })
    }

    /// `getSessionStats` from every message.
    pub(super) async fn stats(&self) -> Result<AgentReply> {
        let chat = &self.chat;
        let records = all_messages(chat, &self.current_session()).await?;
        let window = |model: &ModelRef| chat.catalog.model(model).and_then(|known| known.option.context_window);

        Ok(AgentReply::Stats {
            stats: session_stats(&records, &window),
        })
    }

    /// `getLastAssistantText`.
    pub(super) async fn last_reply(&self) -> Result<AgentReply> {
        let records = all_messages(&self.chat, &self.current_session()).await?;

        Ok(AgentReply::Text {
            text: last_reply_text(&records),
        })
    }

    /// `exportHtml` (see the module docs).
    pub(super) async fn export_html(&self) -> Result<AgentReply> {
        let chat = &self.chat;
        let (session, title, working) = {
            let live = chat.lock();
            (live.session.clone(), live.title.clone(), live.mapper.is_working())
        };

        let records = all_messages(chat, &session).await?;
        let messages: Vec<ChatMessage> = history_messages(&records, working);
        let exported_at = rfc3339(SystemTime::now());
        let page = ExportPage {
            title: title.as_deref().unwrap_or(UNTITLED),
            harness: "OpenCode",
            session_id: session.as_str(),
            exported_at: &exported_at,
        };
        let html = chat.shared.mask(&render(&page, &messages), false);

        let dir = export_dir(chat.export_dir.as_ref())?;
        ensure_private_dir(&dir, "create the export folder")?;

        let path = dir.join(format!("opencode-{session}.html"));
        replace_atomically(&path, html.as_bytes(), "write the HTML export")?;

        Ok(AgentReply::Exported {
            path: path.to_string_lossy().into_owned(),
        })
    }

    fn current_session(&self) -> SessionId {
        self.chat.lock().session.clone()
    }
}

/// Notes the context size a reply of the chat's session reports (`message.updated`).
pub(super) fn note_context(live: &mut Live, envelope: &Envelope) {
    if envelope.kind != "message.updated" || envelope.session_id() != Some(live.session.as_str()) {
        return;
    }

    if let Some(mark) = envelope.properties.get("info").and_then(context_mark) {
        live.context.mark = Some(mark);
    }
}

/// `configured`, or `exports` in the app's data folder.
fn export_dir(configured: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = configured {
        return Ok(dir.clone());
    }

    let env = Env::from_process()?;

    exports_dir(&env).ok_or_else(|| BusinessError::NotFound("the app's data folder".to_owned()))
}
