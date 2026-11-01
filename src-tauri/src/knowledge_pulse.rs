//! What keeps the Knowledge view live: the page that called `knowledge_changes_listen` gets the
//! database's counts (documents, generation, proposals waiting) now and whenever one changes, so it
//! reloads its list when the generation moves and keeps the review badge current, with no Refresh
//! button. The counts are read through `harness-knowledge stats --json` and `held list`
//! every [`POLL_INTERVAL`] while project knowledge is on, never from the database.

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use harness_business::ecosystem::knowledge_pulse;
use harness_business::model::KnowledgePulse;
use tauri::async_runtime::{self, JoinHandle};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// How often the counts are read.
pub const POLL_INTERVAL: Duration = Duration::from_secs(4);

/// The task that sends the counts to the page that listens.
#[derive(Default)]
pub struct PulseWatch(Mutex<Option<JoinHandle<()>>>);

impl PulseWatch {
    /// Sends the counts now and on every change to `channel`, replacing the previous listener.
    pub fn listen(&self, app: AppHandle, channel: Channel<KnowledgePulse>) {
        let task = async_runtime::spawn(async move {
            let mut last: Option<KnowledgePulse> = None;

            loop {
                let state = app.state::<AppState>();

                if state.agents.is_shutting_down() {
                    break;
                }

                if let Some(pulse) = read(&state).await
                    && last.as_ref() != Some(&pulse)
                {
                    // A closed channel means its page is gone.
                    if channel.send(pulse.clone()).is_err() {
                        break;
                    }

                    last = Some(pulse);
                }

                tokio::time::sleep(POLL_INTERVAL).await;
            }
        });

        let previous = self.0.lock().unwrap_or_else(PoisonError::into_inner).replace(task);

        if let Some(previous) = previous {
            previous.abort();
        }

        tracing::info!("a page listens for the knowledge counts");
    }
}

/// The counts, while project knowledge is on and the server answers.
async fn read(state: &AppState) -> Option<KnowledgePulse> {
    if !crate::sidecar_setup::enabled::<crate::product::Knowledge>(state).await {
        return None;
    }

    knowledge_pulse(&state.env, &state.runner, &state.knowledge_features).await
}
