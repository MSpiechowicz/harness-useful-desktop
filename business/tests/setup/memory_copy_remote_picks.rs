//! Which rows a copy to the remote database takes, against the stand-in of `fake_agent` (see
//! `src/bin/fake_agent/memory_push.rs`): nothing without a pick, a list of local ids sends exactly
//! those rows, `all` is the review's pickable rows (never an identical or team-newer one), a picked
//! conflict is the only one overwritten and an unpicked one is named nowhere, an id that is not a
//! pickable row of the review is refused with the review kept, and `stranded` still picks conflicts
//! only. What the selection holds is in `memory_copy_selection`.

use harness_business::ecosystem::{CopyPicks, CopyTarget, NotesCopyCache};
use harness_business::model::{
    LocalNoteIds, MemoryCopyKind, NoteIdsWord, NotesCopyFilter, NotesCopyPreview, NotesCopyResult, NotesCopyRow,
};
use memory_notes_copy_support::{
    KEPT_HERE, NOTES, ON, SCRIPTS, TO_REMOTE, copies, ids, is_invalid, stdin_of, target, try_apply_target,
    try_preview_target,
};
use memory_stand_in::{CURRENT, Setup, Vars};
use pretty_assertions::assert_eq;

use crate::memory_copy_selection::{entries_numbered, selection};
use crate::memory_notes_copy_support;
use crate::memory_stand_in;

/// A server whose dry run fails the fourth entry, a conflict.
const HIDDEN_CONFLICT: Vars = &[
    ("FAKE_MEMORY_PUSH", "hidden"),
    ("HARNESS_MEMORY_REPOSITORY_NOTES", "remote+local"),
    ("HARNESS_MEMORY_GLOBAL_NOTES", "remote+local"),
];

/// The local ids of the pickable rows of the review `preview_id`, by the filter.
fn pickable_ids(cache: &NotesCopyCache, copy: CopyTarget, preview_id: &str) -> Vec<i64> {
    let rows = cache
        .rows(copy, preview_id, NotesCopyFilter::Pickable, None, None)
        .expect("its rows");
    let mut found = ids(&rows.rows);

    found.sort_unstable();
    assert_eq!(rows.total as usize, found.len());

    found
}

/// A review of the `toRemote` copy of `kind` against a server with `vars`, and what keeps it.
async fn reviewed_remote(kind: MemoryCopyKind, vars: Vars) -> (Setup, NotesCopyCache, CopyTarget, NotesCopyPreview) {
    let setup = Setup::installed(CURRENT, vars);
    let cache = NotesCopyCache::new();
    let to_remote = target(kind, TO_REMOTE);
    let review = try_preview_target(&setup, &cache, to_remote, ON)
        .await
        .expect("reviewed");

    (setup, cache, to_remote, review)
}

/// A review of the `toRemote` copy of `kind` and a copy that picks `pick` of it, against the
/// stand-in with the scopes kept here.
async fn copy_picking(kind: MemoryCopyKind, pick: LocalNoteIds) -> (Setup, NotesCopyPreview, NotesCopyResult) {
    let (setup, cache, to_remote, review) = reviewed_remote(kind, KEPT_HERE).await;
    let copied = try_apply_target(
        &setup,
        &cache,
        to_remote,
        ON,
        &CopyPicks::picking(&review.preview_id, &pick),
    )
    .await
    .expect("copied");

    (setup, review, copied)
}

fn row_of(rows: &[NotesCopyRow], local_id: i64) -> &NotesCopyRow {
    rows.iter()
        .find(|row| row.local_id == Some(local_id))
        .unwrap_or_else(|| panic!("no row {local_id}"))
}

