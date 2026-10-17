//! Project memory: the installed `harness-memory` MCP server (Harness Memory, found with
//! `installed_memory`, so only once it reports its version), registered in every detected harness's
//! user MCP file at launch and after its install (`setup_install_memory`) while the
//! `memoryEnabled` pref is on, and unregistered when the user turns it off (`memory_set_enabled`). While the server is not installed nothing is
//! registered; the UI offers its installer. An entry in a file with comments (OpenCode's JSONC) is
//! never rewritten: turning memory off reports it for the user to remove by hand. The setup step
//! also registers it in one harness at the user's request (`setup_register_memory`, [`register_in`]).
//!
//! Each launch also retires the copy of the bundled server older builds kept
//! (`migrate_legacy_memory`). Logs name the harness and the outcome, never a file's contents.

use std::path::PathBuf;

use harness_business::BusinessError;
use harness_business::ecosystem::catalog::MEMORY_MCP_SERVER;
use harness_business::ecosystem::{
    Blocked, McpFile, MemoryRecord, MemoryRegistration, installed_memory, mcp_target, mcp_target_paths,
    memory_mcp_state, migrate_legacy_memory, register_memory_mcp, unregister_memory_mcp,
};
use harness_business::model::{HarnessId, ManualRemoval, McpState};
use harness_business::pi_family::tilde;
use tauri::{AppHandle, Manager};

use crate::commands::harness::summaries;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// The MCP file of a harness (with the files it also reads).
type Target = (HarnessId, McpFile);

/// What a registration did per harness; `None` when the server is not installed (nothing
/// registered).
pub(crate) type Registered = Option<Vec<(HarnessId, MemoryRegistration)>>;

/// Registers the server and retires the old copy in the background; the window does not wait for
/// it.
pub fn spawn_on_launch(app: &AppHandle) {
    let app = app.clone();

    drop(tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        on_launch(&state).await;
    }));
}

/// The launch run (see [`apply_pref`]). The pref is read once the setup lock is held, so a
/// switch-off that got the lock first is not undone.
pub(crate) async fn on_launch(state: &AppState) -> Registered {
    let _running = state.memory_setup.lock().await;

    apply_pref(state).await
}

/// After the server's installer succeeded: [`apply_pref`] runs.
pub(crate) async fn after_install(state: &AppState) -> Registered {
    let _running = state.memory_setup.lock().await;

    apply_pref(state).await
}

/// Registers the server when the pref is on (the record is kept, so an entry the user removed stays
/// removed), then retires the old copy. The server is looked up (and its version asked) once for
/// both. The caller holds `state.memory_setup`.
async fn apply_pref(state: &AppState) -> Registered {
    let server = server(state).await;

    let registered = if memory_enabled(state).await {
        register_server(state, false, server.clone()).await
    } else {
        Some(Vec::new())
    };

    retire_legacy_copy(state, server).await;

    registered
}

/// The installed server, when it reports its version.
async fn server(state: &AppState) -> Option<PathBuf> {
    installed_memory(&state.env, &state.runner)
        .await
        .map(|binary| binary.path)
}

/// The stored `memoryEnabled` pref (off when it cannot be read).
async fn memory_enabled(state: &AppState) -> bool {
    let prefs = state.prefs.clone();

    tauri::async_runtime::spawn_blocking(move || prefs.with(|store| store.load().memory_enabled))
        .await
        .unwrap_or(false)
}

/// Registers the installed server in every detected harness's user MCP file, recording where it is.
/// `fresh` forgets the record first (also when the server is not installed), so it is added back
/// everywhere. The caller holds `state.memory_setup`.
pub(crate) async fn register(state: &AppState, fresh: bool) -> Registered {
    let server = server(state).await;

    register_server(state, fresh, server).await
}

/// [`register`] with the server already looked up; `None` when it is not installed.
async fn register_server(state: &AppState, fresh: bool, server: Option<PathBuf>) -> Registered {
    if fresh {
        clear_record(state).await;
    }

    let targets = targets(state).await;
    let env = state.env.clone();
    let config_dir = state.config_dir.clone();

    let registered = tauri::async_runtime::spawn_blocking(move || {
        let Some(server) = server else {
            tracing::info!("project memory: harness-memory is not installed; nothing registered");
            return None;
        };

        let mut record = MemoryRecord::load(&config_dir);
        let mut outcomes = Vec::new();

        for (harness, file) in targets {
            let outcome = register_memory_mcp(&file, &server, record.is_recorded(&env, &file.path));
            tracing::info!(?harness, ?outcome, "project memory registration");

            record.note(&env, harness, &file.path, outcome);
            outcomes.push((harness, outcome));
        }

        if let Err(error) = record.save() {
            tracing::warn!(%error, "project memory record not saved");
        }

        Some(outcomes)
    })
    .await;

    registered.unwrap_or_else(|error| {
        tracing::warn!(%error, "project memory registration task failed");
        None
    })
}

