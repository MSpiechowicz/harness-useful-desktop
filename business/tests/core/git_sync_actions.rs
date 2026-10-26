//! `git_sync_action`, `git_switch` and `git_branch_search` over temporary repositories with a local
//! bare remote.

use crate::git_support;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use git_support::{AfterCommand, GitHome, Repo, TEN_LINES, diverge as diverged, world};
use harness_business::git::{git_switch, git_sync_action, git_sync_state};
use harness_business::model::{
    ActionOutcome, BranchStart, BranchSync, ConflictFile, GitActionResult, GitOperationKind, IpcErrorCode,
    SwitchRequest, SyncAction,
};
use pretty_assertions::assert_eq;

type Outcome = harness_business::error::Result<GitActionResult>;

pub(super) async fn act(home: &GitHome, repo: &Repo, action: SyncAction) -> Outcome {
    git_sync_action(&home.env, &home.runner, &repo.path, None, action).await
}

async fn switch(home: &GitHome, repo: &Repo, branch: &str, create: Option<BranchStart>, stash: bool) -> Outcome {
    let request = SwitchRequest {
        branch: branch.to_owned(),
        create,
        stash,
    };

    git_switch(&home.env, &home.runner, &repo.path, None, &request).await
}

pub(super) async fn state(home: &GitHome, repo: &Repo) -> BranchSync {
    git_sync_state(&home.env, &home.runner, &repo.path, None).await.unwrap()
}

fn read(repo: &Repo, name: &str) -> String {
    fs::read_to_string(repo.path.join(name)).unwrap()
}

#[tokio::test]
async fn publishing_sets_the_upstream_and_pushing_sends_the_commits() {
    let mut world = world();
    world.work.git(&["switch", "-q", "-c", "feat"]);
    world.work.commit_file("f.txt", "f\n");

    let (home, work) = (&world.home, &world.work);
    assert_eq!(state(home, work).await.upstream, None);

    let published = act(home, work, SyncAction::Publish).await.unwrap();
    assert_eq!(published.outcome, ActionOutcome::Done);
    assert_eq!(published.state.upstream.as_deref(), Some("origin/feat"));
    assert_eq!(published.state.ahead, 0);

    world.work.commit_file("g.txt", "g\n");
    let (home, work) = (&world.home, &world.work);
    assert_eq!(state(home, work).await.ahead, 1);

    let pushed = act(home, work, SyncAction::Push).await.unwrap();
    assert_eq!(pushed.outcome, ActionOutcome::Done);
    assert_eq!(pushed.state.ahead, 0);
    assert_eq!(
        world.work.git(&["rev-parse", "origin/feat"]),
        world.work.git(&["rev-parse", "HEAD"])
    );

    let again = act(home, work, SyncAction::Push).await.unwrap();
    assert_eq!(again.outcome, ActionOutcome::UpToDate);
}

#[tokio::test]
async fn pushing_a_branch_without_an_upstream_is_refused() {
    let world = world();
    world.work.git(&["switch", "-q", "-c", "feat"]);

    let refused = act(&world.home, &world.work, SyncAction::Push).await;

    assert_eq!(refused.unwrap_err().code(), IpcErrorCode::InvalidInput);
}

#[tokio::test]
async fn pull_fast_forwards_and_refuses_a_diverged_branch() {
    let world = world();
    let mut other = world.other();
    other.commit_file("m.txt", "m\n");
    other.git(&["push", "-q", "origin", "main"]);

    let pulled = act(&world.home, &world.work, SyncAction::Pull).await.unwrap();
    assert_eq!(pulled.outcome, ActionOutcome::Done);
    assert_eq!(pulled.state.behind, 0);
    assert!(world.work.path.join("m.txt").exists());

    let mut work = world.work;
    work.commit_file("mine.txt", "mine\n");
    other.commit_file("n.txt", "n\n");
    other.git(&["push", "-q", "origin", "main"]);
    let before = work.head();

    // Whatever `pull.rebase` says, the pull never rewrites or merges.
    work.git(&["config", "pull.rebase", "true"]);
    let refused = act(&world.home, &work, SyncAction::Pull).await.unwrap_err();

    assert_eq!(refused.code(), IpcErrorCode::Process);
    assert_eq!(work.head(), before);
}

