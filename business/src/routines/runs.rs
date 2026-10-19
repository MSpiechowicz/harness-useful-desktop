//! Run records (see "Run records" in `docs/routines-format.md`): one file per routine,
//! `<app_local_data_dir>/routine-runs/<routineId>.json` (`0600`, the folder `0700`), `{ "version":
//! 1, "routineId", "runs": [...] }`, newest first, at most [`MAX_RUNS_PER_ROUTINE`]. Written by the
//! app window alone, under its routines lock.
//!
//! - A run is written when it starts (`running`) and rewritten when it ends ([`RunStore::finish`]).
//!   Only a start creates a file: changing a run whose records were deleted (its routine was
//!   deleted meanwhile) writes nothing, so a deleted routine's history never comes back.
//! - At launch, and when the app quits, runs still `running` become `stopped` with
//!   [`CLOSED_MESSAGE`] ([`RunStore::stop_leftovers`]).
//! - Messages, names and excerpts are capped here; redacting them is the caller's job, and a
//!   prompt or command is never copied into a record.
//! - An unreadable or invalid file is set aside (`.corrupt-<unix-seconds>`) and the history starts
//!   again.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::files::{load_json, read_json, remove_file, write_json};
use super::when::rfc3339_utc;
use crate::error::Result;
use crate::model::{
    HookOrigin, Routine, RoutineRun, RoutineRunsPage, RoutineRunsQuery, RunRecord, RunStatus, RunTrigger,
};
use crate::signed_docs::fields::MAX_NAME_CHARS;
use crate::signed_docs::is_doc_id;

/// The folder in the app's local data folder.
pub const RUNS_FOLDER: &str = "routine-runs";

/// Runs kept per routine.
pub const MAX_RUNS_PER_ROUTINE: usize = 100;

/// Longest excerpt, in bytes.
pub const MAX_EXCERPT_BYTES: usize = 4096;

/// Longest message, in characters.
pub const MAX_MESSAGE_CHARS: usize = 500;

/// Largest page of runs.
pub const MAX_RUNS_PAGE: u32 = 200;

/// The message of a run Harness Desktop closed during.
pub const CLOSED_MESSAGE: &str = "Harness Desktop closed during the run";

/// Largest record file read (100 runs with two excerpts each).
const MAX_RUNS_FILE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunsFile {
    version: u32,
    routine_id: String,
    runs: Vec<RunRecord>,
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEnd {
    /// `ok`, `failed`, `needsAttention` or `stopped`.
    pub status: RunStatus,
    /// When it ended.
    pub ended_at: Timestamp,
    /// A short secret-free reason.
    pub message: Option<String>,
    /// The last assistant text, or the stdout tail (redacted).
    pub excerpt: Option<String>,
    /// The stderr tail (redacted).
    pub error_excerpt: Option<String>,
    /// The exit code of a command or script.
    pub exit_code: Option<i32>,
}

/// The run records in one local data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStore {
    dir: PathBuf,
}

impl RunStore {
    /// The records in `<local_data_dir>/routine-runs`.
    pub fn new(local_data_dir: &Path) -> Self {
        Self {
            dir: local_data_dir.join(RUNS_FOLDER),
        }
    }

    fn path(&self, routine_id: &str) -> PathBuf {
        self.dir.join(format!("{routine_id}.json"))
    }

    /// The runs of `routine_id`, newest first (none when there is no usable file).
    pub fn runs_of(&self, routine_id: &str) -> Vec<RoutineRun> {
        if !is_doc_id(routine_id) {
            return Vec::new();
        }

        read_json::<RunsFile>(&self.path(routine_id), MAX_RUNS_FILE_BYTES, |file| {
            file.version == 1 && file.routine_id == routine_id
        })
        .map(|file| file.runs.into_iter().map(|record| shown(routine_id, record)).collect())
        .unwrap_or_default()
    }

    /// Records the start of `run` (newest first, the oldest dropped past
    /// [`MAX_RUNS_PER_ROUTINE`]).
    pub fn start(&self, run: &RoutineRun) -> Result<()> {
        self.change(&run.routine_id, |runs| {
            runs.retain(|record| record.id != run.record.id);
            runs.insert(0, capped(run.record.clone()));
            runs.truncate(MAX_RUNS_PER_ROUTINE);
            true
        })
    }