#[tokio::test]
async fn only_a_row_that_copies_something_and_does_not_fail_is_pickable_to_the_remote() {
    for (kind, first) in [(NOTES, 0), (SCRIPTS, 10)] {
        let (setup, cache, to_remote, review) = reviewed_remote(kind, KEPT_HERE).await;

        let pickable: Vec<bool> = (1..=7)
            .map(|number| row_of(&review.rows, first + number).pickable)
            .collect();
        assert_eq!(
            pickable,
            [true, false, true, true, false, true, true],
            "{kind:?}: add, conflict, conflict, update and add can be ticked, identical and team-newer cannot"
        );
        assert_eq!(
            pickable_ids(&cache, to_remote, &review.preview_id),
            [1, 3, 4, 6, 7].map(|number| first + number),
            "{kind:?}"
        );
        setup.scripts.shutdown().await;
    }

    let (setup, cache, to_remote, review) = reviewed_remote(NOTES, HIDDEN_CONFLICT).await;

    assert!(!row_of(&review.rows, 4).pickable, "a row that fails is never pickable");
    assert_eq!(pickable_ids(&cache, to_remote, &review.preview_id), [1, 3, 6, 7]);
    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_list_of_ids_sends_exactly_those_rows_and_a_picked_conflict_is_overwritten() {
    for (kind, first) in [(NOTES, 0), (SCRIPTS, 10)] {
        let picked = LocalNoteIds::Ids(vec![first + 6, first + 3, first + 6]);
        let (setup, _, copied) = copy_picking(kind, picked).await;

        assert_eq!(
            stdin_of(&setup, 1).as_deref(),
            Some(selection(&entries_numbered(kind, &[3, 6]), &[first + 3]).as_str()),
            "{kind:?}: the update and the picked conflict, which is the only one overwritten; the other conflict is named nowhere"
        );
        assert_eq!(
            (copied.deleted, copied.kept, copied.failed.len()),
            (2, 0, 0),
            "{kind:?}: the entries not picked were not sent, so they are neither copied nor deleted"
        );
        assert_eq!((copied.counts.updated, copied.counts.overwritten), (1, 1), "{kind:?}");
        setup.scripts.shutdown().await;
    }
}

#[tokio::test]
async fn a_pick_without_a_conflict_overwrites_nothing() {
    let (setup, _, _) = copy_picking(NOTES, LocalNoteIds::Ids(vec![1, 6])).await;

    assert_eq!(
        stdin_of(&setup, 1).as_deref(),
        Some(selection(&entries_numbered(NOTES, &[1, 6]), &[]).as_str())
    );
    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn all_is_the_pickable_rows_of_the_review() {
    for (kind, first) in [(NOTES, 0), (SCRIPTS, 10)] {
        let (setup, review, _) = copy_picking(kind, LocalNoteIds::Word(NoteIdsWord::All)).await;

        let named = entries_numbered(kind, &[1, 3, 4, 6, 7]);
        let shown: Vec<i64> = review
            .rows
            .iter()
            .filter(|row| row.pickable)
            .map(|row| row.local_id.unwrap_or(0))
            .collect();
        assert_eq!(shown.len(), named.len());
        assert!(
            named.iter().all(|entry| shown.contains(&entry.0)),
            "{kind:?}: {shown:?}"
        );
        assert_eq!(
            stdin_of(&setup, 1).as_deref(),
            Some(selection(&named, &[first + 3, first + 4]).as_str()),
            "{kind:?}: never the identical or the team-newer entry"
        );
        setup.scripts.shutdown().await;
    }
}

#[tokio::test]
async fn nothing_picked_or_an_id_that_is_not_pickable_is_refused_and_the_review_stays() {
    let (setup, cache, to_remote, review) = reviewed_remote(NOTES, HIDDEN_CONFLICT).await;
    let runs = copies(&setup).len();

    let none = LocalNoteIds::Word(NoteIdsWord::None);
    let all = LocalNoteIds::Word(NoteIdsWord::All);
    let refused_picks = [
        None,
        Some(LocalNoteIds::Word(NoteIdsWord::None)),
        Some(LocalNoteIds::Ids(Vec::new())),
        Some(LocalNoteIds::Ids(vec![2])),
        Some(LocalNoteIds::Ids(vec![5])),
        Some(LocalNoteIds::Ids(vec![4])),
        Some(LocalNoteIds::Ids(vec![99])),
        Some(LocalNoteIds::Ids(vec![1, 2])),
        Some(LocalNoteIds::Ids(vec![0])),
    ];

    for pick in &refused_picks {
        let picks = CopyPicks {
            preview_id: &review.preview_id,
            overwrite: &none,
            pick: pick.as_ref(),
        };

        assert!(
            is_invalid(&try_apply_target(&setup, &cache, to_remote, ON, &picks).await),
            "{pick:?}"
        );
    }

    let overwriting = CopyPicks {
        preview_id: &review.preview_id,
        overwrite: &all,
        pick: Some(&all),
    };
    assert!(
        is_invalid(&try_apply_target(&setup, &cache, to_remote, ON, &overwriting).await),
        "a pick is the choice to overwrite, so overwrite is not given"
    );
    assert_eq!(copies(&setup).len(), runs, "nothing ran");

    let one = LocalNoteIds::Ids(vec![1]);
    try_apply_target(
        &setup,
        &cache,
        to_remote,
        ON,
        &CopyPicks::picking(&review.preview_id, &one),
    )
    .await
    .expect("the review stayed");
    assert_eq!(
        stdin_of(&setup, 1).as_deref(),
        Some(selection(&entries_numbered(NOTES, &[1]), &[]).as_str())
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_stranded_copy_still_picks_conflicts_to_overwrite_and_takes_no_entry_pick() {
    let setup = Setup::installed(CURRENT, &[]);
    let cache = NotesCopyCache::new();
    let review = try_preview_target(&setup, &cache, NOTES.into(), ON)
        .await
        .expect("reviewed");

    let pickable: Vec<_> = review.rows.iter().filter(|row| row.pickable).collect();
    assert_eq!(
        pickable.iter().map(|row| row.conflict).collect::<Vec<_>>(),
        [true, true],
        "only conflicts"
    );

    let none = LocalNoteIds::Word(NoteIdsWord::None);
    let all = LocalNoteIds::Word(NoteIdsWord::All);
    let picks = CopyPicks {
        preview_id: &review.preview_id,
        overwrite: &none,
        pick: Some(&all),
    };
    assert!(
        is_invalid(&try_apply_target(&setup, &cache, NOTES.into(), ON, &picks).await),
        "a stranded copy picks conflicts, not entries"
    );

    let conflict = LocalNoteIds::Ids(vec![3]);
    try_apply_target(
        &setup,
        &cache,
        NOTES.into(),
        ON,
        &CopyPicks::overwriting(&review.preview_id, &conflict),
    )
    .await
    .expect("the review stayed");

    assert_eq!(
        copies(&setup)[1],
        ["push", "--json", "--overwrite", "-", "--delete-copied"]
    );
    assert_eq!(stdin_of(&setup, 1).as_deref(), Some("3\n"));
    setup.scripts.shutdown().await;
}
