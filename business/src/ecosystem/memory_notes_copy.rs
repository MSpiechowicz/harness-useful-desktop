//! Copying notes or scripts between this computer and the team database
//! (`memory_notes_copy_preview`, `memory_notes_copy_rows`, `memory_notes_copy_apply`, each for one
//! [`MemoryCopyKind`](crate::model::MemoryCopyKind) and one [`NotesCopyMode`], see [`CopyTarget`]).
//!
//! - [`preview_notes_copy`] runs `harness-memory push --dry-run --json --overwrite none` for notes
//!   (`scripts push …` for scripts) within [`PREVIEW_TIMEOUT`]. `stranded`, the default, never names
//!   a `--scope`: a plain push takes exactly what the server routes `remote` (its config and
//!   environment alike) and skips what is routed `local` or `remote+local`, while `--scope` would
//!   opt a `remote+local` scope in and so delete entries kept on this computer. `toRemote` adds
//!   `--remote-local`, which takes the scopes routed `remote+local` and no other, and `toLocal`
//!   runs `pull --dry-run --json` (`scripts pull …`): the team's entries of those scopes that are
//!   missing here. The two need their Settings switch and the server's feature ([`CopyGate`]).
//!   Rows are kept in a [`NotesCopyCache`] under a fresh preview id (one preview per kind and mode,
//!   replaced by every new one of that target), grouped by repository (global entries first) and
//!   sorted by title or name, and paged from there ([`NotesCopyCache::rows`]). A row the dry run
//!   says will fail (a team entry hidden on this computer matches it) carries its redacted message
//!   and counts as failed alone; it is never a conflict to pick.
//! - [`apply_notes_copy`] uses up the preview of its target and runs `harness-memory push --json
//!   --overwrite none|- --delete-copied` (`scripts push …`) within [`APPLY_TIMEOUT`], the picked
//!   conflicts' local ids on stdin with `-`; picking all sends the preview's conflicts, so a
//!   conflict that appeared since is never overwritten unseen. `toRemote` copies only what is
//!   picked (`pick`, required: the local ids of pickable rows or `all`, the preview's pickable rows,
//!   never one that appeared since; nothing picked is `invalidInput`) and runs `push --remote-local
//!   --json --selection - --delete-copied` with `{"only": [{"id", "name", "updatedAt"}, ...],
//!   "overwrite": [...]}` on stdin: `only` is the picked rows of the reviewed preview (their local id,
//!   title or name and update time exactly as the dry run printed them) and `overwrite` the picked
//!   conflicts, ticking one being the choice to replace the remote text, both from the cached preview
//!   and never read again. An entry not picked is not named, so it is neither copied nor deleted. The
//!   server takes an entry only while all three
//!   still match, so an entry saved or changed after the review is neither copied nor deleted; one
//!   it refuses comes back in `failed` with its local id, so the result lists it with its title (the
//!   reviewed one when the answer does not carry it) beside the entries copied. The
//!   server deletes the local copies of the entries it added, updated, overwrote or found
//!   identical; conflicts not picked, team-newer and failed entries stay, and so does a copied entry
//!   changed on this computer during the push. The result counts the entries kept (neither deleted
//!   nor failed) apart from the failed ones, and lists the local ids left over (kept, but not
//!   changed during the push, including an entry copied whose local copy could not be deleted),
//!   which the caller may discard by id, and none at all when a failure is no one entry's.
//!   `toLocal` runs `pull --json --ids -` with the picked team ids on stdin, one per line (`all` is
//!   the preview's entries to add, never one that appeared since), passes no delete flag and deletes
//!   nothing: the team's entries stay.
//!
//! The answers are read by `memory_copy_answer` (a push) and `memory_pull_answer` (a pull).
//! Output is capped at [`PUSH_OUTPUT_BYTES`]. Logs carry counts only, never titles or names.

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use super::memory_cli::{CliCall, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, clean};
use super::memory_copy_answer::{PushAnswer, counts, direction, noun, run_copy, run_push};
use super::memory_copy_gate::{CopyGate, CopyTarget};
use super::memory_copy_picks::CopyPicks;
use super::memory_pull_answer::PullAnswer;
use super::memory_scripts::MemoryScripts;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    NotesCopyFailure, NotesCopyFilter, NotesCopyMode, NotesCopyPreview, NotesCopyResult, NotesCopyRow, NotesCopyRows,
};
use crate::process::ProcessRunner;

