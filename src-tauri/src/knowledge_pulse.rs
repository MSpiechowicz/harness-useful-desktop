//! What keeps the Knowledge view live: the page that called `knowledge_changes_listen` gets the
//! database's counts (documents, generation, proposals waiting) now and whenever one changes, so it
//! reloads its list when the generation moves and keeps the review badge current, with no Refresh
//! button. The counts are read through one `harness-knowledge stats --json` call, never from the
//! database: every [`WATCHING_INTERVAL`] while the Knowledge view is open, every [`IDLE_INTERVAL`]
//! otherwise (the rail badge only). While project knowledge is off the counts are zero, so the badge
//! clears.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use harness_business::ecosystem::{KnowledgeSupport, knowledge_pulse, knowledge_remote_get};
use harness_business::model::{KnowledgePulse, KnowledgeRemote, MemoryNotesRouting};
use tauri::async_runtime::{self, JoinHandle};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tokio::sync::Notify;

use crate::state::AppState;

/// How often the counts are read while the Knowledge view is open.
pub const WATCHING_INTERVAL: Duration = Duration::from_secs(4);

/// How often they are read otherwise.
pub const IDLE_INTERVAL: Duration = Duration::from_secs(30);

/// The task that sends the counts to the page that listens.
#[derive(Default)]
pub struct PulseWatch {
    task: Mutex<Option<JoinHandle<()>>>,
    /// The Knowledge view is open.
    watching: Arc<AtomicBool>,
    /// Ends the wait for the next read early.
    wake: Arc<Notify>,
}

impl PulseWatch {
    /// The interval to read at, for whether the Knowledge view is open.
    pub fn interval(watching: bool) -> Duration {
        if watching { WATCHING_INTERVAL } else { IDLE_INTERVAL }
    }

    /// Tells that the Knowledge view opened or closed: the counts are read at once and then at the
    /// interval that fits.
    pub fn set_watching(&self, watching: bool) {
        self.watching.store(watching, Ordering::Relaxed);
        self.wake.notify_one();
    }

    /// Sends the counts now and on every change to `channel`, replacing the previous listener.
    pub fn listen(&self, app: AppHandle, channel: Channel<KnowledgePulse>) {
        let watching = Arc::clone(&self.watching);
        let wake = Arc::clone(&self.wake);
        let task = async_runtime::spawn(async move {
            let mut last: Option<KnowledgePulse> = None;

            loop {
                let state = app.state::<AppState>();

                if state.agents.is_shutting_down() {
                    break;
                }

                let pulse = match read(&state).await {
                    Some(pulse) => Some(pulse),
                    None => unreadable(
                        last.as_ref(),
                        &knowledge_remote_get(&state.env, KnowledgeSupport::Unknown),
                    ),
                };

                if let Some(pulse) = pulse
                    && last.as_ref() != Some(&pulse)
                {
                    // A closed channel means its page is gone.
                    if channel.send(pulse.clone()).is_err() {
                        break;
                    }

                    last = Some(pulse);
                }

                let interval = Self::interval(watching.load(Ordering::Relaxed));

                tokio::select! {
                    () = tokio::time::sleep(interval) => {}
                    () = wake.notified() => {}
                }
            }
        });

        let previous = self.task.lock().unwrap_or_else(PoisonError::into_inner).replace(task);

        if let Some(previous) = previous {
            previous.abort();
        }

        tracing::info!("a page listens for the knowledge counts");
    }
}

/// What to send when the counts cannot be read: the last ones without `offline` when the remote
/// database is the only place in use. Then there is nothing to fall back on, so the server fails
/// while the remote is unreachable and the banner of the last pulse would stay for ever. Nothing
/// otherwise.
fn unreadable(last: Option<&KnowledgePulse>, remote: &KnowledgeRemote) -> Option<KnowledgePulse> {
    let last = last.filter(|last| last.offline)?;

    let both = remote.enabled
        && (remote.repository_documents == MemoryNotesRouting::RemoteLocal
            || remote.global_documents == MemoryNotesRouting::RemoteLocal
            || remote.repository_documents != remote.global_documents);

    (!both).then(|| KnowledgePulse {
        offline: false,
        ..last.clone()
    })
}

/// The counts while the server answers, and none (zero, so no badge) while project knowledge is off.
async fn read(state: &AppState) -> Option<KnowledgePulse> {
    if !crate::sidecar_setup::enabled::<crate::product::Knowledge>(state).await {
        return Some(KnowledgePulse {
            documents: 0,
            generation: 0,
            held: Some(0),
            offline: false,
        });
    }

    knowledge_pulse(&state.env, &state.runner, &state.knowledge_features).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(enabled: bool, repository: MemoryNotesRouting, global: MemoryNotesRouting) -> KnowledgeRemote {
        KnowledgeRemote {
            enabled,
            kind: None,
            url_redacted: None,
            has_token: false,
            embedder_supported: false,
            repository_documents: repository,
            global_documents: global,
            repository_documents_env: false,
            global_documents_env: false,
            routing_supported: true,
            config_path: String::new(),
            problem: None,
        }
    }

    #[test]
    fn an_offline_flag_is_cleared_when_the_counts_fail_and_only_the_remote_is_in_use() {
        let offline = KnowledgePulse {
            documents: 2,
            generation: 5,
            held: Some(1),
            offline: true,
        };
        let cleared = KnowledgePulse {
            offline: false,
            ..offline.clone()
        };
        let remote_only = remote(true, MemoryNotesRouting::Remote, MemoryNotesRouting::Remote);
        let split = remote(true, MemoryNotesRouting::Remote, MemoryNotesRouting::Local);

        assert_eq!(unreadable(Some(&offline), &remote_only), Some(cleared));
        assert_eq!(unreadable(Some(&offline), &split), None, "this computer still answers");
        assert_eq!(
            unreadable(Some(&cleared_of(&offline)), &remote_only),
            None,
            "nothing to clear"
        );
        assert_eq!(unreadable(None, &remote_only), None);
    }

    fn cleared_of(pulse: &KnowledgePulse) -> KnowledgePulse {
        KnowledgePulse {
            offline: false,
            ..pulse.clone()
        }
    }

    #[test]
    fn the_counts_are_read_often_only_while_the_view_is_open() {
        assert_eq!(PulseWatch::interval(true), Duration::from_secs(4));
        assert_eq!(PulseWatch::interval(false), Duration::from_secs(30));
    }
}
