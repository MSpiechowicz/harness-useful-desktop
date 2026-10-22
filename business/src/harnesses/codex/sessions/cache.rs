//! The caches of [`super::CodexSessions`]: entries per binary, project and (for reads) session,
//! each with the time it was stored.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use crate::harness::CliSessionCtx;

/// What a cache entry belongs to: the binary, the project and, for reads, the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CacheKey {
    binary: PathBuf,
    project: PathBuf,
    pub(super) session: Option<String>,
}

impl CacheKey {
    pub(super) fn new(ctx: &CliSessionCtx<'_>, session: Option<&str>) -> Self {
        Self {
            binary: ctx.binary.path.clone(),
            project: ctx.project.to_path_buf(),
            session: session.map(str::to_owned),
        }
    }
}

#[derive(Debug)]
pub(super) struct Cached<T> {
    pub(super) key: CacheKey,
    at: Instant,
    value: Arc<T>,
}

pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The value cached for `key` when stored at `since` or later (any time without `since`).
pub(super) fn cached<T>(cache: &Mutex<Vec<Cached<T>>>, key: &CacheKey, since: Option<Instant>) -> Option<Arc<T>> {
    let fresh = |entry: &Cached<T>| since.is_none_or(|since| entry.at >= since);

    lock(cache)
        .iter()
        .find(|entry| &entry.key == key && fresh(entry))
        .map(|entry| Arc::clone(&entry.value))
}

/// Caches `value` for `key`, keeping at most `max` entries (the oldest go first).
pub(super) fn store<T>(cache: &Mutex<Vec<Cached<T>>>, key: CacheKey, value: T, max: usize) -> Arc<T> {
    let value = Arc::new(value);
    let mut entries = lock(cache);

    entries.retain(|entry| entry.key != key);
    entries.push(Cached {
        key,
        at: Instant::now(),
        value: Arc::clone(&value),
    });

    let excess = entries.len().saturating_sub(max);
    entries.drain(..excess);

    value
}
