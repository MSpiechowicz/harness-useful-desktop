//! How failing writes are reported: redacted, never mistaken for a stop, and without losing the
//! user's stashed changes.

use crate::git_support;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use git_support::{AfterCommand, GitHome, Repo, diverge, world};
use harness_business::error::{BusinessError, Result};
use harness_business::git::{git_switch, git_sync_action, git_sync_state};
use harness_business::model::{
    ActionOutcome, BranchSync, GitActionResult, GitOperationKind, IpcErrorCode, SwitchRequest, SyncAction,
};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};
use pretty_assertions::assert_eq;

async fn act(home: &GitHome, repo: &Repo, action: SyncAction) -> Result<GitActionResult> {
    git_sync_action(&home.env, &home.runner, &repo.path, None, action).await
}

async fn state(home: &GitHome, repo: &Repo) -> BranchSync {
    git_sync_state(&home.env, &home.runner, &repo.path, None).await.unwrap()
}

/// A token-shaped secret, built at run time: a literal in the source would trip secret scanners.
fn token() -> String {
    ["gh", "p_", "0123456789abcdefghijklmnopqrstuvwxyzAB"].concat()
}

/// A hook that rejects with a message carrying a remote URL with credentials and the bare token.
#[cfg(unix)]
fn leaky_rejection(repo: &Repo, hook: &str, token: &str) {
    repo.install_hook(
        hook,
        &format!(
            "echo 'error: blocked, see https://alice:{token}@git.example.invalid/team/project.git token {token}' >&2\nexit 1"
        ),
    );
}

/// `work` on `feat`, a merge of main stopped on a conflict in `a.txt` that is resolved and staged.
async fn resolved_merge() -> git_support::World {
    let mut world = world();
    diverge(&mut world, ("a.txt", "feature\n"), ("a.txt", "main\n")).await;
    act(&world.home, &world.work, SyncAction::Merge).await.unwrap();
    world.work.resolve("a.txt", "merged\n");
    world
}

#[cfg(unix)]
#[tokio::test]
async fn a_hook_that_rejects_a_write_is_reported_without_credentials() {
    let token = token();

    let mut world = world();
    diverge(&mut world, ("f.txt", "f\n"), ("m.txt", "m\n")).await;
    leaky_rejection(&world.work, "pre-rebase", &token);
    let rebase = act(&world.home, &world.work, SyncAction::Rebase).await.unwrap_err();

    let world = resolved_merge().await;
    leaky_rejection(&world.work, "pre-commit", &token);
    let commit = act(&world.home, &world.work, SyncAction::Continue).await.unwrap_err();

    for error in [rebase, commit] {
        assert_eq!(error.code(), IpcErrorCode::Process);

        let text = error.to_string();
        assert!(text.contains("git.example.invalid"), "the host stays: {text}");
        assert!(!text.contains(&token), "{text}");
        assert!(!text.contains("alice"), "{text}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_hook_that_rejects_the_merge_commit_is_an_error_and_the_merge_stays_open() {
    let world = resolved_merge().await;
    world
        .work
        .install_hook("pre-commit", "echo 'error: lint failed' >&2\nexit 1");

    let rejected = act(&world.home, &world.work, SyncAction::Continue).await.unwrap_err();

    assert_eq!(rejected.code(), IpcErrorCode::Process);
    assert!(rejected.to_string().contains("lint failed"), "{rejected}");

    let still = state(&world.home, &world.work).await;
    assert_eq!(
        still.operation.map(|operation| operation.kind),
        Some(GitOperationKind::Merge)
    );

    fs::remove_file(world.work.path.join(".git/hooks/pre-commit")).unwrap();
    let committed = act(&world.home, &world.work, SyncAction::Continue).await.unwrap();
    assert_eq!(committed.outcome, ActionOutcome::Done);
}

#[tokio::test]
async fn an_abort_that_fails_is_an_error_not_a_stop() {
    let mut world = world();
    diverge(&mut world, ("a.txt", "feature\n"), ("a.txt", "main\n")).await;
    act(&world.home, &world.work, SyncAction::Rebase).await.unwrap();
    let lock = world.work.path.join(".git/index.lock");
    fs::write(&lock, "").unwrap();

    let failed = act(&world.home, &world.work, SyncAction::Abort).await.unwrap_err();

    assert_eq!(failed.code(), IpcErrorCode::Process);
    assert!(failed.to_string().contains("File exists"), "{failed}");

    fs::remove_file(&lock).unwrap();
    let aborted = act(&world.home, &world.work, SyncAction::Abort).await.unwrap();
    assert_eq!(aborted.state.operation, None);
}

#[tokio::test]
async fn continuing_a_rebase_that_reaches_the_next_conflict_stops_again() {
    let mut world = world();
    let mut other = world.other();
    other.commit_file("a.txt", "main\n");
    other.git(&["push", "-q", "origin", "main"]);
    world.work.git(&["switch", "-q", "-c", "feat"]);
    world.work.commit_file("a.txt", "feature one\n");
    world.work.commit_file("a.txt", "feature two\n");
    world.work.git(&["fetch", "-q"]);

    let first = act(&world.home, &world.work, SyncAction::Rebase).await.unwrap();
    assert_eq!(first.outcome, ActionOutcome::Stopped);

    world.work.resolve("a.txt", "resolved one\n");
    let second = act(&world.home, &world.work, SyncAction::Continue).await.unwrap();

    assert_eq!(second.outcome, ActionOutcome::Stopped);
    let operation = second.state.operation.expect("stopped at the second commit");
    assert_eq!(operation.conflicts.len(), 1);
}

#[tokio::test]
async fn continuing_a_rebase_that_git_refuses_is_an_error_and_the_stop_stays() {
    let mut world = world();
    diverge(&mut world, ("a.txt", "feature\n"), ("a.txt", "main\n")).await;
    act(&world.home, &world.work, SyncAction::Rebase).await.unwrap();
    world.work.resolve("a.txt", "resolved\n");
    // Another git process holds the index: the continued pick cannot be committed.
    let lock = world.work.path.join(".git/index.lock");
    fs::write(&lock, "").unwrap();

    let refused = act(&world.home, &world.work, SyncAction::Continue).await.unwrap_err();
    fs::remove_file(&lock).unwrap();

    assert_eq!(refused.code(), IpcErrorCode::Process);
    let still = state(&world.home, &world.work).await;
    assert_eq!(
        still.operation.map(|operation| operation.kind),
        Some(GitOperationKind::Rebase)
    );
}

/// Runs every command for real, except the `switch`: that one runs `before` and then fails as if
/// git had been killed.
struct KilledSwitch {
    real: Box<dyn ProcessRunner>,
    before: Box<dyn Fn() + Send + Sync>,
}

impl ProcessRunner for KilledSwitch {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        Box::pin(async move {
            if spec.args.iter().any(|arg| arg == "switch") {
                (self.before)();

                return Err(BusinessError::Timeout("git did not finish within 120 s".to_owned()));
            }

            self.real.run(spec).await
        })
    }
}

fn stash_request() -> SwitchRequest {
    SwitchRequest {
        branch: "dev".to_owned(),
        create: Some(harness_business::model::BranchStart::Current),
        stash: true,
    }
}

fn list(repo: &Repo) -> String {
    repo.git(&["stash", "list"])
}

#[tokio::test]
async fn a_switch_that_fails_after_the_stash_puts_the_changes_back() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    work.write("a.txt", "mine\n");
    work.write("loose.txt", "loose\n");
    let runner = KilledSwitch {
        real: Box::new(harness_business::process::TokioRunner),
        before: Box::new(|| ()),
    };

    let failed = git_switch(&home.env, &runner, &work.path, None, &stash_request()).await;

    assert_eq!(failed.unwrap_err().code(), IpcErrorCode::Timeout);
    assert_eq!(fs::read_to_string(work.path.join("a.txt")).unwrap(), "mine\n");
    assert_eq!(fs::read_to_string(work.path.join("loose.txt")).unwrap(), "loose\n");
    assert_eq!(list(work), "");
}