#[tokio::test]
async fn rebasing_puts_a_tracked_change_back_and_leaves_untracked_files_alone() {
    let mut world = world();
    diverged(&mut world, ("f.txt", "f\n"), ("m.txt", "m\n")).await;
    world.work.write("b.txt", "dirty\n");
    world.work.write("loose.txt", "loose\n");

    let result = act(&world.home, &world.work, SyncAction::Rebase).await.unwrap();

    assert_eq!(result.outcome, ActionOutcome::Done);
    assert_eq!(result.state.operation, None);
    assert_eq!(result.state.main.as_ref().map(|main| main.new_commits), Some(0));
    assert_eq!(read(&world.work, "b.txt"), "dirty\n");
    assert_eq!(read(&world.work, "loose.txt"), "loose\n");
    assert!(world.work.path.join("m.txt").exists());
    assert!(world.work.path.join("f.txt").exists());
    assert!(!result.state.stash_kept);

    let again = act(&world.home, &world.work, SyncAction::Rebase).await.unwrap();
    assert_eq!(again.outcome, ActionOutcome::UpToDate);
}

#[tokio::test]
async fn aborting_a_conflicting_rebase_restores_the_head_and_the_stashed_change() {
    let mut world = world();
    diverged(&mut world, ("a.txt", "feature\n"), ("a.txt", "main\n")).await;
    world.work.write("b.txt", "dirty\n");
    let before = world.work.head();

    let stopped = act(&world.home, &world.work, SyncAction::Rebase).await.unwrap();

    assert_eq!(stopped.outcome, ActionOutcome::Stopped);
    let operation = stopped.state.operation.expect("stopped");
    assert_eq!(operation.kind, GitOperationKind::Rebase);
    assert_eq!(operation.conflicts.len(), 1);
    assert_eq!(stopped.state.unmerged, operation.conflicts);

    let refused = act(&world.home, &world.work, SyncAction::Pull).await;
    assert_eq!(refused.unwrap_err().code(), IpcErrorCode::InvalidInput);

    let still_conflicting = act(&world.home, &world.work, SyncAction::Continue).await;
    assert_eq!(still_conflicting.unwrap_err().code(), IpcErrorCode::InvalidInput);

    let aborted = act(&world.home, &world.work, SyncAction::Abort).await.unwrap();

    assert_eq!(aborted.outcome, ActionOutcome::Done);
    assert_eq!(aborted.state.operation, None);
    assert_eq!(world.work.head(), before);
    assert_eq!(read(&world.work, "b.txt"), "dirty\n");
    assert!(!aborted.state.stash_kept);
    assert_eq!(world.work.git(&["stash", "list"]), "");
}

#[tokio::test]
async fn continuing_a_rebase_once_the_conflict_is_resolved_finishes_it() {
    let mut world = world();
    diverged(&mut world, ("a.txt", "feature\n"), ("a.txt", "main\n")).await;
    act(&world.home, &world.work, SyncAction::Rebase).await.unwrap();

    world.work.resolve("a.txt", "resolved\n");

    let waiting = state(&world.home, &world.work).await;
    let operation = waiting
        .operation
        .expect("still stopped, only the continue step is left");
    assert_eq!(operation.conflicts, []);

    let finished = act(&world.home, &world.work, SyncAction::Continue).await.unwrap();

    assert_eq!(finished.outcome, ActionOutcome::Done);
    assert_eq!(finished.state.operation, None);
    assert_eq!(finished.state.branch.as_deref(), Some("feat"));
    assert_eq!(read(&world.work, "a.txt"), "resolved\n");
}

