//! Project memory: the bundled `harness-memory` MCP server, installed as a stable copy and
//! registered in every detected harness's user MCP file at launch (while the `memoryEnabled` pref
//! is on), and unregistered when the user turns it off (`memory_set_enabled`).
//!
//! Debug builds do the same as release builds, so `bun run dev` is a full testing ground (it installs
//! the debug server as the stable copy). Logs name the harness and the outcome, never a file's
//! contents.

use std::path::{Path, PathBuf};

use harness_business::app_dirs::memory_bin_dir;
use harness_business::ecosystem::{
    MemoryRecord, MemoryRegistration, install_memory_binary, mcp_target_paths, memory_binary_name, register_memory_mcp,
    unregister_memory_mcp,
};
use harness_business::model::HarnessId;
use tauri::{AppHandle, Manager};

use crate::commands::harness::summaries;
use crate::state::AppState;

/// The user MCP file of a harness and the files it also reads.
type Target = (HarnessId, PathBuf, Vec<PathBuf>);

/// Installs and registers the server in the background when the pref is on; the window does not
/// wait for it.
pub fn spawn_on_launch(app: &AppHandle) {
    let app = app.clone();

    drop(tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();

        if let Some(source) = bundled_server() {
            register_on_launch(&state, &source).await;
        }
    }));
}

/// The launch run: installs and registers the server when the pref is on. The pref is read once the
/// setup lock is held, so a switch-off that got the lock first is not undone.
pub(crate) async fn register_on_launch(state: &AppState, source: &Path) -> Vec<(HarnessId, MemoryRegistration)> {
    let _running = state.memory_setup.lock().await;
    let prefs = state.prefs.clone();

    let enabled = tauri::async_runtime::spawn_blocking(move || prefs.with(|store| store.load().memory_enabled))
        .await
        .unwrap_or(false);

    if !enabled {
        return Vec::new();
    }

    install_and_register(state, source, false).await
}

/// The bundled server next to the app's executable, or `None` when the executable's folder is
/// unknown.
pub(crate) fn bundled_server() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;

    Some(exe.parent()?.join(memory_binary_name()))
}

/// Installs `source` as the stable copy and registers it in every detected harness's user MCP file,
/// recording where it is. `fresh` forgets the record first (also when the install then fails), so
/// it is added back everywhere. The caller holds `state.memory_setup`.
pub(crate) async fn install_and_register(
    state: &AppState,
    source: &Path,
    fresh: bool,
) -> Vec<(HarnessId, MemoryRegistration)> {
    if fresh {
        clear_record(state).await;
    }

    let Some(dir) = memory_bin_dir(&state.env) else {
        tracing::warn!("project memory: no folder for the server on this machine");
        return Vec::new();
    };

    let source = source.to_path_buf();
    let installed = tauri::async_runtime::spawn_blocking(move || install_memory_binary(&source, &dir)).await;

    let binary = match installed {
        Ok(Ok(installed)) => {
            tracing::info!(changed = installed.changed, "project memory server installed");
            installed.path
        }
        Ok(Err(error)) => {
            tracing::warn!(%error, "project memory server not installed");
            return Vec::new();
        }
        Err(error) => {
            tracing::warn!(%error, "project memory install task failed");
            return Vec::new();
        }
    };

    let targets = targets(state).await;
    let env = state.env.clone();
    let config_dir = state.config_dir.clone();

    let registered = tauri::async_runtime::spawn_blocking(move || {
        let mut record = MemoryRecord::load(&config_dir);
        let mut outcomes = Vec::new();

        for (harness, path, also_read) in targets {
            let outcome = register_memory_mcp(&path, &also_read, &binary, record.is_recorded(&env, &path));
            tracing::info!(?harness, ?outcome, "project memory registration");

            record.note(&env, harness, &path, outcome);
            outcomes.push((harness, outcome));
        }

        if let Err(error) = record.save() {
            tracing::warn!(%error, "project memory record not saved");
        }

        outcomes
    })
    .await;

    registered.unwrap_or_default()
}

/// Removes the app's own entry from every detected harness's user MCP file; the record is kept. The
/// caller holds `state.memory_setup`.
pub(crate) async fn unregister_all(state: &AppState) {
    let targets = targets(state).await;

    let removed = tauri::async_runtime::spawn_blocking(move || {
        for (harness, path, _) in targets {
            let outcome = unregister_memory_mcp(&path);
            tracing::info!(?harness, ?outcome, "project memory unregistration");
        }
    })
    .await;

    if let Err(error) = removed {
        tracing::warn!(%error, "project memory unregistration task failed");
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

/// The user MCP files of the installed harnesses that have one.
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
            let (path, also_read) = mcp_target_paths(&state.env, harness)?;

            Some((summary.info.id, path, also_read))
        })
        .collect()
}
