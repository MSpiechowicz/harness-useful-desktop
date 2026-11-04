//! Team approval: `harness-knowledge approval get|set --json` (`knowledge_approval_get`,
//! `knowledge_approval_set`). `approval get` prints `{"approval", "place", "writable", "changedBy",
//! "changedAt"}` (the last two null while unknown), `approval set on|off` the same, or only what it
//! changed. The server refuses a role that may not change the
//! setting (`readOnly`) and a setup with no team database (`refused`). Needs `docs-approval`.

use std::path::Path;

use serde_json::Value;

use super::knowledge_cli::{call, words};
use super::knowledge_review::{answered, outcome};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{KnowledgeOutcome, KnowledgePlace, KnowledgeTeamApproval, MAX_AUTHOR_NAME_CHARS, display_text};
use crate::process::ProcessRunner;

/// Whether team approval is on, where it is kept and whether the user may change it (`approval get
/// --json`).
pub async fn get_approval(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Result<KnowledgeTeamApproval> {
    let data = answered(
        env,
        runner,
        server,
        call("approval get", words(&["approval", "get", "--json"])),
    )
    .await?;

    read_approval(&data, None)
}

/// Turns team approval on or off for the team database (`approval set --json on|off`). The server
/// refuses a role that may not change it (`readOnly`) and a setup with no team database
/// (`refused`).
pub async fn set_approval(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    on: bool,
) -> Result<KnowledgeOutcome<KnowledgeTeamApproval>> {
    let args = words(&["approval", "set", "--json", if on { "on" } else { "off" }]);

    let outcome = outcome(env, runner, server, call("approval set", args), |data| {
        read_approval(data, Some(on))
    })
    .await?;
    tracing::info!(on, "team approval was set");

    Ok(outcome)
}

/// The approval in `approval get|set`'s answer. A `set` that left out what it did not change is
/// read as the remote database (the only place a setting is written) and the role that just wrote.
fn read_approval(data: &Value, written: Option<bool>) -> Result<KnowledgeTeamApproval> {
    let unreadable =
        || BusinessError::Protocol("harness-knowledge approval answered a setting this app cannot read".to_owned());

    let approval = data
        .get("approval")
        .and_then(Value::as_bool)
        .or(written)
        .ok_or_else(unreadable)?;

    let place = match data.get("place").and_then(Value::as_str) {
        Some("remote") => KnowledgePlace::Remote,
        Some("local") => KnowledgePlace::Local,
        Some(_) => return Err(unreadable()),
        None if written.is_some() => KnowledgePlace::Remote,
        None => return Err(unreadable()),
    };

    let writable = match data.get("writable") {
        Some(Value::Bool(writable)) => Some(*writable),
        Some(_) => None,
        None => written.map(|_| true),
    };

    // Who switched it and when, from a server that records it; a `set` that left them out says none.
    let text = |key: &str| {
        data.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };

    Ok(KnowledgeTeamApproval {
        approval,
        place,
        writable,
        changed_by: text("changedBy")
            .map(|name| display_text(&name, MAX_AUTHOR_NAME_CHARS))
            .filter(|name| !name.is_empty()),
        changed_at: text("changedAt"),
    })
}
