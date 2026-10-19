//! Copying the notes or scripts saved on this computer to the team database, for a note scope or
//! the scripts moved there (`memory_notes_copy_preview`, `memory_notes_copy_rows`,
//! `memory_notes_copy_apply`, each for one [`MemoryCopyKind`]).
//!
//! - [`preview_notes_copy`] runs `harness-memory push --dry-run --json --overwrite none` for notes
//!   (`scripts push …` for scripts) within [`PREVIEW_TIMEOUT`]. It never names a `--scope`: a plain
//!   push takes exactly what the server routes `remote` (its config and environment alike) and
//!   skips what is routed `local` or `remote+local`, while `--scope` would opt a `remote+local`
//!   scope in and so delete entries kept on this computer. Its rows are kept in a
//!   [`NotesCopyCache`] under a fresh preview id (one preview per kind, replaced by every new one of
//!   that kind), grouped by repository (global entries first) and sorted by title or name, and
//!   paged from there ([`NotesCopyCache::rows`]). A row the dry run says will fail (a team entry
//!   hidden on this computer matches it) carries its redacted message and counts as failed alone;
//!   it is never a conflict to pick.
//! - [`apply_notes_copy`] uses up the preview of its kind and runs `harness-memory push --json
//!   --overwrite none|- --delete-copied` (`scripts push …`) within [`APPLY_TIMEOUT`] (no `--scope`
//!   either), the picked conflicts' local ids on stdin with `-`; picking all sends the preview's
//!   conflicts, so a conflict that appeared since is never overwritten unseen. The server deletes
//!   the local copies of the entries it added, updated, overwrote or found identical; conflicts not
//!   picked, team-newer and failed entries stay, and so does a copied entry changed on this
//!   computer during the push. The result counts the entries kept (neither deleted nor failed)
//!   apart from the failed ones, and lists the local ids left over (kept, but not changed during
//!   the push, including an entry copied whose local copy could not be deleted), which the caller
//!   may discard by id, and none at all when a failure is no one entry's.
//!
//! The answer is read by `memory_copy_answer`. Output is capped at [`PUSH_OUTPUT_BYTES`].
//! Logs carry counts only, never titles or names.

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use super::memory_cli::{CliCall, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, clean};
use super::memory_copy_answer::{counts, noun, run_push};
use super::memory_notes::{ids_on_stdin, server};
use super::memory_scripts::MemoryScripts;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    LocalNoteIds, MemoryCopyKind, NoteIdsWord, NotesCopyFailure, NotesCopyFilter, NotesCopyPreview, NotesCopyResult,
    NotesCopyRow, NotesCopyRows,
};
use crate::process::ProcessRunner;

/// How long the dry run may take (120 s: every local entry is compared with the team database).
pub const PREVIEW_TIMEOUT: Duration = Duration::from_secs(120);

/// How long the copy may take (300 s: every local entry is written to a team database that may be
/// far away).
pub const APPLY_TIMEOUT: Duration = Duration::from_secs(300);

/// The output kept of each stream (4 MiB: a row per local entry).
pub const PUSH_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

/// The rows of the latest preview of each kind, kept until it is copied or replaced by a newer one
/// of the same kind.
#[derive(Debug, Default)]
pub struct NotesCopyCache {
    current: Mutex<HashMap<MemoryCopyKind, StoredNotesCopy>>,
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

    /// The rows of the `kind` preview `preview_id` under `filter`, from `offset` (0 unless given),
    /// `limit` of them (50 unless given). `invalidInput` for a limit of 0 or above 200, or a
    /// preview that is not the current one of `kind`.
    pub fn rows(
        &self,
        kind: MemoryCopyKind,
        preview_id: &str,
        filter: NotesCopyFilter,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<NotesCopyRows> {
        let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);

        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(BusinessError::InvalidInput(format!(
                "a page holds 1 to {MAX_PAGE_SIZE} entries"
            )));
        }

        let current = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        let stored = current
            .get(&kind)
            .filter(|stored| stored.id == preview_id)
            .ok_or_else(expired)?;

