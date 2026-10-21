//! What an [`OpenCodeV2Session`] reports from its session: the statistics, the last reply and the
//! HTML export ([`stats`](crate::opencode_server_v2::stats),
//! [`export_html`](crate::opencode_server::export_html)), as on OpenCode 1.
//!
//! - **Statistics:** every message of the session, its replies and compactions adding up the tokens
//!   and cost, as on OpenCode 1 (a fork or clone counts what it copied).
//! - **Context meter:** the `state`'s context usage comes from the newest step with output, which
//!   the event task notes as steps end; a resumed, forked or cloned session's from its newest
//!   reply (`startup.rs`, `ops.rs`).
//! - **Export:** every message, rendered escaped and redacted (the server password masked), written
//!   (`0600`, replaced in one rename) as `opencode-<session id>.html` in
//!   [`SessionOptions::export_dir`] (`0700`, also when it already exists), by default the
//!   `exports` folder in the app's data folder, never the project.
//!
//! [`SessionOptions::export_dir`]: crate::agent::SessionOptions::export_dir

use std::path::PathBuf;
use std::time::SystemTime;

use super::OpenCodeV2Session;
use super::history::all_messages;
use crate::agent::rfc3339;
use crate::app_dirs::exports_dir;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{ensure_private_dir, replace_atomically};
use crate::model::AgentReply;
use crate::opencode_server::export_html::{ExportPage, render};
use crate::opencode_server_v2::SessionId;
use crate::opencode_server_v2::mapper::history;
use crate::opencode_server_v2::messages::SessionMessage;
use crate::opencode_server_v2::stats::{last_reply_text, session_stats};
use crate::opencode_server_v2::wire::ModelRef;

/// The title of an export of a session OpenCode has not named yet.
const UNTITLED: &str = "OpenCode session";

impl OpenCodeV2Session {
    /// `getSessionStats` from every message.
    pub(super) async fn stats(&self) -> Result<AgentReply> {
        let chat = &self.chat;
        let messages = self.messages(&self.current_session()).await?;

        let window = |model: &ModelRef| chat.catalog.model(model).and_then(|known| known.option.context_window);

        Ok(AgentReply::Stats {
            stats: session_stats(&messages, &window),
        })
    }

    /// `getLastAssistantText`.
    pub(super) async fn last_reply(&self) -> Result<AgentReply> {
        let messages = self.messages(&self.current_session()).await?;

        Ok(AgentReply::Text {
            text: last_reply_text(&messages),
        })
    }

    /// `exportHtml` (see the module docs).
    pub(super) async fn export_html(&self) -> Result<AgentReply> {
        let chat = &self.chat;
        let (session, title) = {
            let live = chat.lock();
            (live.session.clone(), live.title.clone())
        };

        let messages = self.messages(&session).await?;
        let exported_at = rfc3339(SystemTime::now());
        let page = ExportPage {
            title: title.as_deref().unwrap_or(UNTITLED),
            harness: "OpenCode",
            session_id: session.as_str(),
            exported_at: &exported_at,
        };
        let html = chat.shared.mask(&render(&page, &history(&messages)), false);

        let dir = export_dir(chat.export_dir.as_ref())?;
        ensure_private_dir(&dir, "create the export folder")?;

        let path = dir.join(format!("opencode-{session}.html"));
        replace_atomically(&path, html.as_bytes(), "write the HTML export")?;

        Ok(AgentReply::Exported {
            path: path.to_string_lossy().into_owned(),
        })
    }

    /// Every message of `session`, oldest first.
    async fn messages(&self, session: &SessionId) -> Result<Vec<SessionMessage>> {
        let records = all_messages(&self.chat.client, session, &self.chat.directory).await?;

        Ok(records.iter().map(SessionMessage::parse).collect())
    }

    fn current_session(&self) -> SessionId {
        self.chat.lock().session.clone()
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
