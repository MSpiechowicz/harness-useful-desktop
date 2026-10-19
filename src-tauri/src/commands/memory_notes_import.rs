//! Importing notes from a CSV or JSON file (`memory_notes_import_preview`, then
//! `memory_notes_import_apply`), as "Share setup" imports a setup file (see [`super::sync`]).
//!
//! The file is only read from a native file picker, through the deny-list and capped just over
//! 2 MiB. Rust keeps the reviewed rows under the plan id (one review at a time, replaced by every
//! new one, used up by its import) with the project it was reviewed for: the webview sends back
//! row indexes only, never rows. One import runs at a time. Logs carry counts only, never titles
//! or bodies.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::PoisonError;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    MAX_IMPORT_BYTES, NotesImportRequest, StoredNotesImport, apply_notes_import, read_notes_file,
};
use harness_business::model::{NotesImportPreview, NotesImportProgress, NotesImportResult, ScriptScope};
use harness_business::security::SafeReader;
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime, State};

use super::memory_scripts::project_root;
use super::{blocking, saved_language};
use crate::dialogs::{self, FileFilter};
use crate::error::CommandResult;
use crate::native_text::native_text;
use crate::state::AppState;

/// Reviews a CSV or JSON file of notes the user picks in a native dialog (`None` when cancelled)
/// for `project` (null for global notes only): `plan`, whose rows are kept under its `planId` for
/// `memory_notes_import_apply`, or `refused` with the reason (code) when the file cannot be read
/// as notes (over 2 MiB, not UTF-8, blank or without notes, a CSV file without a `title` and a
/// `body` column or with a broken record, JSON that does not parse or has another shape). Either
/// way any earlier review is dropped. `forbidden` for a project that is not open or recent or a
/// file on the deny-list, `io` when the file cannot be read.
#[tauri::command]
pub async fn memory_notes_import_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    project: Option<String>,
) -> CommandResult<Option<NotesImportPreview>> {
    let project = project_root(&state, project).await?;

    let Some(path) = pick_notes_file(&app, &state).await? else {
        return Ok(None);
    };

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let bytes = blocking(move || read_capped(&path)).await?;

    preview(&state, project, file_name, bytes).await.map(Some)
}

async fn pick_notes_file<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> CommandResult<Option<PathBuf>> {
    let text = native_text(&saved_language(state).await?);
    let filter = FileFilter {
        name: text.notes_files,
        extensions: &["csv", "tsv", "json"],
    };

    dialogs::pick_file(app, &state.dialog, text.open_notes_file, Some(&state.env.home), filter).await
}

/// The bytes of `path`, read through the deny-list and capped one byte over the size limit (so
/// a larger file is refused as too large).
fn read_capped(path: &Path) -> CommandResult<Vec<u8>> {
    let mut bytes = Vec::new();

    SafeReader::open(path)?
        .take(MAX_IMPORT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| BusinessError::from_io("reading the notes file", error))?;

    Ok(bytes)
}

/// Builds the review of `bytes` and keeps its rows as the current notes import (none when the
/// file is refused).
async fn preview(
    state: &AppState,
    project: Option<PathBuf>,
    file_name: String,
    bytes: Vec<u8>,
) -> CommandResult<NotesImportPreview> {
    let read = blocking(move || {
        Ok(read_notes_file(&file_name, &bytes, project.as_deref()).map_err(|refusal| (file_name, refusal)))
    })
    .await?;
    let mut current = state.notes_import.lock().unwrap_or_else(PoisonError::into_inner);

    match read {
        Ok(read) => {
            *current = Some(read.stored);

            Ok(NotesImportPreview::Plan { plan: read.plan })
        }
        Err((file_name, refusal)) => {
            *current = None;

            Ok(NotesImportPreview::Refused {
                file_name,
                reason: refusal.reason,
                line: refusal.line,
            })
        }
    }
}

/// Imports the reviewed rows `rows` of plan `plan_id` in the file's order, each with the scope
/// the file gives or `default_scope`, streaming each row's end to `on_progress`. A row whose
/// title a note of its scope has is skipped (`exists`), or with `replace` that note is rewritten
/// (`replaced`). `invalidInput`
/// (nothing saved, the review stays) when `plan_id` is not the current review, `project` is not
/// the project it was made for, or another import is running. Otherwise the plan is used up, also when the import is then
/// refused for its selection (`invalidInput`: empty, or a row with problems or not in the file)
/// or ends early because the server is missing or too old (`notFound`, `unsupported`; the rows
/// saved before stay saved). `forbidden` for a project that is not open or recent.
#[tauri::command]
pub async fn memory_notes_import_apply(
    state: State<'_, AppState>,
    plan_id: String,
    rows: Vec<u32>,
    default_scope: ScriptScope,
    replace: bool,
    project: Option<String>,
    on_progress: Channel<NotesImportProgress>,
) -> CommandResult<NotesImportResult> {
    let request = NotesImportRequest {
        rows,
        default_scope,
        replace,
    };

    apply(&state, &plan_id, &request, project, on_progress).await
}

async fn apply(
    state: &AppState,
    plan_id: &str,
    request: &NotesImportRequest,
    project: Option<String>,
    on_progress: Channel<NotesImportProgress>,
) -> CommandResult<NotesImportResult> {
    let project = project_root(state, project).await?;

    let Ok(_applying) = state.notes_import_apply.try_lock() else {
        return Err(BusinessError::InvalidInput("an import is already running".to_owned()).into());
    };

    let stored = take_plan(state, plan_id, project.as_deref())?;

    let result = apply_notes_import(
        &state.env,
        &state.memory_scripts,
        &state.runner,
        &stored,
        request,
        |progress| {
            // A closed channel only means the page went away; the import goes on.
            on_progress.send(progress).ok();
        },
    )
    .await?;

    Ok(result)
}

/// Takes the current review out of the state when it is `plan_id`, made for `project`; anything
/// else leaves it in place.
fn take_plan(state: &AppState, plan_id: &str, project: Option<&Path>) -> CommandResult<StoredNotesImport> {
    let mut current = state.notes_import.lock().unwrap_or_else(PoisonError::into_inner);

    if let Some(plan) = current.take_if(|plan| plan.id() == plan_id && plan.project() == project) {
        return Ok(plan);
    }

    let message = if current.as_ref().is_some_and(|plan| plan.id() == plan_id) {
        "this file was reviewed for another project, so review it again"
    } else {
        "this review has expired, so review the file again"
    };

    Err(BusinessError::InvalidInput(message.to_owned()).into())
}

#[cfg(test)]
mod tests;
