//! Branch sync commands: where the current branch stands against its remote and the main branch,
//! and the actions that move it (fetch, push, pull, rebase, merge, switch).
//!
//! The project must be the active project or a recent one. Everything that changes the repository
//! or reaches the network takes the repository's write lock (keyed by its common git directory, so
//! work trees and subdirectory projects of one repository share it), so one such command runs at a
//! time.
//! git runs through the business layer's hardened runner; the main branch the user picked comes
//! from the preferences, which only `git_set_main_branch` changes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use harness_business::git;
use harness_business::model::{BranchPage, BranchSync, GitActionResult, SwitchRequest, SyncAction};
use tauri::State;
use tauri::async_runtime::{self, block_on};

use super::{authorized_dir, blocking};
use crate::error::CommandResult;
use crate::state::AppState;

/// Branch, upstream, ahead/behind, main branch and any stopped rebase or merge of `project`.
/// Local only: never reaches the network.
#[tauri::command]
pub async fn git_sync_state(state: State<'_, AppState>, project: String) -> CommandResult<BranchSync> {
    sync_state(&state, project).await
}

/// Fetches the remotes of `project`. A `background` fetch skips the network (and returns the
/// current state) while another git write runs, runs hardened (no hooks, no prompts, no `ext::`
/// transport, no submodules) and is skipped, returning the unchanged state, while the repository's
/// own configuration names a program.
#[tauri::command]
pub async fn git_fetch(state: State<'_, AppState>, project: String, background: bool) -> CommandResult<BranchSync> {
    fetch(&state, project, background).await
}

/// Runs one sync `action` (push, publish, pull, rebase, merge, abort, continue, apply stash).
#[tauri::command]
pub async fn git_sync_action(
    state: State<'_, AppState>,
    project: String,
    action: SyncAction,
) -> CommandResult<GitActionResult> {
    sync_action(&state, project, action).await
}

/// Switches to a branch, or creates one (from main or the current branch) and switches to it,
/// optionally stashing the working-tree changes first.
#[tauri::command]
pub async fn git_switch(
    state: State<'_, AppState>,
    project: String,
    request: SwitchRequest,
) -> CommandResult<GitActionResult> {
    switch(&state, project, request).await
}

/// At most `limit` (1 to 100) branches whose name contains `query`, local ones first.
#[tauri::command]
pub async fn git_branch_search(
    state: State<'_, AppState>,
    project: String,
    query: String,
    limit: u32,
) -> CommandResult<BranchPage> {
    branch_search(&state, project, query, limit).await
}

/// Picks `branch` as the main branch of `project`, or clears the pick with `None`.
#[tauri::command]
pub async fn git_set_main_branch(
    state: State<'_, AppState>,
    project: String,
    branch: Option<String>,
) -> CommandResult<BranchSync> {
    set_main_branch(&state, project, branch).await
}

/// An authorized project: its canonical directory and the main branch picked for it.
struct Target {
    dir: PathBuf,
    picked: Option<String>,
}

async fn target(state: &AppState, project: String) -> CommandResult<Target> {
    let dir = authorized_dir(state, project.clone()).await?;
    let prefs = state.prefs.clone();
    let picked =
        blocking(move || Ok(prefs.with(|store| store.load().picked_main_branch(&project).map(str::to_owned)))).await?;

    Ok(Target { dir, picked })
}

