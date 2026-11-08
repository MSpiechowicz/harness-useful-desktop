//! What a copy of the reviewed rows is told to take, for [`super::memory_notes_copy`]: the
//! conflicts to overwrite when copying what stranded on this computer ([`CopyPicks::overwrite`],
//! the local ids of conflicts of the review) or the entries to pick in a copy to either side
//! ([`CopyPicks::pick`]): the team ids of entries to add to this computer (`toLocal`) or the local
//! ids of rows to copy to the team database (`toRemote`). Either is a list, or `all`, which is the
//! review's own rows and never one that appeared since, and each id is checked against the review
//! first.
//!
//! A copy to the team database that picks (`toRemote`) names only the picked rows (`only`, each with
//! its local id, title or name and the update time the dry run printed) next to the picked conflicts
//! (`overwrite`: ticking a conflict is the choice to replace the remote text). The server takes an
//! entry only while all three still match, so an entry saved or changed after the review is never
//! copied and, with the local copies deleted, never lost. An entry not picked is not named, so it is
//! neither copied nor deleted.

use serde::Serialize;

use super::memory_hidden::{MAX_LOCAL_IDS, ids_on_stdin};
use crate::error::{BusinessError, Result};
use crate::model::{LocalNoteIds, MemoryCopyKind, NoteIdsWord, NotesCopyMode, NotesCopyRow};

/// No conflict to overwrite, for a copy that only picks.
static NOTHING_TO_OVERWRITE: LocalNoteIds = LocalNoteIds::Word(NoteIdsWord::None);

/// What a copy of one review is told to take.
#[derive(Debug, Clone, Copy)]
pub struct CopyPicks<'a> {
    /// The review the copy runs.
    pub preview_id: &'a str,
    /// The conflicts to overwrite (`stranded`); nothing in `toRemote` and `toLocal`.
    pub overwrite: &'a LocalNoteIds,
    /// The entries to copy (`toLocal` by team id, `toRemote` by local id), required there; nothing
    /// in `stranded`.
    pub pick: Option<&'a LocalNoteIds>,
}

impl<'a> CopyPicks<'a> {
    /// Overwrites `overwrite` of the review `preview_id` and picks nothing (a copy to the team
    /// database).
    pub fn overwriting(preview_id: &'a str, overwrite: &'a LocalNoteIds) -> Self {
        Self {
            preview_id,
            overwrite,
            pick: None,
        }
    }

    /// Picks `pick` of the review `preview_id` and overwrites nothing (a copy to this computer, or
    /// the picked rows to the team database).
    pub fn picking(preview_id: &'a str, pick: &'a LocalNoteIds) -> Self {
        Self {
            preview_id,
            overwrite: &NOTHING_TO_OVERWRITE,
            pick: Some(pick),
        }
    }

    /// What goes on the standard input of the copy of `rows` in `mode`: `None` for no input (a
    /// `stranded` copy with no conflict to overwrite), else the ids as the command reads them (for
    /// `toRemote` the selection of the picked rows, see [`selection_stdin`]). `invalidInput` for an
    /// id that is not one of the rows to take, a pick that does not fit `mode`, or a pick of
    /// nothing where one is required.
    pub(super) fn stdin(
        &self,
        kind: MemoryCopyKind,
        mode: NotesCopyMode,
        rows: &[NotesCopyRow],
    ) -> Result<Option<Vec<u8>>> {
        match mode {
            NotesCopyMode::Stranded => {
                if self.picks_anything() {
                    return Err(invalid(
                        "a copy of what stayed here picks conflicts to overwrite, not entries".to_owned(),
                    ));
                }

                let overwrite = overwrite_ids(kind, rows, self.overwrite)?;

                if overwrite.is_empty() {
                    return Ok(None);
                }

                ids_on_stdin(&overwrite).map(Some)
            }
            NotesCopyMode::ToRemote => {
                if *self.overwrite != LocalNoteIds::Word(NoteIdsWord::None) {
                    return Err(invalid(
                        "a copy to the remote database overwrites what is picked, so pick the conflicts".to_owned(),
                    ));
                }

                let picked = picked_ids(rows, self.pick, |row| row.local_id, Refusals::to_remote(kind))?;

                selection_stdin(rows, &picked).map(Some)
            }
            NotesCopyMode::ToLocal => {
                if *self.overwrite != LocalNoteIds::Word(NoteIdsWord::None) {
                    return Err(invalid("a copy to this computer overwrites nothing".to_owned()));
                }

                let picked = picked_ids(rows, self.pick, |row| row.team_id, Refusals::to_local(kind))?;

                Ok(Some(pull_stdin(&picked)))
            }
        }
    }

    /// Whether a pick names something (or everything).
    fn picks_anything(&self) -> bool {
        self.pick
            .is_some_and(|pick| *pick != LocalNoteIds::Word(NoteIdsWord::None))
    }
}

/// The word `kind` goes by in messages.
fn entry(kind: MemoryCopyKind) -> &'static str {
    match kind {
        MemoryCopyKind::Notes => "note",
        MemoryCopyKind::Scripts => "script",
    }
}

fn invalid(message: String) -> BusinessError {
    BusinessError::InvalidInput(message)
}