/// How long the dry run may take (120 s: every local entry is compared with the team database).
pub const PREVIEW_TIMEOUT: Duration = Duration::from_secs(120);

/// How long the copy may take (300 s: every local entry is written to a team database that may be
/// far away).
pub const APPLY_TIMEOUT: Duration = Duration::from_secs(300);

/// The output kept of each stream (4 MiB: a row per local entry).
pub const PUSH_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

/// The rows of the latest preview of each target, kept until it is copied or replaced by a newer
/// one of the same target (a kind and a direction, see [`CopyTarget`]).
#[derive(Debug, Default)]
pub struct NotesCopyCache {
    current: Mutex<HashMap<CopyTarget, StoredNotesCopy>>,
}

/// One preview: its id and its rows in page order.
#[derive(Debug, Clone)]
struct StoredNotesCopy {
    id: String,
    rows: Vec<NotesCopyRow>,
}

impl NotesCopyCache {
    /// No preview yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The rows of the `target` preview `preview_id` under `filter`, from `offset` (0 unless
    /// given), `limit` of them (50 unless given). A bare kind is its `stranded` target.
    /// `invalidInput` for a limit of 0 or above 200, or a preview that is not the current one of
    /// `target`: a preview of another kind or direction is never one.
    pub fn rows(
        &self,
        target: impl Into<CopyTarget>,
        preview_id: &str,
        filter: NotesCopyFilter,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<NotesCopyRows> {
        let target = target.into();
        let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);

        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(BusinessError::InvalidInput(format!(
                "a page holds 1 to {MAX_PAGE_SIZE} entries"
            )));
        }

        let current = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        let stored = current
            .get(&target)
            .filter(|stored| stored.id == preview_id)
            .ok_or_else(expired)?;

        Ok(page(&stored.rows, filter, offset.unwrap_or(0), limit))
    }

    fn keep(&self, target: CopyTarget, stored: StoredNotesCopy) {
        self.current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(target, stored);
    }

    /// Takes the `target` preview out when it is the current one of `target` and `picks` are ids of
    /// its rows (see [`CopyPicks`]), answering what the copy reads on stdin (`None`: nothing) and
    /// the rows reviewed. Anything else leaves the preview in place.
    fn take(&self, target: CopyTarget, picks: &CopyPicks<'_>) -> Result<(Option<Vec<u8>>, Vec<NotesCopyRow>)> {
        let mut current = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        let stored = current
            .get(&target)
            .filter(|stored| stored.id == picks.preview_id)
            .ok_or_else(expired)?;

        let stdin = picks.stdin(target.kind, target.mode, &stored.rows)?;

        let stored = current.remove(&target).ok_or_else(expired)?;

        Ok((stdin, stored.rows))
    }
}

fn expired() -> BusinessError {
    BusinessError::InvalidInput("this review has expired, so review the copy again".to_owned())
}

/// The rows under `filter`, from `offset`, at most `limit`.
fn page(rows: &[NotesCopyRow], filter: NotesCopyFilter, offset: u32, limit: u32) -> NotesCopyRows {
    let kept = |row: &&NotesCopyRow| match filter {
        NotesCopyFilter::All => true,
        NotesCopyFilter::Conflicts => row.conflict && !row.failed,
        NotesCopyFilter::Failed => row.failed,
        NotesCopyFilter::Pickable => row.pickable,
    };
    let total = rows.iter().filter(kept).count();

    let rows = rows
        .iter()
        .filter(kept)
        .skip(usize::try_from(offset).unwrap_or(usize::MAX))
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .cloned()
        .collect();

    NotesCopyRows {
        total: u32::try_from(total).unwrap_or(u32::MAX),
        rows,
    }
}