/// Runs a git call in a blocking task: the repository state is read from files.
async fn on_blocking<T, F>(state: &AppState, task: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: AsyncFnOnce(&harness_business::env::Env, &harness_business::process::TokioRunner) -> harness_business::Result<T>
        + Send
        + 'static,
{
    let env = state.env.clone();
    let runner = state.runner;

    blocking(move || Ok(block_on(task(&env, &runner))?)).await
}

async fn sync_state(state: &AppState, project: String) -> CommandResult<BranchSync> {
    let target = target(state, project).await?;

    state_of(state, &target.dir, target.picked).await
}

async fn state_of(state: &AppState, dir: &Path, picked: Option<String>) -> CommandResult<BranchSync> {
    let dir = dir.to_path_buf();

    on_blocking(state, async move |env, runner| {
        git::git_sync_state(env, runner, &dir, picked.as_deref()).await
    })
    .await
}

/// The write lock of the repository around `dir` (of `dir` itself when it is in none).
async fn write_lock(state: &AppState, dir: &Path) -> Arc<async_runtime::Mutex<()>> {
    let project = dir.to_path_buf();
    let common = on_blocking(state, async move |env, runner| {
        Ok(git::git_common_dir(env, runner, &project).await)
    })
    .await
    .ok()
    .flatten();

    state.git_writes.of(&common.unwrap_or_else(|| dir.to_path_buf()))
}

async fn fetch(state: &AppState, project: String, background: bool) -> CommandResult<BranchSync> {
    let target = target(state, project).await?;
    let lock = write_lock(state, &target.dir).await;

    let _held = if background {
        match lock.try_lock() {
            Ok(held) => held,
            Err(_busy) => return state_of(state, &target.dir, target.picked).await,
        }
    } else {
        lock.lock().await
    };

    let Target { dir, picked } = target;

    on_blocking(state, async move |env, runner| {
        git::git_fetch(env, runner, &dir, picked.as_deref(), background).await
    })
    .await
}

async fn sync_action(state: &AppState, project: String, action: SyncAction) -> CommandResult<GitActionResult> {
    write_locked(state, project, async move |env, runner, dir, picked| {
        git::git_sync_action(env, runner, dir, picked, action).await
    })
    .await
}

async fn switch(state: &AppState, project: String, request: SwitchRequest) -> CommandResult<GitActionResult> {
    write_locked(state, project, async move |env, runner, dir, picked| {
        git::git_switch(env, runner, dir, picked, &request).await
    })
    .await
}

/// Runs a git call that changes the repository of `project` while holding its write lock.
async fn write_locked<T, F>(state: &AppState, project: String, task: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: AsyncFnOnce(
            &harness_business::env::Env,
            &harness_business::process::TokioRunner,
            &Path,
            Option<&str>,
        ) -> harness_business::Result<T>
        + Send
        + 'static,
{
    let Target { dir, picked } = target(state, project).await?;
    let lock = write_lock(state, &dir).await;
    let _held = lock.lock().await;

    on_blocking(state, async move |env, runner| {
        task(env, runner, &dir, picked.as_deref()).await
    })
    .await
}

async fn branch_search(state: &AppState, project: String, query: String, limit: u32) -> CommandResult<BranchPage> {
    let Target { dir, .. } = target(state, project).await?;

    on_blocking(state, async move |env, runner| {
        git::git_branch_search(env, runner, &dir, &query, limit).await
    })
    .await
}

async fn set_main_branch(state: &AppState, project: String, branch: Option<String>) -> CommandResult<BranchSync> {
    let dir = authorized_dir(state, project.clone()).await?;
    let prefs = state.prefs.clone();

    let saved = blocking(move || {
        Ok(prefs.with(|store| {
            store
                .set_main_branch(Path::new(&project), branch.as_deref())
                .map(|saved| (project, saved))
        })?)
    })
    .await?;
    let (project, saved) = saved;
    let picked = saved.picked_main_branch(&project).map(str::to_owned);

    state_of(state, &dir, picked).await
}

#[cfg(test)]
mod tests {
    use harness_business::model::{BranchStart, IpcErrorCode, MainSource};

    use super::*;
    use crate::test_support::{TempDir, open_git_project, setup_git, stranger_beside};

    struct Setup {
        state: AppState,
        project: String,
        origin: PathBuf,
        _dir: TempDir,
    }

    /// A setup git command in `dir`, which sits under the empty home.
    fn git(dir: &Path, args: &[&str]) {
        let home = dir
            .ancestors()
            .find(|path| path.ends_with("home"))
            .expect("under a home");
        setup_git(home, dir, args);
    }

    /// An opened project cloned from a bare `origin` with one commit on `main`, plus a local `dev`
    /// branch.
    fn setup() -> Setup {
        let dir = TempDir::new("git-sync");
        let home = dir.path().join("home");
        let origin = home.join("origin.git");
        let seed = home.join("seed");
        let project = home.join("demo");
        std::fs::create_dir_all(&origin).expect("create origin");
        std::fs::create_dir_all(&seed).expect("create seed");

        git(&origin, &["init", "-q", "--bare", "-b", "main"]);
        git(&seed, &["init", "-q", "-b", "main"]);
        std::fs::write(seed.join("a.txt"), "one\n").expect("write");
        git(&seed, &["add", "a.txt"]);
        git(&seed, &["commit", "-q", "-m", "base"]);
        git(&seed, &["push", "-q", origin.to_str().unwrap(), "main"]);
        git(
            &home,
            &["clone", "-q", origin.to_str().unwrap(), project.to_str().unwrap()],
        );
        git(&project, &["branch", "dev"]);

        let (state, project) = open_git_project(&dir, &home, &project);

        Setup {
            state,
            project,
            origin,
            _dir: dir,
        }
    }

    fn create(branch: &str) -> SwitchRequest {
        SwitchRequest {
            branch: branch.to_owned(),
            create: Some(BranchStart::Current),
            stash: false,
        }
    }

    #[test]
    fn unknown_projects_are_refused_by_every_command() {
        let setup = setup();
        let state = &setup.state;
        let stranger = stranger_beside(&setup.project);

        let refused = [
            block_on(sync_state(state, stranger.clone())).map(drop),
            block_on(fetch(state, stranger.clone(), true)).map(drop),
            block_on(fetch(state, stranger.clone(), false)).map(drop),
            block_on(sync_action(state, stranger.clone(), SyncAction::Push)).map(drop),
            block_on(switch(state, stranger.clone(), create("topic"))).map(drop),
            block_on(branch_search(state, stranger.clone(), String::new(), 10)).map(drop),
            block_on(set_main_branch(state, stranger, Some("dev".to_owned()))).map(drop),
        ];
        for error in refused {
            assert_eq!(error.expect_err("unknown project").code(), IpcErrorCode::Forbidden);
        }
    }

    #[test]
    fn bad_branch_names_are_refused() {
        let setup = setup();
        let (state, project) = (&setup.state, &setup.project);

        for name in ["a..b", "-x", "", "with space", "x.lock"] {
            let error = block_on(switch(state, project.clone(), create(name))).expect_err(name);
            assert_eq!(error.code(), IpcErrorCode::InvalidInput, "switch {name:?}");

            let error = block_on(set_main_branch(state, project.clone(), Some(name.to_owned()))).expect_err(name);
            assert_eq!(error.code(), IpcErrorCode::InvalidInput, "main {name:?}");
        }
    }

    #[test]
    fn the_picked_main_branch_persists_and_only_this_command_changes_it() {
        let setup = setup();
        let (state, project) = (&setup.state, &setup.project);

        let before = block_on(sync_state(state, project.clone())).expect("state");
        assert_eq!(before.main.as_ref().map(|main| main.source), Some(MainSource::Remote));

        let picked = block_on(set_main_branch(state, project.clone(), Some("dev".to_owned()))).expect("pick");
        let main = picked.main.expect("main");
        assert_eq!((main.name.as_str(), main.source), ("dev", MainSource::Picked));

        let stored = state.prefs.with(|store| store.load());
        assert_eq!(stored.picked_main_branch(project), Some("dev"));

        // A prefs payload from the webview cannot change the pick, even with it emptied.
        let mut sent = stored.clone();
        sent.main_branches.clear();
        let saved = state.prefs.with(|store| store.save_ui(&sent)).expect("save");
        assert_eq!(saved.picked_main_branch(project), Some("dev"));

        let reloaded = block_on(sync_state(state, project.clone())).expect("state");
        assert_eq!(reloaded.main.map(|main| main.source), Some(MainSource::Picked));

        let cleared = block_on(set_main_branch(state, project.clone(), None)).expect("clear");
        assert_ne!(cleared.main.map(|main| main.source), Some(MainSource::Picked));
        assert_eq!(state.prefs.with(|store| store.load()).picked_main_branch(project), None);
    }

    #[test]
    fn a_background_fetch_skips_the_network_while_a_git_write_runs() {
        let setup = setup();
        let (state, project) = (&setup.state, &setup.project);
        let home = Path::new(project).parent().unwrap();
        let pusher = home.join("pusher");
        git(
            home,
            &["clone", "-q", setup.origin.to_str().unwrap(), pusher.to_str().unwrap()],
        );
        std::fs::write(pusher.join("b.txt"), "two\n").expect("write");
        git(&pusher, &["add", "b.txt"]);
        git(&pusher, &["commit", "-q", "-m", "more"]);
        git(&pusher, &["push", "-q"]);

        let dir = block_on(authorized_dir(state, project.clone())).expect("authorized");
        let held = block_on(write_lock(state, &dir)).try_lock_owned().expect("free");

        let busy = block_on(fetch(state, project.clone(), true)).expect("state while busy");
        assert_eq!(busy.behind, 0, "no fetch while the lock is held");

        drop(held);
        let fetched = block_on(fetch(state, project.clone(), true)).expect("fetch");
        assert_eq!(fetched.behind, 1);
    }

    #[test]
    fn work_trees_and_subdirectories_of_one_repository_share_one_write_lock() {
        let setup = setup();
        let state = &setup.state;
        let project = PathBuf::from(&setup.project);
        let home = project.parent().unwrap().to_path_buf();

        let worktree = home.join("linked");
        git(
            &project,
            &["worktree", "add", "-q", worktree.to_str().unwrap(), "-b", "wt"],
        );
        let subdirectory = project.join("sub");
        std::fs::create_dir_all(&subdirectory).expect("subdirectory");
        let plain = home.join("plain");
        std::fs::create_dir_all(&plain).expect("plain folder");

        let [root, linked, sub, other] =
            [&project, &worktree, &subdirectory, &plain].map(|dir| block_on(write_lock(state, dir)));

        assert!(Arc::ptr_eq(&root, &linked), "a linked work tree shares the lock");
        assert!(Arc::ptr_eq(&root, &sub), "a subdirectory shares the lock");
        assert!(!Arc::ptr_eq(&root, &other), "a folder in no repository has its own");
        assert!(Arc::ptr_eq(&other, &block_on(write_lock(state, &plain))));
    }

    #[test]
    fn a_switch_creates_and_searches_branches() {
        let setup = setup();
        let (state, project) = (&setup.state, &setup.project);

        let result = block_on(switch(state, project.clone(), create("topic"))).expect("switch");
        assert_eq!(result.state.branch.as_deref(), Some("topic"));

        let page = block_on(branch_search(state, project.clone(), "dev".to_owned(), 10)).expect("search");
        let names: Vec<&str> = page.branches.iter().map(|branch| branch.name.as_str()).collect();
        assert_eq!(names, ["dev"]);
    }
}