/// The local ids of the conflicts of `rows` that `overwrite` picks. `all` is the conflicts of `rows`.
/// `invalidInput` for an id that is not a conflict of `rows` or whose copy will fail.
fn overwrite_ids(kind: MemoryCopyKind, rows: &[NotesCopyRow], overwrite: &LocalNoteIds) -> Result<Vec<i64>> {
    let entry = entry(kind);
    let conflicts = || {
        rows.iter()
            .filter(|row| row.conflict && !row.failed)
            .filter_map(|row| row.local_id)
    };

    let picked: Vec<i64> = match overwrite {
        LocalNoteIds::Word(NoteIdsWord::All) => conflicts().collect(),
        LocalNoteIds::Word(NoteIdsWord::None) => Vec::new(),
        LocalNoteIds::Ids(ids) => {
            let fails = rows
                .iter()
                .any(|row| row.failed && row.local_id.is_some_and(|id| ids.contains(&id)));

            if fails {
                return Err(invalid(format!(
                    "a picked {entry} cannot be copied, so it cannot replace the remote one"
                )));
            }

            if !ids.iter().all(|id| conflicts().any(|conflict| conflict == *id)) {
                return Err(invalid(format!(
                    "a picked {entry} is not a conflict of this review, so review the copy again"
                )));
            }

            ids.clone()
        }
    };

    Ok(picked)
}

/// One reviewed entry of `only`, as the dry run printed it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Reviewed<'a> {
    id: i64,
    /// A note's title, a script's name.
    name: &'a str,
    /// The `localUpdatedAt` of the dry run, as printed.
    updated_at: &'a str,
}

/// The selection a copy to the team database reads on stdin: `{"only": [{"id", "name",
/// "updatedAt"}, ...], "overwrite": [...]}`, `only` the rows of `rows` whose local id is `picked`
/// (taken from the review, never read again, the update time as the dry run printed it) and
/// `overwrite` the conflicts among them. A picked row without an update time is not named, so it is
/// not copied. `invalidInput` for an id that is not positive or for more than [`MAX_LOCAL_IDS`]
/// entries.
fn selection_stdin(rows: &[NotesCopyRow], picked: &[i64]) -> Result<Vec<u8>> {
    let mut only: Vec<(Reviewed<'_>, bool)> = rows
        .iter()
        .filter(|row| !row.failed && row.local_id.is_some_and(|id| picked.contains(&id)))
        .filter_map(|row| {
            let reviewed = Reviewed {
                id: row.local_id?,
                name: &row.title,
                updated_at: row.local_updated_at.as_deref().filter(|at| !at.is_empty())?,
            };

            Some((reviewed, row.conflict))
        })
        .collect();

    only.sort_unstable_by_key(|(entry, _)| entry.id);
    only.dedup_by_key(|(entry, _)| entry.id);

    let overwrite: Vec<i64> = only
        .iter()
        .filter(|(_, conflict)| *conflict)
        .map(|(entry, _)| entry.id)
        .collect();
    let only: Vec<Reviewed<'_>> = only.into_iter().map(|(entry, _)| entry).collect();

    if only.len() > MAX_LOCAL_IDS {
        return Err(invalid(format!("copy at most {MAX_LOCAL_IDS} at a time")));
    }

    if only.iter().any(|entry| entry.id <= 0) {
        return Err(invalid("a local id is a positive number".to_owned()));
    }

    serde_json::to_vec(&serde_json::json!({ "only": only, "overwrite": overwrite }))
        .map_err(|error| invalid(format!("the selection could not be written: {error}")))
}

/// What a refused pick says.
struct Refusals {
    /// An id whose copy will fail.
    fails: String,
    /// An id that is not a row to pick of the review.
    outside: String,
    /// Nothing picked.
    nothing: String,
}

impl Refusals {
    fn to_local(kind: MemoryCopyKind) -> Self {
        let entry = entry(kind);

        Self {
            fails: format!("a picked {entry} cannot be copied to this computer"),
            outside: format!("a picked {entry} is not one to add in this review, so review the copy again"),
            nothing: format!("pick at least one {entry} to copy to this computer"),
        }
    }

    fn to_remote(kind: MemoryCopyKind) -> Self {
        let entry = entry(kind);

        Self {
            fails: format!("a picked {entry} cannot be copied to the remote database"),
            outside: format!("a picked {entry} is not one to copy in this review, so review the copy again"),
            nothing: format!("pick at least one {entry} to copy to the remote database"),
        }
    }
}

/// The ids (by `key`) of the pickable rows of `rows` that `pick` names, sorted and without
/// repeats. `all` is the pickable rows of the review. `invalidInput` for no pick, an id that is
/// not a pickable row of `rows` or whose copy will fail, nothing picked, or more than
/// [`MAX_LOCAL_IDS`].
fn picked_ids(
    rows: &[NotesCopyRow],
    pick: Option<&LocalNoteIds>,
    key: fn(&NotesCopyRow) -> Option<i64>,
    refusals: Refusals,
) -> Result<Vec<i64>> {
    let pickable = || rows.iter().filter(|row| row.pickable).filter_map(key);

    let mut picked: Vec<i64> = match pick {
        Some(LocalNoteIds::Word(NoteIdsWord::All)) => pickable().collect(),
        Some(LocalNoteIds::Ids(ids)) => {
            if rows
                .iter()
                .any(|row| row.failed && key(row).is_some_and(|id| ids.contains(&id)))
            {
                return Err(invalid(refusals.fails));
            }

            if !ids.iter().all(|id| pickable().any(|key| key == *id)) {
                return Err(invalid(refusals.outside));
            }

            ids.clone()
        }
        Some(LocalNoteIds::Word(NoteIdsWord::None)) | None => Vec::new(),
    };

    picked.sort_unstable();
    picked.dedup();

    if picked.is_empty() {
        return Err(invalid(refusals.nothing));
    }

    if picked.len() > MAX_LOCAL_IDS {
        return Err(invalid(format!("pick at most {MAX_LOCAL_IDS} at a time")));
    }

    Ok(picked)
}

/// `picked` team ids, one per line as `pull --ids -` reads them.
fn pull_stdin(picked: &[i64]) -> Vec<u8> {
    let lines: Vec<String> = picked.iter().map(i64::to_string).collect();

    format!("{}\n", lines.join("\n")).into_bytes()
}
