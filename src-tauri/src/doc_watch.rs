//! Notices changes to a signed document folder and its approvals (see
//! `harness_business::signed_docs`): a thread lists them every [`POLL_INTERVAL`] (names,
//! modification times and sizes only, nothing is read). On a change, [`DocWatch::poll`] drops the
//! approvals and approved copies of files that are gone (the MCP server never writes approvals,
//! so an agent's removal leaves them behind) and tells its owner, which reads the documents again
//! and tells its view.

use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use harness_business::signed_docs::{DocKind, DocStore};

/// How often the folder is listed.
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// The state of the folder and the approvals file: each entry's name, modification time and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    files: Vec<(String, Option<SystemTime>, u64)>,
    approvals: Option<(Option<SystemTime>, u64)>,
}

impl Fingerprint {
    fn of<K: DocKind>(store: &DocStore<K>) -> Self {
        let mut files: Vec<(String, Option<SystemTime>, u64)> = std::fs::read_dir(store.docs_dir())
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
            approvals: stamp(&store.config_dir().join(K::APPROVALS_FILE)),
        }
    }
}

fn stamp(path: &Path) -> Option<(Option<SystemTime>, u64)> {
    let metadata = std::fs::metadata(path).ok()?;

    Some((metadata.modified().ok(), metadata.len()))
}

/// The fingerprint of the latest look (none before the first).
#[derive(Debug, Default)]
pub struct DocWatch {
    watched: Mutex<Option<Fingerprint>>,
}

impl DocWatch {
    /// One look at `store`'s folder and approvals. A change drops the approvals of files that are
    /// gone, under `writes` (the owner's write lock). The first look only takes the fingerprint
    /// and drops what is left of removed documents. True when something changed since the look
    /// before.
    pub fn poll<K: DocKind>(&self, store: &DocStore<K>, writes: &Mutex<()>) -> bool {
        let now = Fingerprint::of(store);
        let mut watched = self.watched.lock().unwrap_or_else(PoisonError::into_inner);

        if watched.as_ref() == Some(&now) {
            return false;
        }

        let first = watched.is_none();
        let pruned = prune(store, writes);

        // The pruned approvals file is part of this change, not the next one.
        *watched = Some(if pruned { Fingerprint::of(store) } else { now });
        drop(watched);

        !first
    }
}

/// Drops the approvals of documents whose file is gone; true when it did.
fn prune<K: DocKind>(store: &DocStore<K>, writes: &Mutex<()>) -> bool {
    let _writing = writes.lock().unwrap_or_else(PoisonError::into_inner);

    match store.prune_approvals() {
        Ok(0) => false,
        Ok(dropped) => {
            tracing::info!(dropped, "approvals of removed {} dropped", K::PLURAL);
            true
        }
        Err(error) => {
            tracing::warn!(code = ?error.code(), "approvals of removed {} not dropped", K::PLURAL);
            false
        }
    }
}

/// Starts the watcher thread of kind `K` (it runs until the app exits): `look` every
/// [`POLL_INTERVAL`].
pub fn spawn<K: DocKind>(mut look: impl FnMut() + Send + 'static) {
    let started = std::thread::Builder::new()
        .name(format!("{}-watch", K::PLURAL))
        .spawn(move || {
            loop {
                look();
                std::thread::sleep(POLL_INTERVAL);
            }
        });

    if let Err(error) = started {
        tracing::warn!(
            %error,
            "the {} watcher did not start; changes show after a restart",
            K::PLURAL
        );
    }
}