#[tokio::test]
async fn a_conflicting_merge_stops_and_can_be_aborted_or_committed() {
    let mut world = world();
    diverged(&mut world, ("a.txt", "feature\n"), ("a.txt", "main\n")).await;
    let before = world.work.head();

    let stopped = act(&world.home, &world.work, SyncAction::Merge).await.unwrap();
    assert_eq!(stopped.outcome, ActionOutcome::Stopped);
    assert_eq!(
        stopped.state.operation.as_ref().map(|operation| operation.kind),
        Some(GitOperationKind::Merge)
    );

    let aborted = act(&world.home, &world.work, SyncAction::Abort).await.unwrap();
    assert_eq!(aborted.state.operation, None);
    assert_eq!(world.work.head(), before);

    act(&world.home, &world.work, SyncAction::Merge).await.unwrap();
    world.work.resolve("a.txt", "merged\n");

    let committed = act(&world.home, &world.work, SyncAction::Continue).await.unwrap();

    assert_eq!(committed.outcome, ActionOutcome::Done);
    assert_eq!(committed.state.operation, None);
    assert_eq!(
        world
            .work
            .git(&["rev-list", "--parents", "-1", "HEAD"])
            .split(' ')
            .count(),
        3
    );
}

#[tokio::test]
async fn nothing_to_abort_or_continue_is_refused() {
    let world = world();

    for action in [SyncAction::Abort, SyncAction::Continue, SyncAction::ApplyStash] {
        let refused = act(&world.home, &world.work, action).await;
        assert_eq!(refused.unwrap_err().code(), IpcErrorCode::InvalidInput, "{action:?}");
    }

    let on_main = act(&world.home, &world.work, SyncAction::Rebase).await;
    assert_eq!(on_main.unwrap_err().code(), IpcErrorCode::InvalidInput);
}

#[tokio::test]
async fn a_new_branch_starts_at_main_or_here_without_an_upstream() {
    let mut world = world();
    let mut other = world.other();
    other.commit_file("m.txt", "m\n");
    other.git(&["push", "-q", "origin", "main"]);
    world.work.git(&["fetch", "-q"]);
    world.work.git(&["switch", "-q", "-c", "side"]);
    world.work.commit_file("s.txt", "s\n");

    let (home, work) = (&world.home, &world.work);
    let from_main = switch(home, work, "from-main", Some(BranchStart::Main), false)
        .await
        .unwrap();

    assert_eq!(from_main.outcome, ActionOutcome::Done);
    assert_eq!(from_main.state.branch.as_deref(), Some("from-main"));
    assert_eq!(from_main.state.upstream, None);
    assert_eq!(work.head(), work.git(&["rev-parse", "origin/main"]).trim());

    work.git(&["switch", "-q", "side"]);
    let from_here = switch(home, work, "from-here", Some(BranchStart::Current), false)
        .await
        .unwrap();

    assert_eq!(from_here.state.branch.as_deref(), Some("from-here"));
    assert!(work.path.join("s.txt").exists());
    assert_eq!(from_here.state.upstream, None);
}

#[tokio::test]
async fn a_remote_only_branch_is_switched_to_with_its_tracking_set() {
    let world = world();
    let mut other = world.other();
    other.git(&["switch", "-q", "-c", "remote-feat"]);
    other.commit_file("r.txt", "r\n");
    other.git(&["push", "-q", "origin", "remote-feat"]);
    world.work.git(&["fetch", "-q"]);

    let switched = switch(&world.home, &world.work, "remote-feat", None, false)
        .await
        .unwrap();

    assert_eq!(switched.state.branch.as_deref(), Some("remote-feat"));
    assert_eq!(switched.state.upstream.as_deref(), Some("origin/remote-feat"));
}

#[tokio::test]
async fn creating_from_main_without_a_main_branch_is_refused() {
    let home = git_support::git_home().with_identity();
    let mut repo = home.repo("local");
    repo.commit_file("a.txt", "a\n");
    repo.git(&["branch", "-m", "work"]);

    let refused = switch(&home, &repo, "x", Some(BranchStart::Main), false).await;

    assert_eq!(refused.unwrap_err().code(), IpcErrorCode::InvalidInput);
}

