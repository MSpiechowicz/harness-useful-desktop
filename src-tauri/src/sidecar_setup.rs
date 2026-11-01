//! A Harness family server in the harnesses' user MCP files, for any [`Product`] (Harness Memory,
//! Harness Knowledge): the installed server (found with `installed_sidecar`, so only once it
//! reports its version) registered in every detected harness at launch and after its install
//! while the product's pref is on, and unregistered when the user turns it off. While the server
//! is not installed nothing is registered. An entry in a file with comments (OpenCode's JSONC) is
//! never rewritten: turning the product off reports it for the user to remove by hand. Claude
//! Code's file is changed through its CLI ([`crate::mcp_cli`]), with a time limit per run and no
//! retry. Logs name the harness and the outcome, never a file's contents.
//!
//! `memory_setup` and `knowledge_setup` call these with their product and add what is theirs.

use std::path::PathBuf;

use harness_business::BusinessError;
use harness_business::ecosystem::sidecar_binary::installed_sidecar;
use harness_business::ecosystem::sidecar_record::SidecarRecord;
use harness_business::ecosystem::sidecar_register::{
    SidecarRegistration, register_sidecar_mcp, unregister_sidecar_mcp,
};
use harness_business::ecosystem::{Blocked, mcp_target, register_sidecar_mcp_cli, unregister_sidecar_mcp_cli};
use harness_business::model::{AppPrefs, HarnessId, ManualRemoval, McpState};
use harness_business::pi_family::tilde;
use tauri::async_runtime::block_on;
use tauri::{AppHandle, Manager};

use crate::commands::harness::summaries;
use crate::error::{CommandError, CommandResult};
use crate::mcp_cli::{self, CliContext, HarnessFile};
use crate::product::Product;
use crate::state::AppState;

/// What a registration did per harness; `None` when the server is not installed (nothing
/// registered).
pub(crate) type Registered = Option<Vec<(HarnessId, SidecarRegistration)>>;

/// Registers the server in the background; the window does not wait for it.
pub(crate) fn spawn_on_launch<P: Product>(app: &AppHandle) {
    let app = app.clone();

    drop(tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        on_launch::<P>(&state).await;
    }));
}

/// The launch run (see [`apply_pref`]). The pref is read once the setup lock is held, so a
/// switch-off that got the lock first is not undone.
pub(crate) async fn on_launch<P: Product>(state: &AppState) -> Registered {
    let _running = P::setup_lock(state).lock().await;

    apply_pref::<P>(state).await
}

/// After the server's installer succeeded: [`apply_pref`] runs.
pub(crate) async fn after_install<P: Product>(state: &AppState) -> Registered {
    let _running = P::setup_lock(state).lock().await;

    apply_pref::<P>(state).await
}

/// Registers the server when the pref is on (the record is kept, so an entry the user removed stays
/// removed), then lets the product retire what is left of older builds. The server is looked up
/// (and its version asked) once for both. The caller holds the setup lock.
async fn apply_pref<P: Product>(state: &AppState) -> Registered {
    let server = server::<P>(state).await;

    let registered = if enabled::<P>(state).await {
        register_server::<P>(state, false, server.clone()).await
    } else {
        Some(Vec::new())
    };

    P::retire_legacy(state, server).await;

    registered
}

/// The installed server, when it reports its version.
pub(crate) async fn server<P: Product>(state: &AppState) -> Option<PathBuf> {
    installed_sidecar(P::SIDECAR, &state.env, &state.runner)
        .await
        .map(|binary| binary.path)
}

/// The stored pref that switches the product on (off when it cannot be read).
pub(crate) async fn enabled<P: Product>(state: &AppState) -> bool {
    let prefs = state.prefs.clone();

    tauri::async_runtime::spawn_blocking(move || prefs.with(|store| P::enabled(&store.load())))
        .await
        .unwrap_or(false)
}

/// Registers the installed server in every detected harness's user MCP file, recording where it is.
/// `fresh` forgets the record first (also when the server is not installed), so it is added back
/// everywhere. The caller holds the product's setup lock.
pub(crate) async fn register<P: Product>(state: &AppState, fresh: bool) -> Registered {
    let server = server::<P>(state).await;

    register_server::<P>(state, fresh, server).await
}

