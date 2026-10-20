//! The projects the views scope their tables by (`project_scopes`): the active project and the
//! recent ones, each with its project root and the Harness Memory repository key of that root.
//!
//! A key is asked of the installed server (`harness-memory hooks list --project <root>`, one row:
//! it answers the identity of the root whether or not hooks are shared), at most [`KEYS_AT_ONCE`]
//! at a time, and kept for the session in [`ProjectKeys`]; opening or forgetting a project drops
//! its entry. A server that is missing, predates the command or fails leaves the key `None` and
//! never fails the list. That answer is kept for [`RETRY_AFTER`] while the binary is unchanged.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use super::{Projects, utf8};
use crate::ecosystem::memory_cli::is_repo_key;
use crate::ecosystem::memory_hooks::project_key;
use crate::ecosystem::resolve_memory;
use crate::env::Env;
use crate::model::{ProjectInfo, ProjectScope};
use crate::process::{BoxFuture, ProcessRunner, join_all};
use crate::signed_docs::project_identity;

/// The most keys asked of the server at once.
pub const KEYS_AT_ONCE: usize = 12;

/// How long a key the server did not give stays unasked (while the binary is unchanged).
pub const RETRY_AFTER: Duration = Duration::from_secs(5 * 60);

/// The repository key of each project path, for the session.
#[derive(Debug, Default)]
pub struct ProjectKeys {
    keys: Mutex<HashMap<String, Kept>>,
}

/// What [`ProjectKeys`] keeps of one path.
#[derive(Debug, Clone)]
enum Kept {
    /// The key the server gave.
    Key(String),
    /// The server gave none at `at`; `binary` is the server it asked.
    Missing { at: Instant, binary: Binary },
}

/// The installed server's path and modification time.
type Binary = (PathBuf, Option<SystemTime>);

impl ProjectKeys {
    /// Drops what is kept of `path`, so the next list asks for its key again.
    pub fn forget(&self, path: &str) {
        self.lock().remove(path);
    }

    /// The kept key of `path`: `Some(None)` while a missing one is not asked again (`binary` is the
    /// installed server, `None` when there is none), `None` when it must be asked.
    fn kept(&self, path: &str, binary: Option<&Binary>, now: Instant) -> Option<Option<String>> {
        match self.lock().get(path)? {
            Kept::Key(key) => Some(Some(key.clone())),
            Kept::Missing { at, binary: asked } => {
                let fresh = now.saturating_duration_since(*at) < RETRY_AFTER && binary == Some(asked);

                fresh.then_some(None)
            }
        }
    }

    fn keep(&self, path: &str, kept: Kept) {
        self.lock().insert(path.to_owned(), kept);
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Kept>> {
        self.keys.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Projects<'_> {
    /// The active project, then the recent ones (each once, most recent first), each with its
    /// project root ([`project_identity`]; `None` for a folder that is gone) and no key yet. Reads
    /// the folders.
    pub fn scopes(&self) -> Vec<ProjectScope> {
        let recent = self.recent();
        let active = self
            .active()
            .map(|path| ProjectInfo::inspect(&path))
            .filter(|active| !recent.iter().any(|project| project.path == active.path));

        active
            .into_iter()
            .chain(recent)
            .map(|project| ProjectScope {
                root: root_of(Path::new(&project.path)),
                path: project.path,
                name: project.name,
                key: None,
            })
            .collect()
    }
}

/// `scopes` with the repository key of each root: the kept one, else asked of the installed
/// Harness Memory (see the module docs). Never fails: a key it cannot get is `None`.
pub async fn with_keys(
    env: &Env,
    runner: &dyn ProcessRunner,
    keys: &ProjectKeys,
    scopes: Vec<ProjectScope>,
) -> Vec<ProjectScope> {
    let binary = resolve_memory(env).map(|binary| {
        let modified = std::fs::metadata(&binary.path)
            .and_then(|metadata| metadata.modified())
            .ok();

        (binary.path, modified)
    });

    let ask = |root: String| -> BoxFuture<'_, Option<String>> {
        let server = binary.as_ref().map(|(path, _)| path.clone());

        Box::pin(async move {
            let key = project_key(env, runner, &server?, Path::new(&root)).await.ok()?.key;

            is_repo_key(&key).then_some(key)
        })
    };

    resolve_keys(keys, scopes, binary.as_ref(), &ask).await
}

/// Fills in the key of each of `scopes` from `keys`, asking `ask` for the root of each one not
/// kept ([`KEYS_AT_ONCE`] at a time) while a server is installed (`binary`), and keeping its
/// answer. A scope without a root gets only a key already kept.
async fn resolve_keys<'a>(
    keys: &ProjectKeys,
    mut scopes: Vec<ProjectScope>,
    binary: Option<&Binary>,
    ask: &(dyn Fn(String) -> BoxFuture<'a, Option<String>> + Sync),
) -> Vec<ProjectScope> {
    let now = Instant::now();
    let mut unknown = Vec::new();

    for (index, scope) in scopes.iter_mut().enumerate() {
        match keys.kept(&scope.path, binary, now) {
            Some(kept) => scope.key = kept,
            None => unknown.extend(scope.root.clone().map(|root| (index, root))),
        }
    }

    let Some(binary) = binary else {
        return scopes;
    };

    for batch in unknown.chunks(KEYS_AT_ONCE) {
        let asked = batch.iter().map(|(_, root)| ask(root.clone())).collect();

        for ((index, _), key) in batch.iter().zip(join_all(asked).await) {
            let scope = &mut scopes[*index];
            let kept = match &key {
                Some(key) => Kept::Key(key.clone()),
                None => Kept::Missing {
                    at: now,
                    binary: binary.clone(),
                },
            };

            keys.keep(&scope.path, kept);
            scope.key = key;
        }
    }

    scopes
}

/// The project root of `folder` as hooks and routines store it, `None` when it is gone (or not
/// UTF-8).
fn root_of(folder: &Path) -> Option<String> {
    let root = project_identity(folder).ok()?;

    utf8(&root).ok()
}