/// What copying the entries of `target` would do (see the module docs), kept in `cache` under the
/// preview's id, in place of the previous preview of `target` only. `forbidden` (nothing run) when
/// the switch of a direction is off in `gate`, `notFound` when the server is not installed,
/// `unsupported` when it predates the notes view, the command of `target` or its `--json`,
/// `process` with its (redacted) line when it fails, `protocol` when its answer cannot be read.
pub async fn preview_notes_copy(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    gate: &CopyGate<'_>,
    cache: &NotesCopyCache,
    target: CopyTarget,
) -> Result<NotesCopyPreview> {
    let server = gate.server(env, probe, runner, target).await?;
    let (name, _) = target.names();

    let (nothing, mut rows) = match target.mode {
        NotesCopyMode::ToLocal => {
            let args = target.args(&["--dry-run", "--json"]);
            let stdout = run_copy(
                env,
                runner,
                &server,
                target.kind,
                target.mode,
                call(name, args, None, PREVIEW_TIMEOUT),
            )
            .await?;
            let answer = PullAnswer::read(&stdout, name)?;

            (answer.nothing.clone(), answer.rows())
        }
        NotesCopyMode::Stranded | NotesCopyMode::ToRemote => {
            let args = target.args(&["--dry-run", "--json", "--overwrite", "none"]);
            let call = call(name, args, None, PREVIEW_TIMEOUT);
            let answer = run_push(env, runner, &server, target.kind, target.mode, call).await?;

            (answer.nothing.clone(), answer.rows(target.mode))
        }
    };

    rows.sort_by_cached_key(|row| (row.repo.clone(), row.title.to_lowercase(), row.local_id, row.team_id));

    let preview = NotesCopyPreview {
        preview_id: uuid::Uuid::new_v4().to_string(),
        nothing,
        counts: counts(&rows),
        total: u32::try_from(rows.len()).unwrap_or(u32::MAX),
        rows: page(&rows, NotesCopyFilter::All, 0, DEFAULT_PAGE_SIZE).rows,
    };

    tracing::info!(
        kind = noun(target.kind),
        direction = direction(target.mode),
        rows = rows.len(),
        conflicts = preview.counts.conflicts,
        failed = preview.counts.failed,
        "copy reviewed"
    );

    cache.keep(
        target,
        StoredNotesCopy {
            id: preview.preview_id.clone(),
            rows,
        },
    );

    Ok(preview)
}

/// Copies the entries of the `target` preview `picks.preview_id` (see the module docs),
/// overwriting a teammate's newer-differing text for the conflicts `picks.overwrite` picks (none,
/// all, or those local ids, each still a conflict when the copy runs) in `stranded`, or copying
/// the entries `picks.pick` names in `toRemote` (all, or those local ids, a conflict replacing the
/// remote text) and in `toLocal` (all, or those team ids). `invalidInput` (nothing copied, the
/// preview stays) when the preview is not the current one of `target`, an id is not one of its
/// rows to take or is one its dry run says will fail, or a `toRemote` or `toLocal` copy picks
/// nothing; otherwise the preview is used up. Fails as [`preview_notes_copy`].
pub async fn apply_notes_copy(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    gate: &CopyGate<'_>,
    cache: &NotesCopyCache,
    target: CopyTarget,
    picks: &CopyPicks<'_>,
) -> Result<NotesCopyResult> {
    let server = gate.server(env, probe, runner, target).await?;
    let (stdin, reviewed) = cache.take(target, picks)?;
    let (_, name) = target.names();

    let result = match target.mode {
        NotesCopyMode::ToLocal => {
            let args = target.args(&["--json", "--ids", "-"]);
            let stdout = run_copy(
                env,
                runner,
                &server,
                target.kind,
                target.mode,
                call(name, args, stdin, APPLY_TIMEOUT),
            )
            .await?;

            pulled(&PullAnswer::read(&stdout, name)?)
        }
        NotesCopyMode::Stranded | NotesCopyMode::ToRemote => {
            let args = apply_args(target, stdin.is_some());
            let answer = run_push(
                env,
                runner,
                &server,
                target.kind,
                target.mode,
                call(name, args, stdin, APPLY_TIMEOUT),
            )
            .await?;

            pushed(&answer, target.mode, &reviewed)
        }
    };

    tracing::info!(
        kind = noun(target.kind),
        direction = direction(target.mode),
        added = result.counts.added,
        deleted = result.deleted,
        kept = result.kept,
        failed = result.failed.len(),
        "copied"
    );

    Ok(result)
}