        Ok(page(&stored.rows, filter, offset.unwrap_or(0), limit))
    }

    fn keep(&self, kind: MemoryCopyKind, stored: StoredNotesCopy) {
        self.current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(kind, stored);
    }

    /// Takes the `kind` preview `preview_id` out when it is the current one of `kind` and every id
    /// of `overwrite` is one of its conflicts that does not fail, answering the conflicts to overwrite as the push
    /// reads them on stdin (`None`: none). `all` is this preview's conflicts, never one that
    /// appeared since. Anything else leaves the preview in place.
    fn take(&self, kind: MemoryCopyKind, preview_id: &str, overwrite: &LocalNoteIds) -> Result<Option<Vec<u8>>> {
        let mut current = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        let stored = current
            .get(&kind)
            .filter(|stored| stored.id == preview_id)
            .ok_or_else(expired)?;
        let conflicts = || {
            stored
                .rows
                .iter()
                .filter(|row| row.conflict && !row.failed)
                .map(|row| row.local_id)
        };
        let entry = match kind {
            MemoryCopyKind::Notes => "note",
            MemoryCopyKind::Scripts => "script",
        };

        let picked: Vec<i64> = match overwrite {
            LocalNoteIds::Word(NoteIdsWord::All) => conflicts().collect(),
            LocalNoteIds::Word(NoteIdsWord::None) => Vec::new(),
            LocalNoteIds::Ids(ids) => {
                let fails = stored.rows.iter().any(|row| row.failed && ids.contains(&row.local_id));

                if fails {
                    return Err(BusinessError::InvalidInput(format!(
                        "a picked {entry} cannot be copied, so it cannot replace the remote one"
                    )));
                }

                if !ids.iter().all(|id| conflicts().any(|conflict| conflict == *id)) {
                    return Err(BusinessError::InvalidInput(format!(
                        "a picked {entry} is not a conflict of this review, so review the copy again"
                    )));
                }

                ids.clone()
            }
        };

        let stdin = if picked.is_empty() {
            None
        } else {
            Some(ids_on_stdin(&picked)?)
        };

        current.remove(&kind).ok_or_else(expired)?;
        Ok(stdin)
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

/// What copying every local entry of `kind` routed to the team database would do (see the module
/// docs), kept in `cache` under the preview's id, in place of the previous preview of `kind` only.
/// `notFound` when the server is not installed, `unsupported` when it predates the notes view or
/// the push of `kind` with `--json`, `process` with its (redacted) line when it fails, `protocol`
/// when its answer cannot be read.
pub async fn preview_notes_copy(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    cache: &NotesCopyCache,
    kind: MemoryCopyKind,
) -> Result<NotesCopyPreview> {
    let server = server(env, probe).await?;
    let (name, _) = command_names(kind);
    let args = push_args(kind, &["--dry-run", "--json", "--overwrite", "none"]);

    let answer = run_push(env, runner, &server, kind, push_call(name, args, None, PREVIEW_TIMEOUT)).await?;
    let mut rows = answer.rows();
    rows.sort_by_cached_key(|row| (row.repo.clone(), row.title.to_lowercase(), row.local_id));

    let preview = NotesCopyPreview {
        preview_id: uuid::Uuid::new_v4().to_string(),
        nothing: answer.nothing.clone(),
        counts: counts(&rows),
        total: u32::try_from(rows.len()).unwrap_or(u32::MAX),
        rows: page(&rows, NotesCopyFilter::All, 0, DEFAULT_PAGE_SIZE).rows,
    };

    tracing::info!(
        kind = noun(kind),
        rows = rows.len(),
        conflicts = preview.counts.conflicts,
        failed = preview.counts.failed,
        "copy to the remote database reviewed"
    );

    cache.keep(
        kind,
        StoredNotesCopy {
            id: preview.preview_id.clone(),
            rows,
        },
    );

    Ok(preview)
}

/// Copies the local entries of the `kind` preview `preview_id` to the team database (see the
/// module docs), overwriting a teammate's newer-differing text for the conflicts `overwrite` picks:
/// none, all, or those local ids (each still a conflict when the copy runs). `invalidInput`
/// (nothing copied, the preview stays) when the preview is not the current one of `kind` or an id
/// is not one of its conflicts, or is one its dry run says will fail; otherwise the preview is used up. Fails as [`preview_notes_copy`].
pub async fn apply_notes_copy(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    cache: &NotesCopyCache,
    kind: MemoryCopyKind,
    preview_id: &str,
    overwrite: &LocalNoteIds,
) -> Result<NotesCopyResult> {
    let stdin = cache.take(kind, preview_id, overwrite)?;
    let word = if stdin.is_some() { "-" } else { "none" };
    let server = server(env, probe).await?;
    let (_, name) = command_names(kind);
    let args = push_args(kind, &["--json", "--overwrite", word, "--delete-copied"]);

    let answer = run_push(env, runner, &server, kind, push_call(name, args, stdin, APPLY_TIMEOUT)).await?;
    let rows = answer.rows();
    let failed_ids = answer.failed_ids();
    let unattributed = answer.failed.iter().any(|failure| failure.local_id.is_none());

    let failed: Vec<NotesCopyFailure> = answer
        .failed
        .iter()
        .map(|failure| NotesCopyFailure {
            local_id: failure.local_id,
            title: failure
                .local_id
                .and_then(|id| rows.iter().find(|row| row.local_id == id))
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

    let result = NotesCopyResult {
        counts: counts(&rows),
        deleted: answer
            .deleted
            .unwrap_or(u32::try_from(deleted_rows).unwrap_or(u32::MAX)),
        kept: u32::try_from(kept.len()).unwrap_or(u32::MAX),
        failed,
        leftover,
    };

    tracing::info!(
        kind = noun(kind),
        deleted = result.deleted,
        kept = result.kept,
        failed = result.failed.len(),
        "copied to the remote database"
    );

    Ok(result)
}

/// The names of the dry run and of the copy of `kind`, as errors quote them.
fn command_names(kind: MemoryCopyKind) -> (&'static str, &'static str) {
    match kind {
        MemoryCopyKind::Notes => ("push --dry-run", "push"),
        MemoryCopyKind::Scripts => ("scripts push --dry-run", "scripts push"),
    }
}

/// The push of `kind` (`push` or `scripts push`) with `flags`, never a `--scope`.
fn push_args(kind: MemoryCopyKind, flags: &[&str]) -> Vec<OsString> {
    let command: &[&str] = match kind {
        MemoryCopyKind::Notes => &["push"],
        MemoryCopyKind::Scripts => &["scripts", "push"],
    };

    command.iter().chain(flags).map(OsString::from).collect()
}

fn push_call<'a>(name: &'a str, args: Vec<OsString>, stdin: Option<Vec<u8>>, timeout: Duration) -> CliCall<'a> {
    CliCall {
        name,
        args,
        timeout,
        output_bytes: PUSH_OUTPUT_BYTES,
        stdin,
    }
}