    /// Changes the record of run `run_id` of `routine_id`; `None` (and nothing written) when there
    /// is none.
    pub fn update(
        &self,
        routine_id: &str,
        run_id: &str,
        change: impl FnOnce(&mut RunRecord),
    ) -> Result<Option<RoutineRun>> {
        let mut updated = None;

        self.change(routine_id, |runs| {
            let Some(record) = runs.iter_mut().find(|record| record.id == run_id) else {
                return false;
            };

            change(record);
            *record = capped(record.clone());
            updated = Some(record.clone());
            true
        })?;

        Ok(updated.map(|record| shown(routine_id, record)))
    }

    /// Records how run `run_id` of `routine_id` ended; `None` when there is no such record.
    pub fn finish(&self, routine_id: &str, run_id: &str, end: RunEnd) -> Result<Option<RoutineRun>> {
        self.update(routine_id, run_id, |record| {
            record.duration_ms = record
                .started_at
                .parse::<Timestamp>()
                .ok()
                .map(|started| u64::try_from(end.ended_at.duration_since(started).as_millis()).unwrap_or(0));
            record.ended_at = Some(rfc3339_utc(end.ended_at));
            record.status = end.status;
            record.message = end.message;
            record.excerpt = end.excerpt;
            record.error_excerpt = end.error_excerpt;
            record.exit_code = end.exit_code;
        })
    }

    /// Marks every run still `running` as `stopped` ([`CLOSED_MESSAGE`], ended `now`), but the
    /// runs `live` names (they go on in this session): at launch for what the last session left,
    /// and when the app quits. Returns how many changed.
    pub fn stop_leftovers(&self, now: Timestamp, live: &BTreeSet<String>) -> Result<usize> {
        let mut stopped = 0;

        for routine_id in self.routine_ids() {
            let leftovers: Vec<String> = self
                .runs_of(&routine_id)
                .into_iter()
                .filter(|run| run.record.status == RunStatus::Running && !live.contains(&run.record.id))
                .map(|run| run.record.id)
                .collect();

            for run_id in leftovers {
                let end = RunEnd {
                    status: RunStatus::Stopped,
                    ended_at: now,
                    message: Some(CLOSED_MESSAGE.to_owned()),
                    excerpt: None,
                    error_excerpt: None,
                    exit_code: None,
                };

                if self.finish(&routine_id, &run_id, end)?.is_some() {
                    stopped += 1;
                }
            }
        }

        Ok(stopped)
    }

    /// Deletes the records of `routine_id` (the routine was deleted).
    pub fn delete(&self, routine_id: &str) -> Result<()> {
        if !is_doc_id(routine_id) {
            return Ok(());
        }

        remove_file(&self.path(routine_id), "the runs of a routine")
    }

    /// One page of runs, newest first (by start, then id): of one routine or all, optionally with
    /// one status. `limit` is 1 to [`MAX_RUNS_PAGE`] (0 counts as 1).
    pub fn page(&self, query: &RoutineRunsQuery) -> RoutineRunsPage {
        let ids = match &query.routine_id {
            Some(id) => vec![id.clone()],
            None => self.routine_ids(),
        };

        let runs: Vec<RoutineRun> = ids.iter().flat_map(|id| self.runs_of(id)).collect();

        page_of(runs, query)
    }

    /// The run `run_id`, looked for in every routine's records.
    pub fn find(&self, run_id: &str) -> Option<RoutineRun> {
        self.routine_ids()
            .iter()
            .flat_map(|id| self.runs_of(id))
            .find(|run| run.record.id == run_id)
    }

    /// The routines that have a record file.
    pub fn routine_ids(&self) -> Vec<String> {
        crate::signed_docs::snapshots::json_ids(&self.dir)
    }

    /// Deletes the records of the team routines whose routine is gone: their newest run is a team
    /// run, no routine of `running` runs, and `exists` says neither a routine file nor a team copy
    /// has the id. Returns the ids deleted.
    pub fn delete_gone_team_runs(
        &self,
        running: &BTreeSet<String>,
        exists: impl Fn(&str) -> bool,
    ) -> Result<Vec<String>> {
        let mut deleted = Vec::new();

        for routine_id in self.routine_ids() {
            let team = self
                .runs_of(&routine_id)
                .first()
                .is_some_and(|run| run.record.origin == HookOrigin::Team);

            if !team || running.contains(&routine_id) || exists(&routine_id) {
                continue;
            }

            self.delete(&routine_id)?;
            deleted.push(routine_id);
        }

        Ok(deleted)
    }