#[tokio::test]
async fn a_blocked_switch_can_be_repeated_with_a_stash_and_the_stash_applied_afterwards() {
    let mut world = world();
    world.work.git(&["switch", "-q", "-c", "other"]);
    world.work.write("a.txt", TEN_LINES.replace("line 1\n", "line one\n"));
    world.work.commit("Reword line 1");
    world.work.git(&["switch", "-q", "main"]);
    world.work.write("a.txt", TEN_LINES.replace("line 10\n", "line ten\n"));
    world.work.write("loose.txt", "loose\n");
    let (home, work) = (&world.home, &world.work);

    let blocked = switch(home, work, "other", None, false).await.unwrap();

    assert_eq!(blocked.outcome, ActionOutcome::BlockedByChanges);
    assert_eq!(blocked.state.branch.as_deref(), Some("main"));
    assert_eq!(read(work, "loose.txt"), "loose\n");

    let stashed = switch(home, work, "other", None, true).await.unwrap();

    assert_eq!(stashed.outcome, ActionOutcome::StashKept);
    assert_eq!(stashed.state.branch.as_deref(), Some("other"));
    assert!(stashed.state.stash_kept);
    assert_eq!((stashed.state.tracked_changes, stashed.state.untracked), (0, 0));

    let applied = act(home, work, SyncAction::ApplyStash).await.unwrap();

    assert_eq!(applied.outcome, ActionOutcome::Done);
    assert!(!applied.state.stash_kept);
    assert_eq!(read(work, "loose.txt"), "loose\n");
    assert!(read(work, "a.txt").contains("line ten"));
    assert!(read(work, "a.txt").contains("line one"));
}

#[tokio::test]
async fn a_stash_the_user_made_is_never_applied_by_the_app() {
    let world = world();
    world.work.write("a.txt", "mine\n");
    world.work.git(&["stash", "push", "-m", "my own work"]);

    assert!(!state(&world.home, &world.work).await.stash_kept);

    let refused = act(&world.home, &world.work, SyncAction::ApplyStash).await;

    assert_eq!(refused.unwrap_err().code(), IpcErrorCode::InvalidInput);
    assert!(world.work.git(&["stash", "list"]).contains("my own work"));
}

/// A world whose work tree has the app's stash (a change to `a.txt`) at the top of the list, and
/// a committed `c.txt` that a stash made by someone else will change.
fn world_with_our_stash() -> git_support::World {
    let mut world = world();
    world.work.commit_file("c.txt", "c\n");
    world.work.write("a.txt", "ours\n");
    world
        .work
        .git(&["stash", "push", "-m", "Harness Desktop: before switching to x"]);

    world
}

/// Runs `ApplyStash` while another program pushes a stash of its own (`c.txt` changed) right after
/// the `nth` listing of the stash. The flag tells whether that happened.
async fn apply_with_intruder(world: &git_support::World, nth: usize) -> (Outcome, Arc<AtomicBool>) {
    let fired = Arc::new(AtomicBool::new(false));
    let (other, flag) = (world.work.clone(), Arc::clone(&fired));
    let runner = AfterCommand::new(&["stash", "list"], nth, move || {
        other.write("c.txt", "theirs\n");
        other.git(&["stash", "push", "-m", "someone else", "--", "c.txt"]);
        flag.store(true, Ordering::SeqCst);
    });

    let result = git_sync_action(&world.home.env, &runner, &world.work.path, None, SyncAction::ApplyStash).await;

    (result, fired)
}

/// The stash list is exactly the entry of someone else, with its content intact.
fn assert_only_the_intruders_stash_is_left(repo: &Repo) {
    let list = repo.git(&["stash", "list"]);

    assert_eq!(list.lines().count(), 1, "{list}");
    assert!(list.contains("someone else"), "{list}");
    assert_eq!(repo.git(&["show", "stash@{0}:c.txt"]), "theirs\n");
}

#[tokio::test]
async fn a_stash_made_after_the_check_is_not_applied_in_place_of_the_checked_one() {
    let world = world_with_our_stash();
    assert!(state(&world.home, &world.work).await.stash_kept);

    // The state read and the ownership check each list the newest stash: the other stash appears
    // right after the second listing.
    let (result, fired) = apply_with_intruder(&world, 2).await;

    assert!(fired.load(Ordering::SeqCst));
    assert_eq!(result.unwrap().outcome, ActionOutcome::Done);
    assert_eq!(read(&world.work, "a.txt"), "ours\n");
    assert_eq!(read(&world.work, "c.txt"), "c\n", "their stash was not applied");
    assert_only_the_intruders_stash_is_left(&world.work);
}

