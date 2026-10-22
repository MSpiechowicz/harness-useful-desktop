//! The HTML export of the engines that render it themselves from the saved messages (Claude Code,
//! Codex): rendered escaped by the shared exporter ([`render`]), the chat's secret masked, written
//! (`0600`, replaced in one rename) as `<prefix>-<session id>.html` in
//! [`SessionOptions::export_dir`](super::SessionOptions::export_dir) (`0700`), by default the
//! `exports` folder in the app's data folder, never the project.

use std::path::PathBuf;
use std::time::SystemTime;

use super::io::Shared;
use super::rfc3339;
use crate::app_dirs::exports_dir;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{ensure_private_dir, replace_atomically};
use crate::model::{AgentReply, ChatMessage};
use crate::opencode_server::export_html::{ExportPage, render};

/// What an export shows and where it goes.
pub(super) struct Export<'a> {
    /// The harness's name on the page (`Codex`).
    pub harness: &'a str,
    /// The start of the file name (`codex`); the session id follows. Both must be safe in a path.
    pub prefix: &'a str,
    pub title: &'a str,
    pub session_id: &'a str,
    pub messages: &'a [ChatMessage],
}

/// Renders and writes `export` (see the module docs); the reply names the file.
pub(super) fn write_export(shared: &Shared, configured: Option<&PathBuf>, export: &Export<'_>) -> Result<AgentReply> {
    let exported_at = rfc3339(SystemTime::now());
    let page = ExportPage {
        title: export.title,
        harness: export.harness,
        session_id: export.session_id,
        exported_at: &exported_at,
    };
    let html = shared.mask(&render(&page, export.messages), false);

    let dir = match configured {
        Some(dir) => dir.clone(),
        None => {
            let env = Env::from_process()?;
            exports_dir(&env).ok_or_else(|| BusinessError::NotFound("the app's data folder".to_owned()))?
        }
    };
    ensure_private_dir(&dir, "create the export folder")?;

    let path = dir.join(format!("{}-{}.html", export.prefix, export.session_id));
    replace_atomically(&path, html.as_bytes(), "write the HTML export")?;

    Ok(AgentReply::Exported {
        path: path.to_string_lossy().into_owned(),
    })
}
