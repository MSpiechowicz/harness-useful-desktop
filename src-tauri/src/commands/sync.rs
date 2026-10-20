//! "Share setup": export this machine's marketplaces, plugins, packages and MCP servers as a `hud1:`
//! string (`sync_export`) or a `.json` file (`sync_export_save`), and import them after a review
//! (`sync_import_preview`, then `sync_import_apply`).
//!
//! The webview never sends a bundle to export or apply: exports are rebuilt from the harnesses'
//! files for the item ids it sends, and an import applies the plan Rust stored at review time (one
//! at a time, replaced by every new review, consumed by its apply). Files are only written where
//! the user chose in a native save dialog and only read from a native file picker. Logs carry
//! counts only, never sources, commands or contents.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, PoisonError};
use std::time::SystemTime;

use harness_business::BusinessError;
use harness_business::agent::rfc3339;
use harness_business::fs_atomic::replace_atomically;
use harness_business::model::{
    HarnessId, SyncExportPreview, SyncExported, SyncImportPlan, SyncImportResult, SyncInput, SyncProgress,
};
use harness_business::process::{ProcessRunner, ResolvedBinary};
use harness_business::security::SafeReader;
use harness_business::sync::codec::{self, DecodedBundle};
use harness_business::sync::schema::{Bundle, MAX_JSON_BYTES};
use harness_business::sync::{
    ApplyContext, OpenCodeMerged, StoredPlan, apply_plan, build_bundle, build_plan, collect_export, export_code,
    export_preview,
};
use tauri::async_runtime::block_on;
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime, State};

use super::harness::{detected_binary, summaries};
use super::{blocking, saved_language};
use crate::dialogs::{self, FileFilter};
use crate::error::CommandResult;
use crate::native_text::native_text;
use crate::state::AppState;

/// What this machine can share, per installed harness, and what is left out (with a reason, never
/// a value). Read-only.
#[tauri::command]
pub async fn sync_export_preview(state: State<'_, AppState>) -> CommandResult<SyncExportPreview> {
    shareable(&state).await
}

async fn shareable(state: &AppState) -> CommandResult<SyncExportPreview> {
    let summaries = summaries(state).await?;
    let env = state.env.clone();
    let registry = state.registry.clone();

    blocking(move || Ok(export_preview(&collect_export(&env, &registry, &summaries)))).await
}

/// The `hud1:` string of the `selected` item ids. `invalidInput` when none of them can be shared
/// any more or the string would be over 64 KiB (the message says to save a file instead).
#[tauri::command]
pub async fn sync_export(
    app: AppHandle,
    state: State<'_, AppState>,
    selected: Vec<String>,
) -> CommandResult<SyncExported> {
    export(&state, selected, app_version(&app)).await
}

async fn export(state: &AppState, selected: Vec<String>, app_version: String) -> CommandResult<SyncExported> {
    let bundle = bundle(state, selected, app_version, today()).await?;
    let exported = export_code(&bundle)?;

    tracing::info!(
        items = exported.item_count,
        bytes = exported.bytes,
        "share setup exported as a string"
    );

    Ok(exported)
}

/// Writes the setup file of the `selected` item ids where the user chooses in a native save dialog
/// (`harness-setup-<date>.json` by default). The path written, or `None` when the user cancelled.
#[tauri::command]
pub async fn sync_export_save(
    app: AppHandle,
    state: State<'_, AppState>,
    selected: Vec<String>,
) -> CommandResult<Option<String>> {
    let date = today();
    let bundle = bundle(&state, selected, app_version(&app), date.clone()).await?;
    let json = codec::to_json_pretty(&bundle)?;

    let text = native_text(&saved_language(&state).await?);
    let filter = FileFilter {
        name: text.setup_files,
        extensions: &["json"],
    };
    let default_name = format!("harness-setup-{date}.json");

    let chosen = dialogs::save(
        &app,
        &state.dialog,
        text.save_setup_file,
        Some(&state.env.home),
        &default_name,
        filter,
    )
    .await?;
    let Some(path) = chosen else {
        return Ok(None);
    };

    let saved = blocking(move || {
        write_atomically(&path, json.as_bytes())?;

        Ok(path.display().to_string())
    })
    .await?;

    tracing::info!(items = bundle.item_count(), "share setup saved as a file");

    Ok(Some(saved))
}

