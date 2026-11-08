//! Copying the team's notes and scripts to this computer under "Remote + this computer" through
//! `harness-memory pull --json`, against the stand-in of `fake_agent` (see
//! `src/bin/fake_agent/memory_pull.rs`): the review lists each team entry by its team id and says
//! which can be ticked, the ticked team ids reach stdin one per line, `all` is the review's own
//! entries to add, a pick that is not one of them is refused with the review kept, no delete,
//! overwrite or scope flag is ever passed, and scripts are read by name.

use std::time::Duration;

use harness_business::ecosystem::{CopyPicks, NotesCopyCache};
use harness_business::model::{
    LocalNoteIds, NoteIdsWord, NotesCopyAction, NotesCopyCounts, NotesCopyFilter, NotesCopyMode, NotesCopyRow,
};
use memory_notes_copy_support::{
    KEPT_HERE, NOTES, ON, SCRIPTS, TO_LOCAL, copies, ids, is_invalid, reviewed, stdin_of, target, team_ids,
    try_apply_target, try_preview_target,
};
use memory_stand_in::{CURRENT, Setup};
use pretty_assertions::assert_eq;

use crate::memory_notes_copy_support;
use crate::memory_stand_in;

#[tokio::test]
async fn a_copy_to_this_computer_lists_the_teams_entries_and_says_which_can_be_ticked() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let cache = NotesCopyCache::new();
    let to_local = target(NOTES, TO_LOCAL);

    let review = try_preview_target(&setup, &cache, to_local, ON)
        .await
        .expect("reviewed");
    assert_eq!(
        review.counts,
        NotesCopyCounts {
            added: 4,
            identical: 1,
            local_differs: 1,
            ..NotesCopyCounts::default()
        }
    );
    assert_eq!(review.total, 6);
    assert_eq!(
        team_ids(&review.rows),
        [45, 44, 42, 43, 46, 41],
        "global first, then by repository and title"
    );
    assert_eq!(copies(&setup), [["pull", "--dry-run", "--json"]]);
    assert_eq!(setup.runs().last().unwrap().timeout, Duration::from_secs(120));

    let row = |team_id: i64| {
        review
            .rows
            .iter()
            .find(|row| row.team_id == Some(team_id))
            .expect("a row")
    };
    let (differs, identical, add) = (row(45), row(44), row(42));
    assert_eq!(
        (differs.action, differs.local_id, differs.pickable),
        (NotesCopyAction::LocalDiffers, Some(3), false)
    );
    assert_eq!(
        (identical.action, identical.local_id, identical.pickable),
        (NotesCopyAction::Identical, Some(2), false)
    );
    assert_eq!(
        (add.action, add.local_id, add.pickable, add.conflict),
        (NotesCopyAction::Add, None, true, false)
    );
    assert_eq!(add.team_updated_at.as_deref(), Some("2026-10-11T10:00:00Z"));
    assert_eq!(add.repo, None);
    assert_eq!(row(41).repo.as_deref(), Some("github.com/o/r"));

    let pickable = cache
        .rows(
            to_local,
            &review.preview_id,
            NotesCopyFilter::Pickable,
            Some(1),
            Some(2),
        )
        .expect("a page of the rows to tick");
    assert_eq!((pickable.total, team_ids(&pickable.rows)), (4, vec![43, 46]));

    let wire = serde_json::to_value(row(45)).unwrap();
    assert_eq!(
        (
            wire["action"].as_str(),
            wire["localId"].as_i64(),
            wire["teamId"].as_i64()
        ),
        (Some("localDiffers"), Some(3), Some(45))
    );
    assert_eq!(serde_json::to_value(add).unwrap()["localId"], serde_json::Value::Null);
    assert_eq!(serde_json::from_str::<NotesCopyMode>("\"toLocal\"").unwrap(), TO_LOCAL);
    assert_eq!(NotesCopyMode::default(), NotesCopyMode::Stranded);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn the_picked_team_ids_reach_stdin_one_per_line_and_nothing_is_deleted() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let to_local = target(NOTES, TO_LOCAL);
    let (cache, review) = reviewed(&setup, to_local).await;

    let picked = LocalNoteIds::Ids(vec![42, 41]);
    let picks = CopyPicks::picking(&review.preview_id, &picked);

    let copied = try_apply_target(&setup, &cache, to_local, ON, &picks)
        .await
        .expect("copied");

    assert_eq!(copies(&setup)[1], ["pull", "--json", "--ids", "-"]);
    assert_eq!(stdin_of(&setup, 1).as_deref(), Some("41\n42\n"));
    assert_eq!(setup.runs().last().unwrap().timeout, Duration::from_secs(300));
    assert_eq!((copied.counts.added, copied.deleted, copied.kept), (2, 0, 0));
    assert!(copied.failed.is_empty() && copied.leftover.is_empty(), "{copied:?}");
    assert!(
        copies(&setup)
            .iter()
            .flatten()
            .all(|arg| !["--delete-copied", "--overwrite", "--scope"].contains(&arg.as_str())),
        "{:?}",
        copies(&setup)
    );

    let again = try_apply_target(&setup, &cache, to_local, ON, &picks).await;
    assert!(is_invalid(&again), "the review is used up: {again:?}");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn all_means_the_reviews_own_entries_to_add_and_a_failure_is_reported_apart() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let to_local = target(NOTES, TO_LOCAL);
    let (cache, review) = reviewed(&setup, to_local).await;

    let all = LocalNoteIds::Word(NoteIdsWord::All);
    let picks = CopyPicks::picking(&review.preview_id, &all);

    let copied = try_apply_target(&setup, &cache, to_local, ON, &picks)
        .await
        .expect("copied");

    let mut shown = team_ids(&pickable(&review.rows));
    shown.sort_unstable();
    let text = shown.iter().map(|id| format!("{id}\n")).collect::<String>();
    assert_eq!(
        stdin_of(&setup, 1),
        Some(text),
        "every entry the review offered, and no other"
    );

    assert_eq!((copied.counts.added, copied.counts.failed, copied.deleted), (3, 1, 0));
    let [failure] = copied.failed.as_slice() else {
        panic!("one failure: {:?}", copied.failed);
    };
    assert_eq!((failure.team_id, failure.local_id), (Some(43), None));
    assert_eq!(failure.title.as_deref(), Some("A note this computer refuses"));
    assert!(!failure.message.contains("hunter2"), "{}", failure.message);

    setup.scripts.shutdown().await;
}