/// The push of a copy: `toRemote` always reads the picked selection from stdin (`--selection -`,
/// in place of `--overwrite`), `stranded` the conflicts to overwrite (`--overwrite -`) or none.
fn apply_args(target: CopyTarget, has_stdin: bool) -> Vec<OsString> {
    match target.mode {
        NotesCopyMode::ToRemote => target.args(&["--json", "--selection", "-", "--delete-copied"]),
        _ => {
            let word = if has_stdin { "-" } else { "none" };

            target.args(&["--json", "--overwrite", word, "--delete-copied"])
        }
    }
}

/// What a copy to the team database did.
///
/// A failure keeps the title of its entry in the answer, else the one `reviewed` rows give it.
fn pushed(answer: &PushAnswer, mode: NotesCopyMode, reviewed: &[NotesCopyRow]) -> NotesCopyResult {
    let rows = answer.rows(mode);
    let failed_ids = answer.failed_ids();
    let unattributed = answer.failed.iter().any(|failure| failure.local_id.is_none());

    let failed: Vec<NotesCopyFailure> = answer
        .failed
        .iter()
        .map(|failure| NotesCopyFailure {
            local_id: failure.local_id,
            team_id: None,
            title: failure
                .local_id
                .and_then(|id| {
                    [&rows[..], reviewed]
                        .into_iter()
                        .find_map(|rows| rows.iter().find(|row| row.local_id == Some(id)))
                })
                .map(|row| row.title.clone()),
            message: clean(&failure.message),
        })
        .collect();

    let deleted_rows = answer.changes.iter().filter(|change| change.deleted).count();
    let kept: Vec<_> = answer
        .changes
        .iter()
        .filter(|change| !change.deleted && !failed_ids.contains(&change.local_id))
        .collect();

    // A failure that is no one entry's (a relation not pushed, copies not deleted) may concern any
    // entry left here, so none is offered for discarding. An entry changed on this computer during
    // the push holds edits newer than its copy, so it is not offered either.
    let leftover = if unattributed {
        Vec::new()
    } else {
        kept.iter()
            .filter(|change| !change.changed_since)
            .map(|change| change.local_id)
            .collect()
    };

    NotesCopyResult {
        counts: counts(&rows),
        deleted: answer
            .deleted
            .unwrap_or(u32::try_from(deleted_rows).unwrap_or(u32::MAX)),
        kept: u32::try_from(kept.len()).unwrap_or(u32::MAX),
        failed,
        leftover,
    }
}

/// What a copy to this computer did: nothing is deleted or kept, so nothing is left over.
fn pulled(answer: &PullAnswer) -> NotesCopyResult {
    let rows = answer.rows();

    let failed = answer
        .failed
        .iter()
        .map(|failure| NotesCopyFailure {
            local_id: None,
            team_id: failure.team_id,
            title: failure
                .team_id
                .and_then(|id| rows.iter().find(|row| row.team_id == Some(id)))
                .map(|row| row.title.clone()),
            message: clean(&failure.message),
        })
        .collect();

    NotesCopyResult {
        counts: counts(&rows),
        deleted: 0,
        kept: 0,
        failed,
        leftover: Vec::new(),
    }
}

fn call<'a>(name: &'a str, args: Vec<OsString>, stdin: Option<Vec<u8>>, timeout: Duration) -> CliCall<'a> {
    CliCall {
        name,
        args,
        timeout,
        output_bytes: PUSH_OUTPUT_BYTES,
        stdin,
    }
}
