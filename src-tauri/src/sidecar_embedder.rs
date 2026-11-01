//! The embedder behind vector search of an [`EmbedderProduct`] (see
//! `harness_business::ecosystem::sidecar_embedder_supervisor`): it runs while the product and its
//! vector search pref are on and the installed server lists `embedder`, from launch (also after an
//! update that finds the pref on and the embedder's own switch off), after the server's install,
//! and after each switch of either pref. It stops when either is turned off and at app exit; hiding
//! the window to the tray leaves it running. The page that called the watch command gets the state
//! on every change.
//!
//! `memory_embedder` and `knowledge_embedder` hold the launch and exit hooks of each product, and
//! `commands::memory_embedder` and `commands::knowledge_embedder` its commands.

use std::path::PathBuf;
use std::time::Duration;

use harness_business::ecosystem::sidecar_embedder::set_sidecar_embedder_enabled;
use harness_business::ecosystem::{EmbedderFeatures, Reconciled};
use harness_business::model::{AppPrefs, MemoryEmbedderState, MemoryVectorSwitchResult};
use harness_business::{BusinessError, Result};
use tauri::async_runtime;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::commands::blocking;
use crate::error::CommandResult;
use crate::product::EmbedderProduct;
use crate::state::AppState;

/// The longest the embedder gets to stop when the app exits, after its stdin closed and the kill.
pub const EXIT_STOP_LIMIT: Duration = Duration::from_secs(5);

/// Starts the embedder in the background when it is wanted; the window does not wait for it.
pub(crate) fn spawn_on_launch<P: EmbedderProduct>(app: &AppHandle) {
    let app = app.clone();

    drop(async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        reconcile::<P>(&state).await;
    }));
}

/// Makes the embedder run or stop as the prefs and the installed server say (see the module docs).
/// Does nothing once shutdown began, also when it began while the server was being asked. An
/// embedder switched off on the command line stays off and turns the vector search pref off, so
/// the switch in the settings shows what runs.
pub(crate) async fn reconcile<P: EmbedderProduct>(state: &AppState) {
    let closing = || state.agents.is_shutting_down();

    if closing() {
        return;
    }

    let wanted = wanted::<P>(state).await;

    let settled = P::embedder(state)
        .reconcile(&state.env, &state.runner, P::features(state), wanted, &closing)
        .await;

    if settled == Reconciled::OptedOut {
        turn_vector_search_off::<P>(state).await;
    }
}

async fn turn_vector_search_off<P: EmbedderProduct>(state: &AppState) {
    let prefs = state.prefs.clone();

    let saved = async_runtime::spawn_blocking(move || {
        prefs.with(|store| store.update(|prefs| P::set_vector_search(prefs, false)))
    })
    .await;

    match saved {
        Ok(Ok(_)) => tracing::info!(
            "{}: vector search is off, the embedder was switched off outside the app",
            P::SIDECAR.feature
        ),
        Ok(Err(error)) => tracing::warn!(%error, "the vector search pref could not be saved"),
        Err(error) => tracing::warn!(%error, "the vector search pref could not be saved"),
    }
}

/// True while the product and its vector search are both on.
pub(crate) async fn wanted<P: EmbedderProduct>(state: &AppState) -> bool {
    let prefs = state.prefs.clone();

    async_runtime::spawn_blocking(move || {
        prefs.with(|store| {
            let prefs = store.load();

            P::enabled(&prefs) && P::vector_search(&prefs)
        })
    })
    .await
    .unwrap_or(false)
}

/// Stops the embedder for good (app exit), within [`EXIT_STOP_LIMIT`].
pub(crate) fn stop_for_exit<P: EmbedderProduct>(state: &AppState) {
    // `timeout` registers its timer when created, so it must be built inside the runtime
    // (on the exit event's thread there is none and creating it there panics).
    let stopping = async { tokio::time::timeout(EXIT_STOP_LIMIT, P::embedder(state).stop()).await };

    if async_runtime::block_on(stopping).is_err() {
        tracing::warn!("the {} embedder did not stop in time", P::SIDECAR.feature);
    }
}

/// The installed server when its features list the embedder: `notFound` when it is not installed,
/// `unsupported` when it predates it.
pub(crate) async fn embedder_server<P: EmbedderProduct>(state: &AppState) -> Result<PathBuf> {
    P::features(state).embedder_server(&state.env, &state.runner).await
}

/// Turns vector search on or off: runs `embedder enable|disable`, stores the pref, then starts or
/// stops the embedder (it starts only while the product is on too). The answer holds the stored
/// pref and the embedder's state right then. Nothing changes when the server refuses:
/// `unsupported` for a server without `embedder`, `notFound` when it is not installed, `process`
/// with its line. A pref that cannot be stored undoes the server's switch and answers the error.
///
/// Turning it off also works on a server without `embedder` (`unsupported` for turning it on): the
/// pref is stored, there is nothing to switch, and the choice holds once the server is updated.
pub(crate) async fn set_vector_search<P: EmbedderProduct>(
    state: &AppState,
    enabled: bool,
) -> CommandResult<MemoryVectorSwitchResult> {
    let was = stored_vector_search::<P>(state).await;

    let switched = match embedder_server::<P>(state).await {
        Ok(server) => {
            set_sidecar_embedder_enabled(P::SIDECAR, &state.env, &state.runner, &server, enabled).await?;
            Some(server)
        }
        // Opting out needs no server: there is nothing to switch off.
        Err(BusinessError::Unsupported(_)) if !enabled => None,
        Err(error) => return Err(error.into()),
    };

    let saved = match store_vector_search::<P>(state, enabled).await {
        Ok(saved) => saved,
        Err(error) => {
            if let Some(server) = switched
                && let Err(undo) =
                    set_sidecar_embedder_enabled(P::SIDECAR, &state.env, &state.runner, &server, was).await
            {
                tracing::warn!(%undo, "the embedder's switch could not be put back");
            }

            return Err(error);
        }
    };

    reconcile::<P>(state).await;

    Ok(MemoryVectorSwitchResult {
        vector_search: P::vector_search(&saved),
        state: P::embedder(state).state(),
    })
}

async fn stored_vector_search<P: EmbedderProduct>(state: &AppState) -> bool {
    let prefs = state.prefs.clone();

    blocking(move || Ok(prefs.with(|store| P::vector_search(&store.load()))))
        .await
        .unwrap_or(true)
}

async fn store_vector_search<P: EmbedderProduct>(state: &AppState, enabled: bool) -> CommandResult<AppPrefs> {
    let prefs = state.prefs.clone();

    blocking(move || Ok(prefs.with(|store| store.update(|prefs| P::set_vector_search(prefs, enabled)))?)).await
}

/// Sends the embedder's state to `on_event` now and on every change, replacing the previous
/// listener.
pub(crate) fn watch<P: EmbedderProduct>(state: &AppState, on_event: Channel<MemoryEmbedderState>) {
    P::watch(state).listen(P::embedder(state).subscribe(), on_event);
}

/// Stops the embedder if it runs, then deletes the model files and the vectors
/// (`embedder remove-model`, 60 s). Answers the embedder's state (`off`, `modelPresent` false).
/// The vector search pref is left as it is, so turn vector search off first to keep the model from
/// being fetched again at the next launch. Errors as [`set_vector_search`].
pub(crate) async fn remove_model<P: EmbedderProduct>(state: &AppState) -> CommandResult<MemoryEmbedderState> {
    let server = embedder_server::<P>(state).await?;

    P::embedder(state)
        .remove_model(&state.env, &state.runner, &server)
        .await?;

    Ok(P::embedder(state).state())
}
