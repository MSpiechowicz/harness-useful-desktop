//! The parents of Codex threads, for the session tree. Codex 0.154's `thread/list` answers
//! `forkedFromId: null` for every thread, forks too: only `thread/read` names the real parent. A
//! fork's parent never changes, so each thread is read once and the answer (the parent's id, or
//! none for a thread that is no fork) is kept by thread id, in memory and in
//! `<local data>/codex-thread-parents.json` ([`PARENTS_FILE`]), so a restart reads nothing again.
//!
//! The file holds thread ids only: `{"version": 1, "threads": {"<id>": "<parent id>" | null}}`,
//! the newest [`MAX_PARENTS`] entries in the order they were learnt, `0600` (the folder `0700`),
//! replaced in one rename and never written through a symbolic link. A file that cannot be used
//! (too large, not the expected JSON or version) reads as empty and is replaced on the next
//! write. Without a local data folder the parents are kept in memory for the app run.
//!
//! A list returns at once with the parents known ([`Shared::fill`]). The threads it names without
//! a known parent are read in the background ([`resolve_later`]): one batch at a time, inside the
//! sessions' run lock (one app-server at a time), at most [`PARENT_BATCH`] ids per run, no new read
//! past [`BATCH_TIME`] or the run's read budget, each thread checked to be of the project as any
//! read is. The Sessions view lists again every few seconds, so the parents show once read, and
//! that list starts the next batch. A deleted thread leaves the cache ([`Shared::forget`]).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::app_server::AppServer;
use super::cache::lock;
use super::owned_thread;
use crate::app_dirs::local_data_dir;
use crate::codex_rpc::wire::results::Thread;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically};
use crate::harness::CliSessionCtx;
use crate::model::SessionSummary;
use crate::pi_family::sessions::is_safe_session_id;
use crate::process::{ResolvedBinary, StdRunner};
use crate::signed_docs::json_file::{Stored, read_stored};
use crate::signed_docs::pretty_json;

/// The file in the app's local data folder.
pub const PARENTS_FILE: &str = "codex-thread-parents.json";

/// Entries kept (the oldest go first).
const MAX_PARENTS: usize = 10_000;

/// Largest file read.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Threads read per background run.
pub(super) const PARENT_BATCH: usize = 100;

/// No new read of a background run starts past this.
const BATCH_TIME: Duration = Duration::from_secs(3);

#[derive(Serialize, Deserialize)]
struct ParentsFile {
    version: u32,
    /// Oldest first (the JSON object keeps the order it was written in).
    threads: Map<String, Value>,
}

/// What [`super::CodexSessions`] shares with its background reads: the run lock and the parents.
#[derive(Debug, Default)]
pub(super) struct Shared {
    /// Held while an app-server runs.
    pub(super) runs: Mutex<()>,
    parents: Mutex<Parents>,
    /// Set while a background batch runs.
    resolving: AtomicBool,
}

/// The parents known, by thread id (see the module docs).
#[derive(Debug, Default)]
struct Parents {
    /// The file they were loaded from; `None` before the first load or without a data folder.
    path: Option<PathBuf>,
    loaded: bool,
    /// `"<parent id>"`, or `null` for a thread that is no fork.
    threads: Map<String, Value>,
}

impl Shared {
    /// Fills in the known parent of each session the list did not name one for, and returns the
    /// ids whose parent is not known yet, in list order.
    pub(super) fn fill(&self, env: &Env, sessions: &mut [SessionSummary]) -> Vec<String> {
        let mut parents = lock(&self.parents);
        let parents = parents.of(env);
        let mut unknown = Vec::new();

        for summary in sessions.iter_mut().filter(|summary| summary.parent_session.is_none()) {
            match parents.get(&summary.session.id) {
                Some(parent) => summary.parent_session = parent,
                None => unknown.push(summary.session.id.clone()),
            }
        }

        unknown
    }

    /// Keeps what a read of `thread` told about its parent, when it was not known.
    pub(super) fn learn(&self, env: &Env, thread: &Thread) {
        let mut parents = lock(&self.parents);
        let parents = parents.of(env);

        if parents.get(&thread.id).is_none() {
            parents.insert(vec![(thread.id.clone(), parent_of(thread))]);
        }
    }

    /// Drops the entry of the deleted thread `id`.
    pub(super) fn forget(&self, env: &Env, id: &str) {
        let mut parents = lock(&self.parents);
        let parents = parents.of(env);

        if parents.threads.shift_remove(id).is_some() {
            parents.save();
        }
    }

    /// The ids of `ids` whose parent is still not known.
    fn unknown(&self, env: &Env, ids: Vec<String>) -> Vec<String> {
        let mut parents = lock(&self.parents);
        let parents = parents.of(env);

        ids.into_iter().filter(|id| parents.get(id).is_none()).collect()
    }
}

impl Parents {
    /// The parents of `env`'s data folder, loaded from its file on first use.
    fn of(&mut self, env: &Env) -> &mut Self {
        let path = local_data_dir(env).map(|dir| dir.join(PARENTS_FILE));

        if !self.loaded || self.path != path {
            self.threads = path.as_deref().map(load).unwrap_or_default();
            self.path = path;
            self.loaded = true;
        }

        self
    }

    /// `Some(parent)` when known (`Some(None)`: no fork), `None` when not.
    fn get(&self, id: &str) -> Option<Option<String>> {
        match self.threads.get(id)? {
            Value::String(parent) => Some(Some(parent.clone())),
            _ => Some(None),
        }
    }

