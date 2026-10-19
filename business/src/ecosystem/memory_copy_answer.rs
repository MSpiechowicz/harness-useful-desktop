//! Running `harness-memory push --json` or `scripts push --json` and reading its answer, for
//! [`super::memory_notes_copy`].
//!
//! Both print one object, a dry run and a real run alike: `{"dryRun", "nothing", "notes"|"scripts":
//! {…}, "deleted", "changes": [{"localId", "scope", "repo", "title"|"name", "action", "conflict",
//! "teamId", "localUpdatedAt", "teamUpdatedAt", "deleted", "failed", "changedSince"}], "failed":
//! [{"localId", "message"}]}`. The counts object (`notes` for notes, `scripts` for scripts) is not read: the
//! counts are taken from the rows, so either answer reads the same. A change's `failed`, absent
//! from older servers, marks a write that failed (`false` with an entry of `failed`: copied, but
//! its local copy could not be deleted); a dry run marks the writes that will fail the same way
//! (a team entry hidden on this computer matches). A change's `changedSince`, absent from older
//! servers, marks a copied entry whose local copy was not deleted because it changed during the
//! push. A failure's `localId` is null when it is no one entry's (a relation that was not pushed,
//! local copies that could not be deleted). The command exits 1 when an entry failed or will fail
//! (the object is still read) and 2 for a usage error (a server that
//! predates it: `unsupported`). A row this app cannot read is skipped, and the parser's message is
//! dropped: it can quote a title.

use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::memory_cli::{CliAnswer, CliCall, USAGE_EXIT, clean, run_memory_cli};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{MemoryCopyKind, NotesCopyAction, NotesCopyCounts, NotesCopyRow, ScriptScope};
use crate::process::ProcessRunner;

/// The exit code of a push in which an entry failed (the answer is still printed).
const FAILED_EXIT: i32 = 1;

/// The answer of `push --json` or `scripts push --json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PushAnswer {
    pub(super) nothing: Option<String>,
    #[serde(default)]
    pub(super) changes: Vec<PushChange>,
    #[serde(default)]
    pub(super) failed: Vec<PushFailure>,
    pub(super) deleted: Option<u32>,
}

/// One row of `changes`, as far as this app reads it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PushChange {
    pub(super) local_id: i64,
    scope: ScriptScope,
    repo: Option<String>,
    /// A note's title, a script's name.
    #[serde(alias = "name")]
    title: String,
    action: NotesCopyAction,
    #[serde(default)]
    conflict: bool,
    local_updated_at: Option<String>,
    team_updated_at: Option<String>,
    #[serde(default)]
    pub(super) deleted: bool,
    /// Whether its write failed (or will fail, in a dry run); `None` from older servers, which
    /// lack the flag.
    #[serde(default)]
    failed: Option<bool>,
    /// Whether it was copied but not deleted because its local copy changed during the push.
    #[serde(default)]
    pub(super) changed_since: bool,
}

/// One entry of `failed`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PushFailure {
    /// `None` for a failure that is no one entry's.
    pub(super) local_id: Option<i64>,
    #[serde(default)]
    pub(super) message: String,
}

impl PushAnswer {
    /// The changes as rows, in the server's order, each failed one with the (redacted) message of
    /// its entry of `failed` (see [`Self::failed_ids`]).
    pub(super) fn rows(&self) -> Vec<NotesCopyRow> {
        let failed_ids = self.failed_ids();

        self.changes
            .iter()
            .map(|change| {
                let failed = failed_ids.contains(&change.local_id);

                NotesCopyRow {
                    local_id: change.local_id,
                    scope: change.scope,
                    repo: change.repo.clone(),
                    title: change.title.clone(),
                    action: change.action,
                    conflict: change.conflict,
                    local_updated_at: change.local_updated_at.clone(),
                    team_updated_at: change.team_updated_at.clone(),
                    failed,
                    failure: failed.then(|| self.failure_of(change.local_id)).flatten(),
                }
            })
            .collect()
    }

    /// The (redacted) message of the first entry of `failed` for `local_id`.
    fn failure_of(&self, local_id: i64) -> Option<String> {
        self.failed
            .iter()
            .find(|failure| failure.local_id == Some(local_id) && !failure.message.is_empty())
            .map(|failure| clean(&failure.message))
    }

