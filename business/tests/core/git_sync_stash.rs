//! The stash an action leaves when re-applying it ended in conflicts: it is marked as applied, and
//! then only dropped, never applied a second time.

use super::git_sync_actions::{act, state};
use crate::git_support::{self, Repo, TEN_LINES, diverge as diverged, world};
use harness_business::model::{ActionOutcome, IpcErrorCode, SyncAction};
use pretty_assertions::assert_eq;

const APPLIED: &str = "Harness Desktop: applied with conflicts";

fn list(repo: &Repo) -> Vec<String> {
    repo.git(&["stash", "list"]).lines().map(str::to_owned).collect()
}

/// A foreign stash (of `b.txt`), then a rebase whose autostash conflicts: two entries, ours on top.
async fn conflicting_autostash() -> git_support::World {
    let mut world = world();
    world.work.write("b.txt", "theirs\n");
    world.work.git(&["stash", "push", "-m", "someone else"]);
    diverged(
        &mut world,
        ("f.txt", "f\n"),
        ("a.txt", &TEN_LINES.replace("line 5\n", "line five from main\n")),
    )
    .await;
    world
        .work
        .write("a.txt", TEN_LINES.replace("line 5\n", "line five from me\n"));

    world
}

#[tokio::test]
async fn a_conflicting_autostash_is_marked_applied_and_then_only_dropped() {
    let world = conflicting_autostash().await;
    let (home, work) = (&world.home, &world.work);

    let result = act(home, work, SyncAction::Rebase).await.unwrap();

    assert_eq!(result.outcome, ActionOutcome::StashKept);
    assert!(result.state.stash_kept);
    assert!(result.state.stash_applied);
    let entries = list(work);
    assert_eq!(entries.len(), 2, "{entries:?}");
    assert!(entries[0].ends_with(APPLIED), "{entries:?}");
    assert!(entries[1].contains("someone else"), "{entries:?}");

    // The conflicts are still in the way.
    let waiting = act(home, work, SyncAction::DropStash).await.unwrap_err();
    assert_eq!(waiting.code(), IpcErrorCode::InvalidInput);

    work.resolve("a.txt", "resolved\n");
    let resolved = state(home, work).await;
    assert_eq!(resolved.unmerged, []);
    assert!(resolved.stash_kept && resolved.stash_applied);

    let applied_again = act(home, work, SyncAction::ApplyStash).await.unwrap_err();
    assert_eq!(applied_again.code(), IpcErrorCode::InvalidInput);
    assert_eq!(list(work), entries);

    let dropped = act(home, work, SyncAction::DropStash).await.unwrap();

    assert_eq!(dropped.outcome, ActionOutcome::Done);
    assert!(!dropped.state.stash_kept && !dropped.state.stash_applied);
    let left = list(work);
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(left[0].contains("someone else"), "{left:?}");
    assert_eq!(work.git(&["show", "stash@{0}:b.txt"]), "theirs\n");
}

#[tokio::test]
async fn an_apply_that_ends_in_conflicts_marks_the_stash_applied() {
    let mut world = world();
    world.work.write("a.txt", "ours\n");
    world
        .work
        .git(&["stash", "push", "-m", "Harness Desktop: before switching to x"]);
    world.work.commit_file("a.txt", "committed meanwhile\n");
    let (home, work) = (&world.home, &world.work);
    assert!(state(home, work).await.stash_kept);

    let result = act(home, work, SyncAction::ApplyStash).await.unwrap();

    assert_eq!(result.outcome, ActionOutcome::StashKept);
    assert!(result.state.stash_applied);
    assert_eq!(result.state.unmerged.len(), 1);
    let entries = list(work);
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert!(entries[0].ends_with(APPLIED), "{entries:?}");

    work.resolve("a.txt", "resolved\n");

    let refused = act(home, work, SyncAction::ApplyStash).await.unwrap_err();
    assert_eq!(refused.code(), IpcErrorCode::InvalidInput);

    let dropped = act(home, work, SyncAction::DropStash).await.unwrap();

    assert!(!dropped.state.stash_kept);
    assert_eq!(list(work), Vec::<String>::new());
}

#[tokio::test]
async fn a_stash_that_is_not_an_applied_copy_is_never_dropped() {
    let world = world();
    let (home, work) = (&world.home, &world.work);
    work.write("a.txt", "mine\n");
    work.git(&["stash", "push", "-m", "my own work"]);

    let foreign = act(home, work, SyncAction::DropStash).await.unwrap_err();
    assert_eq!(foreign.code(), IpcErrorCode::InvalidInput);

    work.write("a.txt", "ours\n");
    work.git(&["stash", "push", "-m", "Harness Desktop: before switching to x"]);

    let not_applied = act(home, work, SyncAction::DropStash).await.unwrap_err();
    assert_eq!(not_applied.code(), IpcErrorCode::InvalidInput);
    assert_eq!(list(work).len(), 2);
}
