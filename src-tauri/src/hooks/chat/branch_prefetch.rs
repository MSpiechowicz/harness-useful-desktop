//! The Git branch lookup of a chat, started when the chat starts.
//!
//! The Git branch step fires at the chat's first message, and its lookup is mostly the background
//! fetch (see [`super::lookups`]). So a fresh chat whose project has the step starts the lookup in
//! [`ChatHooks::prefetch_branch`](super::ChatHooks), and the first message finds it done, or waits
//! only for what is left. The note is used once, by the first message that wants the step.
//!
//! The lookup is the same as at the message and ends on its own within its cap, so a chat that is
//! closed before its first message leaves nothing running. A note that is older than [`FRESH_FOR`]
//! or was looked up for another folder (the chat moved) is not used: the lookup runs at the message.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use harness_business::hooks::{HookSet, has_seed_hook};
use harness_business::model::HookSeed;
use tauri::async_runtime::{JoinHandle, spawn};

/// How long a started lookup stays usable. A chat's first message comes within minutes of its start,
/// and a later `sessionStart` (a new session in the same chat) wants a note of its own.
pub(super) const FRESH_FOR: Duration = Duration::from_secs(5 * 60);

/// A lookup that started with the chat.
struct Started {
    folder: PathBuf,
    at: Instant,
    note: JoinHandle<Option<String>>,
}

/// The chat's started branch lookup, if any.
#[derive(Default)]
pub(super) struct BranchPrefetch {
    started: Mutex<Option<Started>>,
}

/// True when a chat that starts now (`fresh`: not a resumed session) fires the Git branch step at
/// its first message, so its lookup is worth starting.
pub(super) fn wanted_at_start(fresh: bool, hooks: &HookSet) -> bool {
    fresh && has_seed_hook(hooks, HookSeed::BranchSync)
}

impl BranchPrefetch {
    /// Starts `lookup` for `folder` in the background, replacing one that was started before.
    pub(super) fn start(&self, folder: &Path, lookup: impl Future<Output = Option<String>> + Send + 'static) {
        let started = Started {
            folder: folder.to_path_buf(),
            at: Instant::now(),
            note: spawn(lookup),
        };

        *self.started.lock().unwrap_or_else(PoisonError::into_inner) = Some(started);
    }

    /// The note for `folder` and whether it came from the started lookup (waited for when it still
    /// runs). Without a usable started lookup, `lookup` runs now.
    pub(super) async fn note<F, Fut>(&self, folder: &Path, lookup: F) -> (Option<String>, bool)
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<String>>,
    {
        match self.take(folder, Instant::now()) {
            Some(started) => (started.await.ok().flatten(), true),
            None => (lookup().await, false),
        }
    }

    /// The started lookup when it is for `folder` and still fresh at `now`. It is used once.
    pub(super) fn take(&self, folder: &Path, now: Instant) -> Option<JoinHandle<Option<String>>> {
        let started = self.started.lock().unwrap_or_else(PoisonError::into_inner).take()?;

        let usable = started.folder == folder && now.saturating_duration_since(started.at) < FRESH_FOR;

        usable.then_some(started.note)
    }
}
