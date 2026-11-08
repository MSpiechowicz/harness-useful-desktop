//! Reading the answer of `harness-memory pull --json` or `scripts pull --json`, for
//! [`super::memory_notes_copy`].
//!
//! Both print one object, a dry run and a real run alike: `{"dryRun", "nothing", "notes"|"scripts":
//! {…}, "changes": [{"teamId", "localId", "scope", "repo", "title"|"name", "action",
//! "teamUpdatedAt", "localUpdatedAt", "failed"}], "failed": [{"teamId", "message"}]}`. The counts
//! object is not read: the counts are taken from the rows, as for a push. A change's `action` is
//! `add` (the team's entry is not here), `identical` (the same is here already) or `localDiffers`
//! (a different version is here, which a pull never replaces); its `localId` is null for an
//! entry to add. A failure's `teamId` is null when it is no one entry's. Only an entry to add that
//! does not fail can be picked. The command exits 1 when an entry failed or will fail (the object
//! is still read) and 2 for a usage error (a server that predates it: `unsupported`). A row this
//! app cannot read is skipped, and the parser's message is dropped: it can quote a title.

use serde::Deserialize;

use super::memory_cli::clean;
use super::memory_copy_answer::read_answer;
use crate::error::Result;
use crate::model::{NotesCopyAction, NotesCopyRow, ScriptScope};

/// The answer of `pull --json` or `scripts pull --json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullAnswer {
    pub(super) nothing: Option<String>,
    #[serde(default)]
    changes: Vec<PullChange>,
    #[serde(default)]
    pub(super) failed: Vec<PullFailure>,
}

/// One row of `changes`, as far as this app reads it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullChange {
    team_id: i64,
    local_id: Option<i64>,
    scope: ScriptScope,
    repo: Option<String>,
    /// A note's title, a script's name.
    #[serde(alias = "name")]
    title: String,
    action: NotesCopyAction,
    team_updated_at: Option<String>,
    local_updated_at: Option<String>,
    /// Whether its copy failed (or will fail, in a dry run).
    #[serde(default)]
    failed: bool,
}

/// One entry of `failed`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PullFailure {
    /// `None` for a failure that is no one entry's.
    pub(super) team_id: Option<i64>,
    #[serde(default)]
    pub(super) message: String,
}

impl PullAnswer {
    /// `stdout` of `name` read as an answer.
    pub(super) fn read(stdout: &str, name: &str) -> Result<Self> {
        read_answer::<Self, PullChange, PullFailure>(stdout, name)
    }

    /// The changes as rows, in the server's order. A row is failed when its change says so or an
    /// entry of `failed` names its team id, and then carries that entry's (redacted) message. Only
    /// an entry to add that does not fail is pickable.
    pub(super) fn rows(&self) -> Vec<NotesCopyRow> {
        self.changes
            .iter()
            .map(|change| {
                let failure = self.failure_of(change.team_id);
                let failed = change.failed || self.failed.iter().any(|entry| entry.team_id == Some(change.team_id));

                NotesCopyRow {
                    local_id: change.local_id,
                    team_id: Some(change.team_id),
                    pickable: change.action == NotesCopyAction::Add && !failed,
                    scope: change.scope,
                    repo: change.repo.clone(),
                    title: change.title.clone(),
                    action: change.action,
                    conflict: false,
                    local_updated_at: change.local_updated_at.clone(),
                    team_updated_at: change.team_updated_at.clone(),
                    failed,
                    failure,
                }
            })
            .collect()
    }

    /// The (redacted) message of the first entry of `failed` for `team_id`.
    fn failure_of(&self, team_id: i64) -> Option<String> {
        self.failed
            .iter()
            .find(|entry| entry.team_id == Some(team_id) && !entry.message.is_empty())
            .map(|entry| clean(&entry.message))
    }
}