/// Registers the installed server in the user MCP file of `harness` because the user asked for it
/// there: added even where the user removed it before, and recorded, so later launches keep its
/// `command` current. Project memory is turned on first when it is off (the other harnesses are
/// left as they are until the next launch). `invalidInput` when the server is not installed (or
/// does not report its version); `unsupported` when the harness has no user MCP file. Returns the
/// file's state afterwards (`refused` when it could not be changed safely, `manualEdit` with the
/// entry to paste when it has comments).
pub(crate) async fn register_in(state: &AppState, harness: HarnessId) -> CommandResult<McpState> {
    let _running = state.memory_setup.lock().await;

    let Some(server) = server(state).await else {
        return Err(BusinessError::InvalidInput("Harness Memory is not installed".to_owned()).into());
    };

    let Some(file) = state
        .registry
        .get(harness)
        .and_then(|found| mcp_target(&state.env, found))
    else {
        return Err(BusinessError::Unsupported(format!("{harness:?} has no user MCP file")).into());
    };

    if !memory_enabled(state).await {
        switch_on(state).await?;
    }

    let env = state.env.clone();
    let config_dir = state.config_dir.clone();

    let registered = tauri::async_runtime::spawn_blocking(move || {
        let mut record = MemoryRecord::load(&config_dir);
        let outcome = register_memory_mcp(&file, &server, false);
        tracing::info!(?harness, ?outcome, "project memory registration requested");

        record.note(&env, harness, &file.path, outcome);
        if let Err(error) = record.save() {
            tracing::warn!(%error, "project memory record not saved");
        }

        // A manual edit is read back too: the state carries the entry to paste.
        match outcome {
            MemoryRegistration::Refused(reason) => McpState::Refused {
                file: tilde(&env, &file.path),
                reason,
            },
            _ => memory_mcp_state(&env, &file),
        }
    })
    .await;

    registered.map_err(|error| CommandError::internal(format!("project memory registration task failed: {error}")))
}

/// Stores the `memoryEnabled` pref as on.
async fn switch_on(state: &AppState) -> CommandResult<()> {
    let prefs = state.prefs.clone();

    let saved = tauri::async_runtime::spawn_blocking(move || {
        prefs.with(|store| store.update(|prefs| prefs.memory_enabled = true))
    })
    .await
    .map_err(|error| CommandError::internal(format!("storing the project memory pref failed: {error}")))?;

    saved?;
    tracing::info!("project memory turned on to register the server in a harness");

    Ok(())
}

/// Removes the app's own entry from every detected harness's user MCP file; the record is kept.
/// Returns the entries left in files with comments (a rewrite would lose them), for the user to
/// remove by hand. The caller holds `state.memory_setup`.
pub(crate) async fn unregister_all(state: &AppState) -> Vec<ManualRemoval> {
    let targets = targets(state).await;
    let env = state.env.clone();

    let removed = tauri::async_runtime::spawn_blocking(move || {
        let mut manual = Vec::new();

        for (harness, file) in targets {
            let outcome = unregister_memory_mcp(&file);

            if outcome == Err(Blocked::ManualEdit) {
                tracing::warn!(
                    ?harness,
                    "project memory: entry left in a file with comments, for the user to remove"
                );

                manual.push(ManualRemoval {
                    harness,
                    file: tilde(&env, &file.path),
                    name: MEMORY_MCP_SERVER.to_owned(),
                });
            } else {
                tracing::info!(?harness, ?outcome, "project memory unregistration");
            }
        }

        manual
    })
    .await;

    removed.unwrap_or_else(|error| {
        tracing::warn!(%error, "project memory unregistration task failed");
        Vec::new()
    })
}

/// Points the owned entries that run the old copy at the installed `server` and deletes the copy
/// once nothing runs it. Checks every harness's user `mcp.json` files, detected or not (the builds
/// that kept the copy registered it only there). The caller holds `state.memory_setup`.
async fn retire_legacy_copy(state: &AppState, server: Option<PathBuf>) {
    let env = state.env.clone();
    let targets: Vec<(PathBuf, Vec<PathBuf>)> = state
        .registry
        .all()
        .filter_map(|harness| mcp_target_paths(&env, harness))
        .collect();

    let migrated =
        tauri::async_runtime::spawn_blocking(move || migrate_legacy_memory(&env, &targets, server.as_deref())).await;

    match migrated {
        Ok(migration) => {
            tracing::info!(
                repointed = migration.repointed,
                copy = ?migration.copy,
                "project memory: old server copy checked"
            );
        }
        Err(error) => tracing::warn!(%error, "project memory migration task failed"),
    }
}

/// Forgets which files the server was registered in.
async fn clear_record(state: &AppState) {
    let config_dir = state.config_dir.clone();
    let cleared = tauri::async_runtime::spawn_blocking(move || MemoryRecord::clear(&config_dir)).await;

    match cleared {
        Ok(Ok(())) => {}
        Ok(Err(error)) => tracing::warn!(%error, "project memory record not cleared"),
        Err(error) => tracing::warn!(%error, "project memory record task failed"),
    }
}

/// The MCP files of the installed harnesses that have one.
async fn targets(state: &AppState) -> Vec<Target> {
    let summaries = match summaries(state).await {
        Ok(summaries) => summaries,
        Err(_) => {
            tracing::warn!("project memory: harness detection failed");
            return Vec::new();
        }
    };

    summaries
        .iter()
        .filter(|summary| summary.detection.installed)
        .filter_map(|summary| {
            let harness = state.registry.get(summary.info.id)?;
            let file = mcp_target(&state.env, harness)?;

            Some((summary.info.id, file))
        })
        .collect()
}
