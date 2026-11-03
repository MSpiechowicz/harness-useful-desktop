//! What the app looks up itself for the Git branch and Harness Dashboard steps of a prompt (the
//! memory and knowledge notes are `harness_business::hooks::run_with_recall`'s).
//!
//! Both are best effort and deterministic: a failure, a slow answer or nothing to say is `None`
//! (no gauges), and the message goes out either way. The app only reads and fetches: it never
//! merges, pulls or switches.
//!
//! - [`branch_note`]: a hardened background fetch (at most [`FETCH_CAP`], else the last fetched
//!   state is used), under the repository's write lock like the Git popover's own background
//!   fetch (busy: no fetch), then where the branch stands against main
//!   ([`harness_business::git::behind`]) and the commits it lacks.
//! - [`limit_gauges`]: the windows and budgets of the Harness Dashboard that are close, for the
//!   chat's harness.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use harness_business::ecosystem::{Gauge, close_gauges};
use harness_business::env::Env;
use harness_business::git::{self, NOTE_COMMITS, behind, behind_note};
use harness_business::model::BranchSync;
use harness_business::process::TokioRunner;
use tauri::async_runtime::{block_on, spawn_blocking};

use crate::state::{AppState, GitWriteLocks, SharedPrefs};

/// How long the background fetch may take before the last fetched state is used.
pub const FETCH_CAP: Duration = Duration::from_secs(5);

/// How long the whole branch lookup may take.
pub const BRANCH_CAP: Duration = Duration::from_secs(8);

/// What the branch lookup needs of the app, owned so a lookup can run in the background (the chat's
/// prefetch, see [`super::branch_prefetch`]).
pub(super) struct BranchLookup {
    env: Arc<Env>,
    runner: TokioRunner,
    prefs: SharedPrefs,
    git_writes: Arc<GitWriteLocks>,
}

impl BranchLookup {
    pub(super) fn of(state: &AppState) -> Self {
        Self {
            env: state.env.clone(),
            runner: state.runner,
            prefs: state.prefs.clone(),
            git_writes: state.git_writes.clone(),
        }
    }

    /// The sentence about the branch of `folder`, or `None` when it has nothing to say. Ends within
    /// [`BRANCH_CAP`].
    pub(super) async fn note(self, folder: PathBuf) -> Option<String> {
        tokio::time::timeout(BRANCH_CAP, self.lookup(folder))
            .await
            .ok()
            .flatten()
    }

    async fn lookup(self, dir: PathBuf) -> Option<String> {
        let picked = picked_main(self.prefs, &dir).await;
        let (env, runner) = (self.env, self.runner);

        let common = {
            let (env, dir) = (env.clone(), dir.clone());

            spawn_blocking(move || block_on(git::git_common_dir(&env, &runner, &dir)))
                .await
                .ok()
                .flatten()
        };

        // Held for the whole lookup, and by the task itself, so it outlives a lookup that timed out.
        let lock = self.git_writes.of(&common.unwrap_or_else(|| dir.clone()));
        let held = lock.try_lock_owned().ok();

        spawn_blocking(move || {
            block_on(async {
                let read = read_branch(&env, &runner, &dir, picked.as_deref(), held.is_some()).await?;
                let gap = behind(&read.sync)?;

                let commits = git::commits_between(&env, &runner, &dir, &gap.reference, NOTE_COMMITS)
                    .await
                    .unwrap_or_default();

                Some(behind_note(&gap, &commits, read.fetched))
            })
        })
        .await
        .ok()
        .flatten()
    }
}

/// The sentence about the branch of `folder`, or `None` when it has nothing to say.
pub(super) async fn branch_note(state: &AppState, folder: &Path) -> Option<String> {
    BranchLookup::of(state).note(folder.to_path_buf()).await
}

/// The state of a repository after the fetch that ran, if one did.
struct Fetched {
    sync: BranchSync,
    fetched: bool,
}

/// The branch state of `dir`; `None` outside a repository or when git fails. A fetch runs first
/// when `may_fetch`, capped at [`FETCH_CAP`].
async fn read_branch(
    env: &harness_business::env::Env,
    runner: &harness_business::process::TokioRunner,
    dir: &Path,
    picked: Option<&str>,
    may_fetch: bool,
) -> Option<Fetched> {
    let before = git::git_sync_state(env, runner, dir, picked).await.ok()?;

    if !before.is_repo || before.remote.is_none() || !may_fetch {
        return Some(Fetched {
            sync: before,
            fetched: false,
        });
    }

    let fetch = git::git_fetch(env, runner, dir, picked, true);

    match tokio::time::timeout(FETCH_CAP, fetch).await {
        Ok(Ok(after)) => {
            // A fetch that was skipped (the repository's own configuration names a program) leaves
            // FETCH_HEAD as it was.
            let fetched = after.last_fetched != before.last_fetched;

            Some(Fetched { sync: after, fetched })
        }
        Ok(Err(_)) | Err(_) => Some(Fetched {
            sync: before,
            fetched: false,
        }),
    }
}

/// The main branch the user picked for `folder`, if any.
async fn picked_main(prefs: SharedPrefs, folder: &Path) -> Option<String> {
    let project = folder.to_string_lossy().into_owned();

    spawn_blocking(move || prefs.with(|store| store.load().picked_main_branch(&project).map(str::to_owned)))
        .await
        .ok()
        .flatten()
}

/// The windows and budgets that are close, for a chat in `project` whose plan is `providers`, from the limits
/// cache ([`super::limits_cache`]): the budgets of other projects are left out. None when the dashboard is
/// missing or gives no reading in time.
pub(super) async fn limit_gauges(state: &AppState, providers: &[String], project: Option<&str>) -> Vec<Gauge> {
    match super::limits_cache::limits_reading(&state.dashboard, &state.env).await {
        Some(reading) => close_gauges(&reading, providers, project),
        None => Vec::new(),
    }
}