    /// Loads the runs of `routine_id` for a change (an invalid file is set aside) and lets
    /// `change` edit them. Saves them when `change` says it changed something.
    fn change(&self, routine_id: &str, change: impl FnOnce(&mut Vec<RunRecord>) -> bool) -> Result<()> {
        crate::signed_docs::check_id::<super::RoutineKind>(routine_id)?;

        let path = self.path(routine_id);
        let file = load_json::<RunsFile>(&path, MAX_RUNS_FILE_BYTES, |file| {
            file.version == 1 && file.routine_id == routine_id
        })?;

        let mut runs = file.map(|file| file.runs).unwrap_or_default();

        if !change(&mut runs) {
            return Ok(());
        }

        let file = RunsFile {
            version: 1,
            routine_id: routine_id.to_owned(),
            runs,
        };

        write_json(&path, &file, "the runs of a routine")
    }
}

/// A new `running` record of `routine` started at `now` by `trigger`: `origin` local or team,
/// `project` the project root it runs in (`None` for a global run).
pub fn started_run(
    routine: &Routine,
    origin: HookOrigin,
    trigger: RunTrigger,
    project: Option<String>,
    now: Timestamp,
) -> RoutineRun {
    let record = RunRecord {
        id: uuid::Uuid::new_v4().simple().to_string(),
        routine_name: routine.name.clone(),
        origin,
        trigger,
        started_at: rfc3339_utc(now),
        ended_at: None,
        duration_ms: None,
        status: RunStatus::Running,
        message: None,
        excerpt: None,
        error_excerpt: None,
        exit_code: None,
        session: None,
        project,
    };

    RoutineRun {
        routine_id: routine.id.clone(),
        needs_you: false,
        record: capped(record),
    }
}

/// One page of `runs` as `query` asks for it: those of its routine and status, newest first (by
/// start, then id), `limit` 1 to [`MAX_RUNS_PAGE`] (0 counts as 1) from `offset`.
pub fn page_of(mut runs: Vec<RoutineRun>, query: &RoutineRunsQuery) -> RoutineRunsPage {
    runs.retain(|run| {
        let routine = query.routine_id.as_ref().is_none_or(|id| run.routine_id == *id);
        let status = query.status.is_none_or(|status| run.record.status == status);

        routine && status
    });

    runs.sort_by(|left, right| {
        right
            .record
            .started_at
            .cmp(&left.record.started_at)
            .then_with(|| right.record.id.cmp(&left.record.id))
    });

    let total = u32::try_from(runs.len()).unwrap_or(u32::MAX);
    let limit = query.limit.clamp(1, MAX_RUNS_PAGE);
    let end = query.offset.saturating_add(limit);

    let page = runs
        .into_iter()
        .skip(query.offset as usize)
        .take(limit as usize)
        .collect();

    RoutineRunsPage {
        runs: page,
        total,
        next_offset: (end < total).then_some(end),
    }
}

/// The tail of `text` that fits [`MAX_EXCERPT_BYTES`], cut on a character boundary.
pub fn excerpt_tail(text: &str) -> String {
    if text.len() <= MAX_EXCERPT_BYTES {
        return text.to_owned();
    }

    let mut start = text.len() - MAX_EXCERPT_BYTES;
    while !text.is_char_boundary(start) {
        start += 1;
    }

    text[start..].to_owned()
}

/// `record` within the limits: name and message cut to their characters, excerpts to their
/// tails.
fn capped(mut record: RunRecord) -> RunRecord {
    record.routine_name = record.routine_name.chars().take(MAX_NAME_CHARS).collect();
    record.message = record
        .message
        .map(|message| message.chars().take(MAX_MESSAGE_CHARS).collect());
    record.excerpt = record.excerpt.as_deref().map(excerpt_tail);
    record.error_excerpt = record.error_excerpt.as_deref().map(excerpt_tail);

    record
}

fn shown(routine_id: &str, record: RunRecord) -> RoutineRun {
    RoutineRun {
        routine_id: routine_id.to_owned(),
        needs_you: false,
        record,
    }
}
