//! Copying the notes or scripts saved on this computer to the team database after a note scope or
//! the scripts moved there (`memory_notes_copy_preview`, `memory_notes_copy_rows`,
//! `memory_notes_copy_apply`, each for one `kind`), through the installed `harness-memory push
//! --json` or `scripts push --json` (120 s for the dry run, 300 s for the copy). Rust keeps the
//! reviewed rows under the preview id (one review per kind, replaced by every new one of that kind,
//! used up by its copy) and pages them: the webview sends back the conflicts' local ids only. Logs
//! carry counts only, never titles or names.

use harness_business::ecosystem::{apply_notes_copy, preview_notes_copy};
use harness_business::model::{
    LocalNoteIds, MemoryCopyKind, NotesCopyFilter, NotesCopyPreview, NotesCopyResult, NotesCopyRows,
};
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Reviews copying every entry of `kind` saved on this computer and routed to the team database
/// alone (notes of such scopes, or scripts) to the team database (a dry run, every repository
/// counted): the counts and the first page of rows, kept under `previewId`. `nothing` holds the
/// server's reason when there is nothing to copy. `notFound` when the server is not installed,
/// `unsupported` when it predates the notes view or copying `kind`, `process` with the server's
/// line when it fails.
#[tauri::command]
pub async fn memory_notes_copy_preview(
    state: State<'_, AppState>,
    kind: MemoryCopyKind,
) -> CommandResult<NotesCopyPreview> {
    Ok(preview_notes_copy(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        &state.notes_copy,
        kind,
    )
    .await?)
}

/// One page of the rows of the `kind` review `previewId` (`all` unless `filter` says
/// `conflicts`), grouped by repository (global entries first) and sorted by title or name.
/// `invalidInput` for a limit of 0 or above 200, or a review that is not the current one of `kind`.
#[tauri::command]
pub fn memory_notes_copy_rows(
    state: State<'_, AppState>,
    kind: MemoryCopyKind,
    preview_id: String,
    filter: Option<NotesCopyFilter>,
    offset: Option<u32>,
    limit: Option<u32>,
) -> CommandResult<NotesCopyRows> {
    Ok(state
        .notes_copy
        .rows(kind, &preview_id, filter.unwrap_or_default(), offset, limit)?)
}

/// Copies the entries of the `kind` review `previewId` to the team database and deletes the local
/// copies of those added, updated, overwritten or already there. `overwrite` picks the conflicts
/// whose teammate text is replaced: `"none"`, `"all"` (the review's conflicts), or those local ids
/// (each only if still a conflict when the copy runs); the others stay on this computer, as do
/// entries older than the team's. `invalidInput` (nothing copied, the review stays) when
/// `previewId` is not the current review of `kind` or an id is not one of its conflicts; otherwise
/// the review is used up. `notFound` when the server is not installed, `unsupported` when it
/// predates copying `kind`, `process` with the server's line when it fails.
#[tauri::command]
pub async fn memory_notes_copy_apply(
    state: State<'_, AppState>,
    kind: MemoryCopyKind,
    preview_id: String,
    overwrite: LocalNoteIds,
) -> CommandResult<NotesCopyResult> {
    Ok(apply_notes_copy(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        &state.notes_copy,
        kind,
        &preview_id,
        &overwrite,
    )
    .await?)
}