    /// Adds `found` as the newest entries, drops the oldest past [`MAX_PARENTS`] and saves.
    fn insert(&mut self, found: Vec<(String, Option<String>)>) {
        for (id, parent) in found {
            self.threads.insert(id, parent.map_or(Value::Null, Value::String));
        }

        let excess = self.threads.len().saturating_sub(MAX_PARENTS);

        if excess > 0 {
            self.threads = std::mem::take(&mut self.threads).into_iter().skip(excess).collect();
        }

        self.save();
    }

    /// Writes the file; a failure keeps the parents in memory for the app run.
    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };

        if let Err(error) = write(path, &self.threads) {
            tracing::warn!(code = ?error.code(), "codex sessions: the thread parents cannot be saved");
        }
    }
}

/// The entries of the file at `path`: none when it is missing or cannot be used, and only the
/// entries made of thread ids.
fn load(path: &Path) -> Map<String, Value> {
    let file = match read_stored::<ParentsFile>(path, MAX_FILE_BYTES, |file| file.version == 1) {
        Ok(Stored::Valid(file)) => file,
        Ok(Stored::Missing) => return Map::new(),
        Ok(Stored::Invalid) => {
            tracing::warn!("codex sessions: the thread parents file is invalid; started again");
            return Map::new();
        }
        Err(error) => {
            tracing::warn!(code = ?error.code(), "codex sessions: the thread parents cannot be read");
            return Map::new();
        }
    };

    file.threads
        .into_iter()
        .filter(|(id, parent)| {
            let parent_ok = match parent {
                Value::Null => true,
                Value::String(parent) => is_safe_session_id(parent) && parent != id,
                _ => false,
            };

            is_safe_session_id(id) && parent_ok
        })
        .collect()
}

fn write(path: &Path, threads: &Map<String, Value>) -> Result<()> {
    #[derive(Serialize)]
    struct Written<'a> {
        version: u32,
        threads: &'a Map<String, Value>,
    }

    let what = "the Codex thread parents";

    if let Some(parent) = path.parent() {
        refuse_link(parent)?;
        create_private_dir(parent, &format!("creating the folder of {what}"))?;
    }

    refuse_link(path)?;

    let bytes = pretty_json(&Written { version: 1, threads }, what)?;

    replace_atomically(path, &bytes, &format!("saving {what}"))
}

/// The thread `thread` was forked from, when it names a valid one.
fn parent_of(thread: &Thread) -> Option<String> {
    thread
        .forked_from_id
        .clone()
        .filter(|parent| is_safe_session_id(parent) && *parent != thread.id)
}

/// The limits of a background run.
#[derive(Debug, Clone, Copy)]
pub(super) struct Batch {
    /// Threads read at most (none at 0).
    pub(super) size: usize,
    pub(super) timeout: Duration,
    pub(super) budget: usize,
}

/// Reads the parents of the first `batch.size` of `ids` (threads of `ctx`'s project the list
/// returned) on a background thread, unless a batch runs already: the next list asks again.
pub(super) fn resolve_later(shared: &Arc<Shared>, ctx: &CliSessionCtx<'_>, mut ids: Vec<String>, batch: Batch) {
    if batch.size == 0 || ids.is_empty() || shared.resolving.swap(true, Ordering::AcqRel) {
        return;
    }

    ids.truncate(batch.size);

    let job = Job {
        resolving: Resolving(Arc::clone(shared)),
        env: ctx.env.clone(),
        project: ctx.project.to_path_buf(),
        binary: ctx.binary.clone(),
        ids,
        batch,
    };

    // A thread that does not start drops the job, which clears `resolving`.
    let started = std::thread::Builder::new()
        .name("codex-thread-parents".to_owned())
        .spawn(move || job.run());

    if started.is_err() {
        tracing::warn!("codex sessions: the thread parents cannot be read in the background");
    }
}

/// Clears `resolving` when the batch ends, however it ends.
struct Resolving(Arc<Shared>);

impl Drop for Resolving {
    fn drop(&mut self) {
        self.0.resolving.store(false, Ordering::Release);
    }
}

/// One background batch.
struct Job {
    resolving: Resolving,
    env: Env,
    project: PathBuf,
    binary: ResolvedBinary,
    ids: Vec<String>,
    batch: Batch,
}

impl Job {
    fn run(self) {
        let shared = &self.resolving.0;
        let ctx = CliSessionCtx {
            env: &self.env,
            project: &self.project,
            binary: &self.binary,
            runner: &StdRunner,
        };

        let found = {
            let _run = lock(&shared.runs);

            // Read meanwhile (a transcript, or the batch a list waited for).
            let ids = shared.unknown(&self.env, self.ids.clone());

            if ids.is_empty() {
                return;
            }

            read_parents(&ctx, &ids, self.batch)
        };

        if !found.is_empty() {
            lock(&shared.parents).of(&self.env).insert(found);
        }
    }
}

/// The parent of each of `ids` one app-server run reads (see the module docs). A thread that is
/// gone or of another folder is skipped; any other failure ends the run with what was read.
fn read_parents(ctx: &CliSessionCtx<'_>, ids: &[String], batch: Batch) -> Vec<(String, Option<String>)> {
    let mut server = match AppServer::start(ctx, batch.timeout) {
        Ok(server) => server,
        Err(error) => {
            tracing::warn!(code = ?error.code(), "codex sessions: the thread parents cannot be read");
            return Vec::new();
        }
    };

    let until = Instant::now() + BATCH_TIME;
    let mut found = Vec::new();

    for id in ids {
        match owned_thread(&mut server, ctx, id) {
            Ok(thread) => found.push((id.clone(), parent_of(&thread))),
            Err(BusinessError::NotFound(_)) => {}
            Err(error) => {
                tracing::warn!(code = ?error.code(), "codex sessions: reading the thread parents stopped");
                return found;
            }
        }

        if Instant::now() >= until || server.read_bytes() >= batch.budget {
            break;
        }
    }

    server.finish();

    found
}