    /// The local ids whose copy failed: each change marked `"failed": true`, and each entry of
    /// `failed` whose change is not marked `"failed": false` (a server that predates the flag). A
    /// change marked `"failed": false` was copied even when its entry of `failed` says the local
    /// copy could not be deleted, so it is not one of them and stays left over.
    pub(super) fn failed_ids(&self) -> Vec<i64> {
        let copied = |id: i64| {
            self.changes
                .iter()
                .any(|change| change.local_id == id && change.failed == Some(false))
        };

        self.changes
            .iter()
            .filter(|change| change.failed == Some(true))
            .map(|change| change.local_id)
            .chain(
                self.failed
                    .iter()
                    .filter_map(|failure| failure.local_id)
                    .filter(|id| !copied(*id)),
            )
            .collect()
    }
}

/// How many `rows` go each way; a failed row counts in `failed` alone.
pub(super) fn counts(rows: &[NotesCopyRow]) -> NotesCopyCounts {
    let mut counts = NotesCopyCounts::default();

    for row in rows {
        if row.failed {
            counts.failed = counts.failed.saturating_add(1);
            continue;
        }

        let count = match row.action {
            NotesCopyAction::Add => Some(&mut counts.added),
            NotesCopyAction::Update => Some(&mut counts.updated),
            NotesCopyAction::Overwrite => Some(&mut counts.overwritten),
            NotesCopyAction::Identical => Some(&mut counts.identical),
            NotesCopyAction::TeamNewer => Some(&mut counts.team_newer),
            NotesCopyAction::Keep => None,
        };

        if let Some(count) = count {
            *count = count.saturating_add(1);
        }

        if row.conflict {
            counts.conflicts = counts.conflicts.saturating_add(1);
        }
    }

    counts
}

/// What `kind` names in messages and logs.
pub(super) fn noun(kind: MemoryCopyKind) -> &'static str {
    match kind {
        MemoryCopyKind::Notes => "notes",
        MemoryCopyKind::Scripts => "scripts",
    }
}

/// Runs the push of `kind` and reads its answer, also when an entry failed (exit 1 with the object
/// on stdout).
pub(super) async fn run_push(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    kind: MemoryCopyKind,
    call: CliCall<'_>,
) -> Result<PushAnswer> {
    let name = call.name.to_owned();

    let stdout = match run_memory_cli(env, runner, server, call).await? {
        CliAnswer::Done(stdout) => stdout,
        CliAnswer::Failed {
            status: Some(USAGE_EXIT),
            ..
        } => {
            return Err(BusinessError::Unsupported(format!(
                "this harness-memory cannot copy {} to the remote database; update Harness Memory",
                noun(kind)
            )));
        }
        CliAnswer::Failed {
            status: Some(FAILED_EXIT),
            stdout,
            message,
        } => match serde_json::from_str::<Value>(&stdout) {
            Ok(value) if value.get("changes").is_some() => stdout,
            _ => return Err(BusinessError::Process(message)),
        },
        CliAnswer::Failed { message, .. } => return Err(BusinessError::Process(message)),
    };

    read_answer(&stdout, &name)
}

/// The answer in `stdout`; rows this app cannot read are skipped. The parser's message is dropped:
/// it can quote a title.
fn read_answer(stdout: &str, name: &str) -> Result<PushAnswer> {
    let unreadable = || BusinessError::Protocol(format!("harness-memory {name} answered output this app cannot read"));
    let mut value: Value = serde_json::from_str(stdout).map_err(|_| unreadable())?;

    for field in ["changes", "failed"] {
        if let Some(entries) = value.get_mut(field).and_then(Value::as_array_mut) {
            let count = entries.len();
            entries.retain(|entry| readable(field, entry));

            if entries.len() < count {
                tracing::warn!(
                    command = name,
                    field,
                    skipped = count - entries.len(),
                    "copy to the remote database: unreadable rows were skipped"
                );
            }
        }
    }

    serde_json::from_value(value).map_err(|_| unreadable())
}

fn readable(field: &str, entry: &Value) -> bool {
    match field {
        "changes" => PushChange::deserialize(entry).is_ok(),
        _ => PushFailure::deserialize(entry).is_ok(),
    }
}
