//! Notices changes to the hooks folder and the approvals: a thread lists them every
//! [`POLL_INTERVAL`] (names, modification times and sizes only, nothing is read). A change reads
//! the hooks again on their next firing, drops the approvals and approved copies of files that are
//! gone (the MCP server never writes approvals, so an agent's removal leaves them behind) and
//! sends `changed` to the Hooks view.

use std::path::Path;
use std::sync::{Arc, PoisonError};
use std::time::{Duration, SystemTime};

use harness_business::hooks::{APPROVALS_FILE, HookStore};

use super::HookRuntime;

/// How often the folder is listed.
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// The state of the folder and the approvals file: each entry's name, modification time and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    files: Vec<(String, Option<SystemTime>, u64)>,
    approvals: Option<(Option<SystemTime>, u64)>,
}

impl Fingerprint {
    fn of(store: &HookStore) -> Self {
        let mut files: Vec<(String, Option<SystemTime>, u64)> = std::fs::read_dir(store.hooks_dir())
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name().into_string().ok()?;

                // Temp files of a write come and go; the rename that ends it shows.
                if name.starts_with('.') || !name.ends_with(".json") {
                    return None;
                }

                let metadata = entry.metadata().ok()?;
                Some((name, metadata.modified().ok(), metadata.len()))
            })
            .collect();
        files.sort();

        Self {
            files,
            approvals: stamp(&store.config_dir().join(APPROVALS_FILE)),
        }
    }
}

fn stamp(path: &Path) -> Option<(Option<SystemTime>, u64)> {
    let metadata = std::fs::metadata(path).ok()?;

    Some((metadata.modified().ok(), metadata.len()))
}

/// Starts the watcher thread (it runs until the app exits). Does nothing without a hooks folder.
pub fn spawn(runtime: Arc<HookRuntime>) {
    if runtime.store.is_none() {
        return;
    }

    let started = std::thread::Builder::new()
        .name("hooks-watch".to_owned())
        .spawn(move || {
            loop {
                runtime.poll();
                std::thread::sleep(POLL_INTERVAL);
            }
        });

    if let Err(error) = started {
        tracing::warn!(%error, "the hooks watcher did not start; changes show after a restart");
    }
}

impl HookRuntime {
    /// One look at the folder and the approvals (see the module docs). The first look only takes
    /// the fingerprint and drops what is left of removed hooks. True when something changed.
    pub fn poll(&self) -> bool {
        let Some(store) = &self.store else {
            return false;
        };

        let now = Fingerprint::of(store);
        let mut watched = self.watched.lock().unwrap_or_else(PoisonError::into_inner);

        if watched.as_ref() == Some(&now) {
            return false;
        }

        let first = watched.is_none();
        let pruned = self.prune(store);

        // The pruned approvals file is part of this change, not the next one.
        *watched = Some(if pruned { Fingerprint::of(store) } else { now });
        drop(watched);

        if first {
            return false;
        }

        self.invalidate();
        self.notify();
        true
    }

    /// Drops the approvals of hooks whose file is gone; true when it did.
    fn prune(&self, store: &HookStore) -> bool {
        let _writing = self.writes.lock().unwrap_or_else(PoisonError::into_inner);

        match store.prune_approvals() {
            Ok(0) => false,
            Ok(dropped) => {
                tracing::info!(dropped, "approvals of removed hooks dropped");
                true
            }
            Err(error) => {
                tracing::warn!(code = ?error.code(), "approvals of removed hooks not dropped");
                false
            }
        }
    }
}
