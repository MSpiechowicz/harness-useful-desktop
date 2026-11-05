//! What keeps the Knowledge view live: the page that called `knowledge_changes_listen` gets the
//! database's counts (documents, generation, proposals waiting) now and whenever one changes, so it
//! reloads its list when the generation moves and keeps the review badge current, with no Refresh
//! button. The counts are read through one `harness-knowledge stats --json` call, never from the
//! database: every [`WATCHING_INTERVAL`] while the Knowledge view is open, every [`IDLE_INTERVAL`]
//! otherwise (the rail badge only). While project knowledge is off the counts are zero, so the badge
//! clears. While the remote database is the only place in use and cannot be reached, the counts are
//! an `offline` pulse without numbers (see `knowledge_pulse`), so the last ones are not shown as
//! current. A database that is not upgraded yet is upgraded first (`harness-knowledge upgrade`), and
//! the pulse carries the one-time notice that running harnesses must be restarted until the page
//! dismissed it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use harness_business::ecosystem::{KnowledgeUpgradeNotice, knowledge_pulse_upgrading, upgrade_installed};
use harness_business::model::KnowledgePulse;
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
    /// What the database's upgrade did, for the pulse.
    notice: Arc<KnowledgeUpgradeNotice>,
}

impl PulseWatch {
    /// The interval to read at, for whether the Knowledge view is open.
    pub fn interval(watching: bool) -> Duration {
        if watching { WATCHING_INTERVAL } else { IDLE_INTERVAL }
    }

    /// What the database's upgrade did: its restart notice and whether it waits for a teammate.
    pub fn notice(&self) -> &Arc<KnowledgeUpgradeNotice> {
        &self.notice
    }

    /// The page showed the restart notice and the user dismissed it: the next pulse leaves it out at once.
    pub fn dismiss_restart_notice(&self) {
        self.notice.dismiss();
        self.wake.notify_one();
    }

    /// Upgrades the database after Harness Knowledge was installed or updated, then reads the
    /// counts at once (a server without `docs-drafts` has nothing to upgrade). The first server with
    /// drafts this computer gets also raises the one-time restart notice, upgraded here or not, and
    /// the app remembers it did.
    pub async fn upgrade_after_install(&self, state: &AppState) {
        let first_drafts = !state.prefs.with(|store| store.load()).knowledge_restart_notice_done;

        let has_drafts = upgrade_installed(
            &state.env,
            &state.runner,
            &state.knowledge_features,
            &self.notice,
            first_drafts,
        )
        .await;

        if has_drafts && first_drafts {
            remember_restart_notice(state).await;
        }

        self.wake.notify_one();
    }

    /// Reads the counts at once: a connection test just said whether the remote can be reached.
    pub fn wake(&self) {
        self.wake.notify_one();
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
        let notice = Arc::clone(&self.notice);
        let task = async_runtime::spawn(async move {
            let mut last: Option<KnowledgePulse> = None;

            loop {
                let state = app.state::<AppState>();

                if state.agents.is_shutting_down() {
                    break;
                }

                // A failure that is not the remote being unreachable sends nothing: the last counts stay.
                if let Some(pulse) = read(&state, &notice).await
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

/// Records that the restart notice was raised. A pref that cannot be stored is logged: the notice
/// itself is already up, and it may come once more.
async fn remember_restart_notice(state: &AppState) {
    let prefs = state.prefs.clone();

    let stored = async_runtime::spawn_blocking(move || {
        prefs.with(|store| store.update(|prefs| prefs.knowledge_restart_notice_done = true))
    })
    .await;

    if !matches!(stored, Ok(Ok(_))) {
        tracing::warn!("the restart notice of Harness Knowledge could not be remembered");
    }
}

/// The counts while the server answers, and none (zero, so no badge) while project knowledge is off.
async fn read(state: &AppState, notice: &KnowledgeUpgradeNotice) -> Option<KnowledgePulse> {
    if !crate::sidecar_setup::enabled::<crate::product::Knowledge>(state).await {
        return Some(KnowledgePulse {
            documents: 0,
            generation: 0,
            held: Some(0),
            offline: false,
            offline_reason: None,
            waiting: Some(0),
            waiting_all: Some(0),
            approval: None,
            restart_harnesses: None,
            upgrade_waits: None,
            me: None,
        });
    }

    knowledge_pulse_upgrading(&state.env, &state.runner, &state.knowledge_features, notice).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_counts_are_read_often_only_while_the_view_is_open() {
        assert_eq!(PulseWatch::interval(true), Duration::from_secs(4));
        assert_eq!(PulseWatch::interval(false), Duration::from_secs(30));
    }
}