/// The bundle of `selected`, rebuilt from the harnesses' files (never from the webview).
async fn bundle(state: &AppState, selected: Vec<String>, app_version: String, date: String) -> CommandResult<Bundle> {
    let summaries = summaries(state).await?;
    let env = state.env.clone();
    let registry = state.registry.clone();

    blocking(move || {
        let collection = collect_export(&env, &registry, &summaries);

        Ok(build_bundle(&collection, &selected, Some(app_version), Some(date))?)
    })
    .await
}

/// Reviews a setup: pasted text, or a file the user picks in a native dialog (`None` when
/// cancelled). The plan is kept under its `planId` for `sync_import_apply`, replacing any earlier
/// one. A rejected input is `invalidInput` (`unsupported` when a newer app made it), and the
/// message never repeats the input.
#[tauri::command]
pub async fn sync_import_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    input: SyncInput,
) -> CommandResult<Option<SyncImportPlan>> {
    let decoded = match input {
        SyncInput::Text { text } => decode_text(text).await?,
        SyncInput::File => {
            let Some(bytes) = read_picked_file(&app, &state).await? else {
                return Ok(None);
            };

            blocking(move || Ok(codec::decode_file(&bytes).map_err(BusinessError::from)?)).await?
        }
    };

    preview(&state, decoded).await.map(Some)
}

async fn decode_text(text: String) -> CommandResult<DecodedBundle> {
    blocking(move || Ok(codec::decode_input(&text).map_err(BusinessError::from)?)).await
}

/// The bytes of a setup file picked in a native dialog, read through the deny-list and capped just
/// over the file limit (so `decode_file` reports a larger one as too large).
async fn read_picked_file<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> CommandResult<Option<Vec<u8>>> {
    let text = native_text(&saved_language(state).await?);
    let filter = FileFilter {
        name: text.setup_files,
        extensions: &["json", "txt"],
    };

    let picked = dialogs::pick_file(app, &state.dialog, text.open_setup_file, Some(&state.env.home), filter).await?;
    let Some(path) = picked else {
        return Ok(None);
    };

    blocking(move || {
        let mut bytes = Vec::new();
        SafeReader::open(&path)?
            .take(MAX_JSON_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| BusinessError::from_io("reading the setup file", error))?;

        Ok(Some(bytes))
    })
    .await
}

/// Builds the review of `decoded` and keeps its steps as the current plan.
async fn preview(state: &AppState, decoded: DecodedBundle) -> CommandResult<SyncImportPlan> {
    let summaries = summaries(state).await?;
    let opencode = opencode_merged(state, &decoded).await;
    let env = state.env.clone();
    let registry = state.registry.clone();

    let built = blocking(move || Ok(build_plan(&env, &registry, &summaries, &decoded, &opencode))).await?;
    *state.sync_plan.lock().unwrap_or_else(PoisonError::into_inner) = Some(built.stored);

    Ok(built.plan)
}

/// The MCP server names OpenCode merges from all its config layers, checked only when `decoded`
/// shares an OpenCode server. Unknown (no OpenCode server is offered) when OpenCode is not detected
/// or the check fails.
async fn opencode_merged(state: &AppState, decoded: &DecodedBundle) -> OpenCodeMerged {
    if !OpenCodeMerged::needed(decoded) {
        return OpenCodeMerged::Unknown;
    }

    let binary = detected_binary(state, HarnessId::OpenCode).await.ok().flatten();

    OpenCodeMerged::probe(&state.env, &state.runner, binary.as_ref()).await
}