/// The rows of a review that can be ticked.
fn pickable(rows: &[NotesCopyRow]) -> Vec<NotesCopyRow> {
    rows.iter().filter(|row| row.pickable).cloned().collect()
}

#[tokio::test]
async fn a_pick_that_is_not_an_entry_to_add_is_refused_and_the_review_stays() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let to_local = target(NOTES, TO_LOCAL);
    let (cache, review) = reviewed(&setup, to_local).await;
    let runs = copies(&setup).len();

    let none = LocalNoteIds::Word(NoteIdsWord::None);
    let all = LocalNoteIds::Word(NoteIdsWord::All);
    let refused_picks = [
        Some(LocalNoteIds::Ids(vec![44])),
        Some(LocalNoteIds::Ids(vec![45])),
        Some(LocalNoteIds::Ids(vec![42, 99])),
        Some(LocalNoteIds::Ids(Vec::new())),
        Some(LocalNoteIds::Word(NoteIdsWord::None)),
        None,
    ];

    for pick in &refused_picks {
        let picks = CopyPicks {
            preview_id: &review.preview_id,
            overwrite: &none,
            pick: pick.as_ref(),
        };

        assert!(
            is_invalid(&try_apply_target(&setup, &cache, to_local, ON, &picks).await),
            "{pick:?}"
        );
    }

    let overwriting = CopyPicks {
        preview_id: &review.preview_id,
        overwrite: &all,
        pick: Some(&all),
    };
    assert!(is_invalid(
        &try_apply_target(&setup, &cache, to_local, ON, &overwriting).await
    ));
    assert_eq!(copies(&setup).len(), runs, "no copy ran");

    let kept = LocalNoteIds::Ids(vec![42]);
    let picks = CopyPicks {
        pick: Some(&kept),
        ..CopyPicks::overwriting(&review.preview_id, &none)
    };
    try_apply_target(&setup, &cache, to_local, ON, &picks)
        .await
        .expect("the review stayed");

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn scripts_come_to_this_computer_by_name_and_with_nothing_kept_here_there_is_nothing() {
    let setup = Setup::installed(CURRENT, KEPT_HERE);
    let cache = NotesCopyCache::new();
    let to_local = target(SCRIPTS, TO_LOCAL);

    let review = try_preview_target(&setup, &cache, to_local, ON)
        .await
        .expect("reviewed");
    assert_eq!(copies(&setup), [["scripts", "pull", "--dry-run", "--json"]]);
    assert_eq!(review.counts.added, 4);
    assert_eq!(review.rows[0].title, "bump-cache", "a script's name is its title");
    assert_eq!(
        ids(&review.rows),
        [13, 12],
        "the ones that exist here have their local ids"
    );

    let one = LocalNoteIds::Ids(vec![52]);
    let picks = CopyPicks::picking(&review.preview_id, &one);
    try_apply_target(&setup, &cache, to_local, ON, &picks)
        .await
        .expect("copied");
    assert_eq!(copies(&setup)[1], ["scripts", "pull", "--json", "--ids", "-"]);
    assert_eq!(stdin_of(&setup, 1).as_deref(), Some("52\n"));
    setup.scripts.shutdown().await;

    let routed_remote = Setup::installed(CURRENT, &[]);
    let review = try_preview_target(&routed_remote, &cache, target(NOTES, TO_LOCAL), ON)
        .await
        .expect("reviewed");
    assert_eq!((review.total, review.rows.len()), (0, 0));
    assert!(review.nothing.is_some(), "the server says why");
    routed_remote.scripts.shutdown().await;
}