#[tokio::test]
async fn a_stash_made_right_before_the_drop_is_not_dropped_in_place_of_ours() {
    let world = world_with_our_stash();

    // The third listing is the one the drop looks its entry up in.
    let (result, fired) = apply_with_intruder(&world, 3).await;

    assert!(fired.load(Ordering::SeqCst));
    assert_eq!(result.unwrap().outcome, ActionOutcome::Done);
    assert_eq!(read(&world.work, "a.txt"), "ours\n");
    assert_only_the_intruders_stash_is_left(&world.work);
}

#[tokio::test]
async fn an_autostash_that_does_not_apply_cleanly_stays_in_the_stash() {
    let mut world = world();
    diverged(
        &mut world,
        ("f.txt", "f\n"),
        ("a.txt", &TEN_LINES.replace("line 5\n", "line five from main\n")),
    )
    .await;
    world
        .work
        .write("a.txt", TEN_LINES.replace("line 5\n", "line five from me\n"));

    let result = act(&world.home, &world.work, SyncAction::Rebase).await.unwrap();

    assert_eq!(result.outcome, ActionOutcome::StashKept);
    assert!(result.state.stash_kept);
    assert_eq!(result.state.operation, None);
    assert!(result.state.stash_applied);
    assert!(world.work.git(&["stash", "list"]).contains("applied with conflicts"));

    // Git left the conflict in the files with no rebase to show it: it is reported all the same,
    // and a pop of the stash would only pile on.
    assert_eq!(
        result.state.unmerged,
        [ConflictFile {
            path: "a.txt".to_owned(),
            in_project: true,
        }]
    );
    let refused = act(&world.home, &world.work, SyncAction::ApplyStash).await.unwrap_err();
    assert_eq!(refused.code(), IpcErrorCode::InvalidInput);
    assert_eq!(world.work.git(&["stash", "list"]).lines().count(), 1);
}

/// Installs a hook that records its name in `marker`, then runs `tail`.
#[cfg(unix)]
fn install_hook(repo: &Repo, name: &str, marker: &std::path::Path, tail: &str) {
    repo.install_hook(name, &format!("echo {name} >> '{}'\n{tail}", marker.display()));
}

#[cfg(unix)]
#[tokio::test]
async fn the_repositorys_hooks_run_for_a_clicked_push_and_a_failing_one_blocks_it() {
    let mut world = world();
    world.work.git(&["switch", "-q", "-c", "feat"]);
    world.work.commit_file("f.txt", "f\n");
    let marker = world.home.home.join("hook.marker");
    install_hook(&world.work, "pre-push", &marker, "exit 0");
    let (home, work) = (&world.home, &world.work);

    act(home, work, SyncAction::Publish).await.unwrap();
    assert_eq!(fs::read_to_string(&marker).unwrap(), "pre-push\n");

    world.work.commit_file("g.txt", "g\n");
    install_hook(
        &world.work,
        "pre-push",
        &marker,
        "echo 'error: lint failed' >&2\nexit 1",
    );
    let (home, work) = (&world.home, &world.work);

    let refused = act(home, work, SyncAction::Push).await.unwrap_err();

    assert_eq!(refused.code(), IpcErrorCode::Process);
    assert_eq!(state(home, work).await.ahead, 1);
}

#[cfg(unix)]
#[tokio::test]
async fn a_hook_that_never_ends_is_cut_off_at_the_time_limit() {
    use harness_business::git::{GitCli, RunOpts};
    use std::time::{Duration, Instant};

    let mut world = world();
    world.work.commit_file("f.txt", "f\n");
    let marker = world.home.home.join("sleepy.marker");
    install_hook(&world.work, "pre-push", &marker, "sleep 60");

    let git = GitCli::new(&world.home.env, &world.home.runner, &world.work.path).unwrap();
    let started = Instant::now();
    let result = git
        .run_with(
            ["push", "origin", "main"],
            &RunOpts::timeout(Duration::from_millis(800)),
        )
        .await;

    assert_eq!(result.unwrap_err().code(), IpcErrorCode::Timeout);
    assert!(started.elapsed() < Duration::from_secs(20));
    assert!(marker.exists(), "the hook did start");
}