/// The stash someone else pushes (`c.txt` changed), then marks that it happened.
fn push_intruders_stash(repo: &Repo, fired: &AtomicBool) {
    repo.write("c.txt", "theirs\n");
    repo.git(&["stash", "push", "-m", "someone else", "--", "c.txt"]);
    fired.store(true, Ordering::SeqCst);
}

/// A stash-and-switch that fails, with `make_runner` building the runner around the work tree
/// (it gets the repository and the flag [`push_intruders_stash`] sets). Our change must be back in
/// the tree, the intruder's `c.txt` change must still sit in the stash and nothing else.
async fn failed_switch_with_intruder(make_runner: impl FnOnce(Arc<Repo>, Arc<AtomicBool>) -> KilledSwitch) {
    let mut world = world();
    world.work.commit_file("c.txt", "c\n");
    let (home, work_path) = (&world.home, world.work.path.clone());
    let work = Arc::new(world.work);
    work.write("a.txt", "mine\n");

    let fired = Arc::new(AtomicBool::new(false));
    let runner = make_runner(Arc::clone(&work), Arc::clone(&fired));

    let failed = git_switch(&home.env, &runner, &work_path, None, &stash_request())
        .await
        .unwrap_err();

    assert_eq!(failed.code(), IpcErrorCode::Timeout);
    assert!(fired.load(Ordering::SeqCst), "the other stash was never pushed");
    assert_eq!(fs::read_to_string(work_path.join("a.txt")).unwrap(), "mine\n");
    assert_eq!(fs::read_to_string(work_path.join("c.txt")).unwrap(), "c\n");

    let stashes = list(&work);
    assert_eq!(stashes.lines().count(), 1, "{stashes}");
    assert!(stashes.contains("someone else"), "{stashes}");
    assert_eq!(work.git(&["show", "stash@{0}:c.txt"]), "theirs\n");
}

#[tokio::test]
async fn a_stash_someone_else_made_meanwhile_is_not_applied_in_place_of_ours() {
    failed_switch_with_intruder(|work, fired| KilledSwitch {
        real: Box::new(harness_business::process::TokioRunner),
        before: Box::new(move || push_intruders_stash(&work, &fired)),
    })
    .await;
}

#[tokio::test]
async fn a_stash_made_right_before_the_restore_drop_is_not_dropped_in_place_of_ours() {
    // The first listing is the state read before the switch, the second the drop's lookup.
    failed_switch_with_intruder(|work, fired| KilledSwitch {
        real: Box::new(AfterCommand::new(&["stash", "list"], 2, move || {
            push_intruders_stash(&work, &fired);
        })),
        before: Box::new(|| ()),
    })
    .await;
}
