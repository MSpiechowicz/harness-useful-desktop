//! The file lists of the projects searched lately, reused for a few seconds.
//!
//! The `@` menu searches on every keystroke. Listing a large repository takes far longer than
//! matching a query against the list, so a list is built once, shared by the requests that follow
//! and reused until it is [`PROJECT_FILES_TTL`] old. An old list is freed at the next search (there is
//! no timer). At most [`MAX_CACHED_PROJECTS`] projects keep a
//! list. Requests that arrive while a list is being built wait for it: one project never has more
//! than one listing at a time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::ProjectFileIndex;
use crate::env::Env;
use crate::error::Result;
use crate::model::{MAX_PROJECT_FILE_QUERY_BYTES, ProjectFile};
use crate::process::ProcessRunner;

/// How long a list is reused.
pub const PROJECT_FILES_TTL: Duration = Duration::from_secs(5);

/// Projects that keep a list at once; the one searched longest ago gives way to a new one.
pub const MAX_CACHED_PROJECTS: usize = 8;

/// The lists of the projects searched lately.
pub struct ProjectFileCache {
    ttl: Duration,
    slots: Mutex<HashMap<PathBuf, (Instant, Arc<Slot>)>>,
}

/// The list of one project.
#[derive(Default)]
struct Slot {
    /// Held while the list is built, so concurrent requests wait instead of building it again.
    building: tokio::sync::Mutex<()>,
    built: Mutex<Option<(Instant, Arc<ProjectFileIndex>)>>,
}

impl Slot {
    fn fresh(&self, ttl: Duration) -> Option<Arc<ProjectFileIndex>> {
        let built = self.built.lock().unwrap_or_else(PoisonError::into_inner);

        built
            .as_ref()
            .filter(|(at, _)| at.elapsed() < ttl)
            .map(|(_, index)| index.clone())
    }

    fn is_idle_and_stale(&self, ttl: Duration) -> bool {
        self.fresh(ttl).is_none() && self.building.try_lock().is_ok()
    }
}

impl Default for ProjectFileCache {
    fn default() -> Self {
        Self::with_ttl(PROJECT_FILES_TTL)
    }
}

impl ProjectFileCache {
    /// A cache that reuses a list for `ttl`.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            ttl,
            slots: Mutex::new(HashMap::new()),
        }
    }

    /// The matches of `query` among the files of the canonical project directory `project`, at most
    /// `limit` (1 to 50), best first. Builds the list when there is no fresh one, which blocks (git
    /// runs, folders are read): call it from a blocking task. An over-long `query` matches
    /// nothing and builds nothing.
    pub async fn search(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
        project: &Path,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ProjectFile>> {
        if query.len() > MAX_PROJECT_FILE_QUERY_BYTES {
            return Ok(Vec::new());
        }

        let index = self.index(env, runner, project).await?;

        Ok(index.search(project, query, limit))
    }

    async fn index(&self, env: &Env, runner: &dyn ProcessRunner, project: &Path) -> Result<Arc<ProjectFileIndex>> {
        let slot = self.slot(project);

        if let Some(index) = slot.fresh(self.ttl) {
            return Ok(index);
        }

        let _building = slot.building.lock().await;

        // Another request may have built it while this one waited.
        if let Some(index) = slot.fresh(self.ttl) {
            return Ok(index);
        }

        let index = Arc::new(ProjectFileIndex::build(env, runner, project).await?);
        *slot.built.lock().unwrap_or_else(PoisonError::into_inner) = Some((Instant::now(), index.clone()));

        Ok(index)
    }

    /// The slot of `project`, marked as used now. Slots whose list has expired and that no request
    /// uses are dropped first, then the least recently used one if the cache is still full.
    fn slot(&self, project: &Path) -> Arc<Slot> {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);

        slots.retain(|_, (_, slot)| Arc::strong_count(slot) > 1 || !slot.is_idle_and_stale(self.ttl));

        if !slots.contains_key(project) && slots.len() >= MAX_CACHED_PROJECTS {
            let oldest = slots
                .iter()
                .filter(|(_, (_, slot))| Arc::strong_count(slot) == 1)
                .min_by_key(|(_, (used, _))| *used)
                .map(|(key, _)| key.clone());

            if let Some(key) = oldest {
                slots.remove(&key);
            }
        }

        let (used, slot) = slots
            .entry(project.to_path_buf())
            .or_insert_with(|| (Instant::now(), Arc::default()));
        *used = Instant::now();

        slot.clone()
    }
}
