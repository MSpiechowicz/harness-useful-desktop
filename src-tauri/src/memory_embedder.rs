//! The embedder behind vector search of Harness Memory: it runs while project memory and the
//! `vectorSearch` pref are on and the installed Harness Memory lists `embedder` (see
//! [`crate::sidecar_embedder`], run for [`Memory`]), from launch, after the server's install, and
//! after each switch of either pref. The page that called `memory_embedder_watch` gets the state
//! on every change.

use std::sync::{Mutex, PoisonError};

use harness_business::model::MemoryEmbedderState;
use tauri::AppHandle;
use tauri::async_runtime::{self, JoinHandle};
use tauri::ipc::Channel;
use tokio::sync::watch;

use crate::product::Memory;
use crate::sidecar_embedder;
use crate::state::AppState;

/// The task that forwards the embedder's state to the page that listens.
#[derive(Default)]
pub struct EmbedderWatch(Mutex<Option<JoinHandle<()>>>);

impl EmbedderWatch {
    /// Sends the state `updates` holds now and on every change to `channel`, replacing the previous
    /// listener.
    pub fn listen(&self, mut updates: watch::Receiver<MemoryEmbedderState>, channel: Channel<MemoryEmbedderState>) {
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
    sidecar_embedder::spawn_on_launch::<Memory>(app);
}

/// Makes the embedder run or stop as the prefs and the installed server say (see
/// [`sidecar_embedder::reconcile`]).
pub(crate) async fn reconcile(state: &AppState) {
    sidecar_embedder::reconcile::<Memory>(state).await;
}

/// Stops the embedder for good (app exit), within [`sidecar_embedder::EXIT_STOP_LIMIT`].
pub fn stop_for_exit(state: &AppState) {
    sidecar_embedder::stop_for_exit::<Memory>(state);
}
