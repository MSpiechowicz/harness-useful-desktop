//! Copying notes or scripts between this computer and the team database (`memory_notes_copy_preview`,
//! `memory_notes_copy_rows`, `memory_notes_copy_apply`, each for one `kind` and one `mode`), through
//! the installed `harness-memory push --json`, `scripts push --json`, `pull --json` or `scripts
//! pull --json` (120 s for the dry run, 300 s for the copy). `mode` is `stranded` (the default: what
//! stayed here after a scope moved to the team database, copied there and deleted here), `toRemote`
//! (the picked entries of the scopes kept here too, copied there and deleted here, needs the Settings
//! switch `memoryCopyToRemote`) or `toLocal` (the team's entries missing here, copied here, needs
//! `memoryCopyToLocal`). Rust reads the switches from the preferences on every call and refuses
//! (`forbidden`) a direction that is off, then a server without the feature (`unsupported`). It
//! keeps the reviewed rows under the preview id (one review per kind and mode, replaced by every
//! new one of that target, used up by its copy) and pages them: the webview sends back ids only.
//! Logs carry counts only, never titles or names.

use harness_business::ecosystem::{CopyGate, CopyPicks, CopyTarget, apply_notes_copy, preview_notes_copy};
use harness_business::model::{
    LocalNoteIds, MemoryCopyKind, NoteIdsWord, NotesCopyFilter, NotesCopyMode, NotesCopyPreview, NotesCopyResult,
    NotesCopyRows,
};
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Reviews copying the entries of `kind` in `mode` (a dry run, every repository counted): the
/// counts and the first page of rows, kept under `previewId`. `mode` is `stranded` unless given:
/// every entry saved on this computer and routed to the team database alone (notes of such scopes,
/// or scripts) goes to the team database. `toRemote` is the entries of the scopes kept on this
/// computer too, and `toLocal` the team's entries of those scopes that are missing here, one row
/// each by `teamId`. `nothing` holds the server's reason when there is nothing to copy.
/// `forbidden` when the switch of `toRemote` or `toLocal` is off, `notFound` when the server is not
/// installed, `unsupported` when it predates the notes view, copying `kind` or the direction,
/// `process` with the server's line when it fails.
#[tauri::command]
pub async fn memory_notes_copy_preview(
    state: State<'_, AppState>,
    kind: MemoryCopyKind,
    mode: Option<NotesCopyMode>,
) -> CommandResult<NotesCopyPreview> {
    let mode = mode.unwrap_or_default();

    Ok(preview_notes_copy(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        &copy_gate(&state),
        &state.notes_copy,
        CopyTarget { kind, mode },
    )
    .await?)
}

/// One page of the rows of the `kind` and `mode` review `previewId` (`all` unless `filter` says
/// `conflicts`, `failed` or `pickable`), grouped by repository (global entries first) and sorted by
/// title or name. `invalidInput` for a limit of 0 or above 200, or a review that is not the
/// current one of `kind` and `mode`.
#[tauri::command]
pub fn memory_notes_copy_rows(
    state: State<'_, AppState>,
    kind: MemoryCopyKind,
    mode: Option<NotesCopyMode>,
    preview_id: String,
    filter: Option<NotesCopyFilter>,
    offset: Option<u32>,
    limit: Option<u32>,
) -> CommandResult<NotesCopyRows> {
    let target = CopyTarget {
        kind,
        mode: mode.unwrap_or_default(),
    };

    Ok(state
        .notes_copy
        .rows(target, &preview_id, filter.unwrap_or_default(), offset, limit)?)
}

/// Copies the entries of the `kind` and `mode` review `previewId`. To the team database the local
/// copies of those added, updated, overwritten or already there are deleted. `stranded` copies
/// every row of the review, and `overwrite` picks the conflicts whose teammate text is replaced:
/// `"none"`, `"all"` (the review's conflicts), or those local ids (each only if still a conflict
/// when the copy runs); the others stay on this computer, as do entries older than the team's.
/// `toRemote` and `toLocal` copy only what `pick` names, at least one entry, each a pickable row of
/// the review (`pickable` in its rows) that does not fail, or `"all"` (the review's pickable rows).
/// `toRemote` picks by local id: a picked conflict replaces the remote text, an entry not picked is
/// neither copied nor deleted, and `overwrite` must be absent or `"none"`. `toLocal` picks the team
/// ids of entries to add: the team's entries stay, nothing is deleted and `overwrite` must be absent
/// or `"none"` too. `invalidInput` (nothing copied, the review stays) when `previewId` is not the
/// current review of `kind` and `mode`, an id is not one of its pickable rows, `toRemote` or
/// `toLocal` picks nothing, or `pick` is given to `stranded`; otherwise the review is used up.
/// `forbidden` when the switch is off, `notFound` when the server is not installed, `unsupported`
/// when it predates copying `kind` or the direction, `process` with the server's line when it fails.
#[tauri::command]
pub async fn memory_notes_copy_apply(
    state: State<'_, AppState>,
    kind: MemoryCopyKind,
    mode: Option<NotesCopyMode>,
    preview_id: String,
    overwrite: Option<LocalNoteIds>,
    pick: Option<LocalNoteIds>,
) -> CommandResult<NotesCopyResult> {
    let overwrite = overwrite.unwrap_or(LocalNoteIds::Word(NoteIdsWord::None));
    let picks = CopyPicks {
        preview_id: &preview_id,
        overwrite: &overwrite,
        pick: pick.as_ref(),
    };

    Ok(apply_notes_copy(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        &copy_gate(&state),
        &state.notes_copy,
        CopyTarget {
            kind,
            mode: mode.unwrap_or_default(),
        },
        &picks,
    )
    .await?)
}

/// The gate of a copy: the Settings switches, read from the preferences now, and the server's features.
fn copy_gate(state: &AppState) -> CopyGate<'_> {
    let prefs = state.prefs.with(|store| store.load());

    CopyGate {
        features: &state.memory_features,
        to_remote: prefs.memory_copy_to_remote,
        to_local: prefs.memory_copy_to_local,
    }
}
