//! The embedder behind vector search (see `harness_business::ecosystem::memory_embedder_supervisor`):
//! it runs while project memory and the `vectorSearch` pref are on and the installed Harness Memory
//! lists `embedder`, from launch (also after an update that finds the pref on and the embedder's own
//! switch off), after the server's install, and after each switch of either pref. It stops when
//! either is turned off and at app exit; hiding the window to the tray leaves it running. The page
//! that called `memory_embedder_watch` gets the state on every change.

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use harness_business::model::MemoryEmbedderState;
use tauri::async_runtime::{self, JoinHandle};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// The longest the embedder gets to stop when the app exits, after its stdin closed and the kill.
pub const EXIT_STOP_LIMIT: Duration = Duration::from_secs(5);

/// The task that forwards the embedder's state to the page that listens.
#[derive(Default)]
pub struct EmbedderWatch(Mutex<Option<JoinHandle<()>>>);

impl EmbedderWatch {
    /// Sends the state now and on every change to `channel`, replacing the previous listener.
    pub fn listen(&self, state: &AppState, channel: Channel<MemoryEmbedderState>) {
        let mut updates = state.memory_embedder.subscribe();

        let task = async_runtime::spawn(async move {
            loop {
                let current = updates.borrow_and_update().clone();

                // A closed channel means its page is gone.
                if channel.send(current).is_err() || updates.changed().await.is_err() {
                    break;
                }
            }
        });

        let previous = self.0.lock().unwrap_or_else(PoisonError::into_inner).replace(task);

        if let Some(previous) = previous {
            previous.abort();
        }

        tracing::info!("a page listens for the embedder");
    }
}

/// Starts the embedder in the background when it is wanted; the window does not wait for it.
pub fn spawn_on_launch(app: &AppHandle) {
    let app = app.clone();

    drop(async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        reconcile(&state).await;
    }));
}

/// Makes the embedder run or stop as the prefs and the installed server say (see the module docs).
/// Does nothing once shutdown began.
pub(crate) async fn reconcile(state: &AppState) {
    if state.agents.is_shutting_down() {
        return;
    }

    let wanted = wanted(state).await;

    state
        .memory_embedder
        .reconcile(&state.env, &state.runner, &state.memory_features, wanted)
        .await;
}

/// True while project memory and vector search are both on.
async fn wanted(state: &AppState) -> bool {
    let prefs = state.prefs.clone();

    async_runtime::spawn_blocking(move || {
        prefs.with(|store| {
            let prefs = store.load();

            prefs.memory_enabled && prefs.vector_search
        })
    })
    .await
    .unwrap_or(false)
}

/// Stops the embedder for good (app exit), within [`EXIT_STOP_LIMIT`].
pub fn stop_for_exit(state: &AppState) {
    let stopping = tokio::time::timeout(EXIT_STOP_LIMIT, state.memory_embedder.stop());

    if async_runtime::block_on(stopping).is_err() {
        tracing::warn!("the embedder did not stop in time");
    }
}