/// [`register`] with the server already looked up; `None` when it is not installed.
pub(crate) async fn register_server<P: Product>(state: &AppState, fresh: bool, server: Option<PathBuf>) -> Registered {
    if fresh {
        clear_record::<P>(state).await;
    }

    let targets = targets::<P>(state).await;
    let env = state.env.clone();
    let config_dir = state.config_dir.clone();
    let context = CliContext::of(state);

    let registered = tauri::async_runtime::spawn_blocking(move || {
        let Some(server) = server else {
            tracing::info!(
                "{}: {} is not installed; nothing registered",
                P::SIDECAR.feature,
                P::SIDECAR.binary
            );
            return None;
        };

        let mut record = SidecarRecord::<P::File>::load(&config_dir);
        let mut outcomes = Vec::new();

        for (harness, file, binary) in targets {
            let recorded = record.is_recorded(&env, &file.path);
            let outcome = match context.cli_for(harness, &file, binary.as_ref()) {
                Some(cli) => block_on(register_sidecar_mcp_cli(P::SIDECAR, cli, &file, &server, recorded)),
                None => register_sidecar_mcp(P::SIDECAR, &file, &server, recorded),
            };
            tracing::info!(?harness, ?outcome, "{} registration", P::SIDECAR.feature);

            record.note(&env, harness, &file.path, outcome);
            outcomes.push((harness, outcome));
        }

        if let Err(error) = record.save() {
            tracing::warn!(%error, "{} record not saved", P::SIDECAR.feature);
        }

        Some(outcomes)
    })
    .await;

    registered.unwrap_or_else(|error| {
        tracing::warn!(%error, "{} registration task failed", P::SIDECAR.feature);
        None
    })
}

/// Registers the installed server in the user MCP file of `harness` because the user asked for it
/// there: added even where the user removed it before, and recorded, so later launches keep its
/// `command` current. The product is turned on first when it is off (the other harnesses are left
/// as they are until the next launch). `invalidInput` when the server is not installed (or does
/// not report its version); `unsupported` when the harness has no user MCP file. Returns the
/// file's state afterwards (`refused` when it could not be changed safely, `manualEdit` with the
/// entry to paste when it has comments).
pub(crate) async fn register_in<P: Product>(state: &AppState, harness: HarnessId) -> CommandResult<McpState> {
    let _running = P::setup_lock(state).lock().await;

    let Some(server) = server::<P>(state).await else {
        return Err(BusinessError::InvalidInput(format!("{} is not installed", P::SIDECAR.product)).into());
    };

    let Some(file) = state
        .registry
        .get(harness)
        .and_then(|found| mcp_target(&state.env, found))
    else {
        return Err(BusinessError::Unsupported(format!("{harness:?} has no user MCP file")).into());
    };

    if !enabled::<P>(state).await {
        switch_on::<P>(state).await?;
    }

    let env = state.env.clone();
    let config_dir = state.config_dir.clone();
    let context = CliContext::of(state);
    let binary = mcp_cli::cached_binary(state, harness).await;

    let registered = tauri::async_runtime::spawn_blocking(move || {
        let mut record = SidecarRecord::<P::File>::load(&config_dir);
        let outcome = match context.cli_for(harness, &file, binary.as_ref()) {
            Some(cli) => block_on(register_sidecar_mcp_cli(P::SIDECAR, cli, &file, &server, false)),
            None => register_sidecar_mcp(P::SIDECAR, &file, &server, false),
        };
        tracing::info!(?harness, ?outcome, "{} registration requested", P::SIDECAR.feature);

        record.note(&env, harness, &file.path, outcome);
        if let Err(error) = record.save() {
            tracing::warn!(%error, "{} record not saved", P::SIDECAR.feature);
        }

        // A manual edit is read back too: the state carries the entry to paste.
        match outcome {
            SidecarRegistration::Refused(reason) => McpState::Refused {
                file: tilde(&env, &file.path),
                reason,
            },
            _ => P::mcp_state(&env, &file),
        }
    })
    .await;

    registered
        .map_err(|error| CommandError::internal(format!("{} registration task failed: {error}", P::SIDECAR.feature)))
}

/// Stores the product's pref as on.
async fn switch_on<P: Product>(state: &AppState) -> CommandResult<()> {
    let prefs = state.prefs.clone();

    let saved = tauri::async_runtime::spawn_blocking(move || {
        prefs.with(|store| store.update(|prefs| P::set_enabled(prefs, true)))
    })
    .await
    .map_err(|error| CommandError::internal(format!("storing the {} pref failed: {error}", P::SIDECAR.feature)))?;

    saved?;
    tracing::info!("{} turned on to register the server in a harness", P::SIDECAR.feature);

    Ok(())
}

