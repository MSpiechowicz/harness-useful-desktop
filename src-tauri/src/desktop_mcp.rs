//! The app's own MCP server (`harness-desktop mcp`, agents manage hooks through it) in the
//! harnesses' user MCP files: registered in every detected harness at launch (release builds
//! only, a dev build's executable is in `target/`), its `command` following the app's program each
//! launch (AppImage moves, updates), and in one harness at the user's request
//! (`setup_register_desktop_mcp`, [`register_in`], dev builds included). An entry the user removed
//! stays removed at launch (`desktop-mcp-registration.json`). Logs name the harness and the
//! outcome, never a file's contents.

use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use harness_business::ecosystem::{
    DesktopRecord, DesktopRegistration, McpFile, current_app_command, desktop_mcp_state, mcp_target,
    register_desktop_mcp,
};
use harness_business::env::Env;
use harness_business::model::{HarnessId, McpState};
use harness_business::pi_family::tilde;
use tauri::{AppHandle, Manager};

use crate::commands::harness::summaries;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// Registers the server in the background at launch; the window does not wait for it. A dev build
/// registers nothing on its own.
pub fn spawn_on_launch(app: &AppHandle) {
    if cfg!(debug_assertions) {
        tracing::info!("desktop MCP: a dev build is registered only on request");
        return;
    }

    let app = app.clone();

    drop(tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        on_launch(&state).await;
    }));
}

/// The launch run: the app's program in every detected harness's user MCP file, as the record
/// allows. Returns what it did per harness (empty when the app's program is unknown).
pub(crate) async fn on_launch(state: &AppState) -> Vec<(HarnessId, DesktopRegistration)> {
    let Some(program) = current_app_command() else {
        tracing::warn!("desktop MCP: the app's program is unknown, nothing registered");
        return Vec::new();
    };

    register_detected(state, program).await
}

/// [`on_launch`] with the app's `program` given.
pub(crate) async fn register_detected(state: &AppState, program: PathBuf) -> Vec<(HarnessId, DesktopRegistration)> {
    let _running = state.desktop_mcp_setup.lock().await;

    let files = detected_files(state).await;
    let env = state.env.clone();
    let config_dir = state.config_dir.clone();

    let registered = tauri::async_runtime::spawn_blocking(move || {
        let mut record = DesktopRecord::load(&config_dir);

        let outcomes: Vec<_> = files
            .into_iter()
            .map(|(harness, file)| {
                (
                    harness,
                    register_noted(&env, &mut record, harness, &file, &program, true),
                )
            })
            .collect();

        save(&record);
        outcomes
    })
    .await;

    registered.unwrap_or_else(|error| {
        tracing::warn!(%error, "desktop MCP registration task failed");
        Vec::new()
    })
}

/// Registers the server in the user MCP file of `harness` because the user asked: added even
/// where the user removed it before, and recorded, so later launches keep its `command` current.
/// `unsupported` when the harness has no user MCP file, `internal` when the app's program is
/// unknown. Returns the file's state afterwards (`refused` when it could not be changed safely,
/// `manualEdit` with the entry to paste when it has comments).
pub(crate) async fn register_in(state: &AppState, harness: HarnessId) -> CommandResult<McpState> {
    let program = current_app_command().ok_or_else(|| CommandError::internal("the app's program is unknown"))?;

    register_program_in(state, harness, program).await
}

/// [`register_in`] with the app's `program` given.
pub(crate) async fn register_program_in(
    state: &AppState,
    harness: HarnessId,
    program: PathBuf,
) -> CommandResult<McpState> {
    let _running = state.desktop_mcp_setup.lock().await;

    let Some(file) = state
        .registry
        .get(harness)
        .and_then(|found| mcp_target(&state.env, found))
    else {
        return Err(BusinessError::Unsupported(format!("{harness:?} has no user MCP file")).into());
    };

    let env = state.env.clone();
    let config_dir = state.config_dir.clone();

    let registered = tauri::async_runtime::spawn_blocking(move || {
        let mut record = DesktopRecord::load(&config_dir);
        let outcome = register_noted(&env, &mut record, harness, &file, &program, false);
        save(&record);

        match outcome {
            DesktopRegistration::Refused(reason) => McpState::Refused {
                file: tilde(&env, &file.path),
                reason,
            },
            // A manual edit is read back too: the state carries the entry to paste.
            _ => desktop_mcp_state(&env, &file, Some(&program)),
        }
    })
    .await;

    registered.map_err(|error| CommandError::internal(format!("desktop MCP registration task failed: {error}")))
}

/// One registration in `file`, noted in `record`. `as_recorded` keeps an entry the user removed
/// out (launch); otherwise it is added back (the user asked).
fn register_noted(
    env: &Env,
    record: &mut DesktopRecord,
    harness: HarnessId,
    file: &McpFile,
    program: &Path,
    as_recorded: bool,
) -> DesktopRegistration {
    let recorded = as_recorded && record.is_recorded(env, &file.path);
    let outcome = register_desktop_mcp(file, program, recorded, record.command_in(env, &file.path));
    tracing::info!(?harness, ?outcome, "desktop MCP registration");

    record.note(env, harness, &file.path, outcome, program);
    outcome
}

fn save(record: &DesktopRecord) {
    if let Err(error) = record.save() {
        tracing::warn!(%error, "desktop MCP record not saved");
    }
}

/// The user MCP file of each detected harness that has one.
async fn detected_files(state: &AppState) -> Vec<(HarnessId, McpFile)> {
    let Ok(detected) = summaries(state).await else {
        tracing::warn!("desktop MCP: harness detection failed");
        return Vec::new();
    };

    let mut files = Vec::new();

    for summary in detected.iter().filter(|summary| summary.detection.installed) {
        let id = summary.info.id;

        if let Some(file) = state
            .registry
            .get(id)
            .and_then(|harness| mcp_target(&state.env, harness))
        {
            files.push((id, file));
        }
    }

    files
}

#[cfg(test)]
mod tests;