/// Applies the `selected` items of the reviewed plan `plan_id`, streaming each item's start and end
/// to `on_progress`. `invalidInput` (nothing runs) unless `confirmed`, while another import runs,
/// or when `plan_id` is not the current review; those refusals leave the current review in place.
/// The plan is used up once it is taken for an attempt (confirmed, no other import running, the
/// current `plan_id`), whether that attempt then applies, fails or is refused for its selection.
#[tauri::command]
pub async fn sync_import_apply(
    state: State<'_, AppState>,
    plan_id: String,
    selected: Vec<String>,
    confirmed: bool,
    on_progress: Channel<SyncProgress>,
) -> CommandResult<SyncImportResult> {
    let runner: Arc<dyn ProcessRunner> = Arc::new(state.runner);

    apply(&state, runner, &plan_id, selected, confirmed, on_progress).await
}

async fn apply(
    state: &AppState,
    runner: Arc<dyn ProcessRunner>,
    plan_id: &str,
    selected: Vec<String>,
    confirmed: bool,
    on_progress: Channel<SyncProgress>,
) -> CommandResult<SyncImportResult> {
    if !confirmed {
        return Err(BusinessError::InvalidInput("the user has not confirmed the import".to_owned()).into());
    }

    let Ok(_applying) = state.sync_apply.try_lock() else {
        return Err(BusinessError::InvalidInput("an import is already running".to_owned()).into());
    };

    let stored = take_plan(state, plan_id)?;
    let binaries = resolve_binaries(state, stored.harnesses()).await?;
    let env = state.env.clone();
    let registry = state.registry.clone();

    tracing::info!(
        selected = selected.len(),
        harnesses = binaries.len(),
        "share setup import started"
    );

    blocking(move || {
        let context = ApplyContext {
            env: &env,
            registry: &registry,
            runner: runner.as_ref(),
            binaries: &binaries,
        };

        let applied = block_on(apply_plan(context, &stored, &selected, confirmed, |progress| {
            // A closed channel only means the page went away; the import goes on.
            on_progress.send(progress).ok();
        }));

        Ok(applied?)
    })
    .await
}

/// Takes the stored plan out of the state when it is `plan_id`; any other id leaves it in place.
fn take_plan(state: &AppState, plan_id: &str) -> CommandResult<StoredPlan> {
    let mut current = state.sync_plan.lock().unwrap_or_else(PoisonError::into_inner);

    current
        .take_if(|plan| plan.id() == plan_id)
        .ok_or_else(|| BusinessError::InvalidInput("this review has expired; review the setup again".to_owned()).into())
}

/// The detected binary of each of `harnesses` that is installed; the CLI items of the others end
/// `skipped: harnessNotInstalled`. A detection that fails fails the whole import (nothing runs).
async fn resolve_binaries(
    state: &AppState,
    harnesses: Vec<HarnessId>,
) -> CommandResult<BTreeMap<HarnessId, ResolvedBinary>> {
    let mut binaries = BTreeMap::new();

    for harness in harnesses {
        if let Some(binary) = detected_binary(state, harness).await? {
            binaries.insert(harness, binary);
        }
    }

    Ok(binaries)
}

fn app_version<R: Runtime>(app: &AppHandle<R>) -> String {
    app.package_info().version.to_string()
}

/// Today's date (UTC) as `YYYY-MM-DD`.
fn today() -> String {
    let mut stamp = rfc3339(SystemTime::now());
    stamp.truncate("YYYY-MM-DD".len());

    stamp
}

/// Replaces `target` with `bytes` in one rename (a private temporary sibling, flushed first), so a
/// failure never leaves a half-written setup file at `target`.
fn write_atomically(target: &Path, bytes: &[u8]) -> CommandResult<()> {
    Ok(replace_atomically(target, bytes, "saving the setup file")?)
}

#[cfg(test)]
mod tests;