/// Stores the product's pref as `enabled` and applies it to the harnesses' user MCP files: on
/// registers the installed server everywhere (also where the user removed it before, when it was
/// off), off removes the app's entries. Returns the prefs stored and, when switched off, the entries
/// left in files with comments (a rewrite would lose them), for the user to remove by hand. The
/// setup lock is held from storing the pref to applying it, so quick toggles and the launch run
/// apply in order.
pub(crate) async fn set_enabled<P: Product>(
    state: &AppState,
    enabled: bool,
) -> CommandResult<(AppPrefs, Vec<ManualRemoval>)> {
    let _running = P::setup_lock(state).lock().await;
    let prefs = state.prefs.clone();

    let (was_enabled, saved) = tauri::async_runtime::spawn_blocking(move || {
        prefs.with(|store| {
            let was_enabled = P::enabled(&store.load());

            store
                .update(|prefs| P::set_enabled(prefs, enabled))
                .map(|saved| (was_enabled, saved))
        })
    })
    .await
    .map_err(|error| CommandError::internal(format!("background task failed: {error}")))??;

    let manual_removals = if enabled {
        register::<P>(state, !was_enabled).await;
        Vec::new()
    } else {
        unregister_all::<P>(state).await
    };

    Ok((saved, manual_removals))
}

/// Removes the app's own entry from every detected harness's user MCP file; the record is kept.
/// Returns the entries left in files with comments (a rewrite would lose them), for the user to
/// remove by hand. The caller holds the product's setup lock.
pub(crate) async fn unregister_all<P: Product>(state: &AppState) -> Vec<ManualRemoval> {
    let targets = targets::<P>(state).await;
    let env = state.env.clone();
    let context = CliContext::of(state);

    let removed = tauri::async_runtime::spawn_blocking(move || {
        let mut manual = Vec::new();

        for (harness, file, binary) in targets {
            let outcome = match context.cli_for(harness, &file, binary.as_ref()) {
                Some(cli) => block_on(unregister_sidecar_mcp_cli(P::SIDECAR, cli, &file)),
                None => unregister_sidecar_mcp(P::SIDECAR, &file),
            };

            if outcome == Err(Blocked::ManualEdit) {
                tracing::warn!(
                    ?harness,
                    "{}: entry left in a file with comments, for the user to remove",
                    P::SIDECAR.feature
                );

                manual.push(ManualRemoval {
                    harness,
                    file: tilde(&env, &file.path),
                    name: P::SIDECAR.mcp_server.to_owned(),
                });
            } else {
                tracing::info!(?harness, ?outcome, "{} unregistration", P::SIDECAR.feature);
            }
        }

        manual
    })
    .await;

    removed.unwrap_or_else(|error| {
        tracing::warn!(%error, "{} unregistration task failed", P::SIDECAR.feature);
        Vec::new()
    })
}

/// Forgets which files the server was registered in.
async fn clear_record<P: Product>(state: &AppState) {
    let config_dir = state.config_dir.clone();
    let cleared = tauri::async_runtime::spawn_blocking(move || SidecarRecord::<P::File>::clear(&config_dir)).await;

    match cleared {
        Ok(Ok(())) => {}
        Ok(Err(error)) => tracing::warn!(%error, "{} record not cleared", P::SIDECAR.feature),
        Err(error) => tracing::warn!(%error, "{} record task failed", P::SIDECAR.feature),
    }
}

/// The MCP files of the installed harnesses that have one, with their binaries.
async fn targets<P: Product>(state: &AppState) -> Vec<HarnessFile> {
    let summaries = match summaries(state).await {
        Ok(summaries) => summaries,
        Err(_) => {
            tracing::warn!("{}: harness detection failed", P::SIDECAR.feature);
            return Vec::new();
        }
    };

    summaries
        .iter()
        .filter(|summary| summary.detection.installed)
        .filter_map(|summary| {
            let harness = state.registry.get(summary.info.id)?;
            let file = mcp_target(&state.env, harness)?;

            Some((summary.info.id, file, mcp_cli::detected_binary(summary)))
        })
        .collect()
}
